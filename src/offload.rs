//! Déchargement d'une carte (ou de tout volume) vers un dossier local : copie vérifiée
//! des images, sans les fichiers déjà déchargés, avant de trier dans le même outil.
//!
//! Étapes : [`plan`] (inventaire de la carte, fichiers déjà déchargés d'après
//! l'historique), [`suggest_folder`] (nom de dossier proposé), [`run`] (copie, hash
//! BLAKE3 pendant la copie puis relecture de la copie, date d'origine conservée, écriture
//! atomique), puis éventuellement [`delete_sources`] (seulement les fichiers copiés *et*
//! vérifiés). Sans interface : la ligne de commande s'en sert telle quelle. Voir
//! `doc/offload.md`.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::media::{self, MediaFile};

/// Réglages du déchargement (section `[offload]` du fichier de configuration).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OffloadSettings {
    /// Dossier racine des déchargements (`~` accepté) ; le nom du dossier de la séance
    /// est ajouté dessous.
    pub destination: String,
    /// Relire chaque copie et comparer son empreinte à celle de l'original.
    pub verify: bool,
    /// Ignorer les fichiers déjà déchargés (historique), même vers un autre dossier.
    pub skip_known: bool,
    /// Fichier de l'historique (`~` accepté) ; vide : `offload-history.tsv` dans le
    /// dossier de données (`settings::data_dir`).
    pub history: String,
}

impl Default for OffloadSettings {
    fn default() -> Self {
        Self {
            destination: "~/Pictures".into(),
            verify: true,
            skip_known: true,
            history: String::new(),
        }
    }
}

/// Une image de la carte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Fichier sur la carte.
    pub source: PathBuf,
    /// Taille (octets).
    pub size: u64,
    /// Date de modification (prise de vue, sur une carte).
    pub modified: SystemTime,
    /// Déjà déchargé (d'après l'historique).
    pub known: bool,
}

impl Item {
    fn identity(&self) -> Identity {
        Identity {
            name: self
                .source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: self.size,
            modified_ns: nanos(self.modified),
        }
    }
}

fn nanos(t: SystemTime) -> u128 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
}

/// Ce qui identifie un fichier de carte d'un déchargement à l'autre : nom, taille, date.
/// (Les numéros de fichiers repartent à zéro : le nom seul ne suffit pas.)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Identity {
    name: String,
    size: u64,
    modified_ns: u128,
}

/// Inventaire d'une carte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Dossier source (racine de la carte).
    pub source: PathBuf,
    /// Images trouvées (toute la profondeur), dans l'ordre de [`media::scan`].
    pub items: Vec<Item>,
    /// Fichiers qui ne sont pas des images (vidéos, fichiers du boîtier…), non copiés.
    pub ignored: usize,
}

impl Plan {
    /// Images à copier (hors celles déjà déchargées si `skip_known`).
    pub fn to_copy(&self, skip_known: bool) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |i| !(skip_known && i.known))
    }

    /// Octets à copier.
    pub fn bytes(&self, skip_known: bool) -> u64 {
        self.to_copy(skip_known).map(|i| i.size).sum()
    }
}

/// Historique des déchargements : un fichier texte dans le dossier de données
/// (`settings::data_dir`), une ligne par fichier copié (nom, taille, date, copie). Pas
/// dans le cache, qu'on peut vider sans rien perdre.
#[derive(Debug, Default)]
pub struct History {
    path: Option<PathBuf>,
    known: HashSet<Identity>,
}

impl History {
    /// Historique lu dans `path` (vide si le fichier n'existe pas encore).
    pub fn open(path: impl Into<PathBuf>) -> io::Result<History> {
        let path = path.into();
        let mut known = HashSet::new();
        match fs::read_to_string(&path) {
            Ok(text) => {
                for line in text.lines() {
                    let mut f = line.split('\t');
                    if let (Some(name), Some(size), Some(modified)) = (f.next(), f.next(), f.next())
                        && let (Ok(size), Ok(modified_ns)) = (size.parse(), modified.parse())
                    {
                        known.insert(Identity {
                            name: name.to_string(),
                            size,
                            modified_ns,
                        });
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(History {
            path: Some(path),
            known,
        })
    }

    /// Historique des réglages : `settings.history`, sinon
    /// `<données>/offload-history.tsv`.
    pub fn open_configured(settings: &OffloadSettings) -> io::Result<History> {
        if !settings.history.trim().is_empty() {
            return History::open(crate::settings::expand_home(settings.history.trim()));
        }
        let dir = crate::settings::data_dir()
            .ok_or_else(|| io::Error::other("dossier de données introuvable (HOME absent)"))?;
        History::open(dir.join("offload-history.tsv"))
    }

    /// Historique en mémoire seulement (simulation, tests).
    pub fn in_memory() -> History {
        History::default()
    }

    fn contains(&self, item: &Item) -> bool {
        self.known.contains(&item.identity())
    }

    fn record(&mut self, item: &Item, copy: &Path) -> io::Result<()> {
        let id = item.identity();
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            let mut file = File::options().create(true).append(true).open(path)?;
            writeln!(
                file,
                "{}\t{}\t{}\t{}",
                id.name,
                id.size,
                id.modified_ns,
                copy.display()
            )?;
        }
        self.known.insert(id);
        Ok(())
    }
}

/// Inventaire de la carte `source` (toute la profondeur) ; marque les fichiers déjà
/// déchargés d'après `history`.
pub fn plan(source: &Path, history: &History) -> io::Result<Plan> {
    let files = media::scan(source, usize::MAX)?;
    let all = walkdir::WalkDir::new(source)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .count();
    let mut items = Vec::with_capacity(files.len());
    for MediaFile { path, .. } in files {
        let meta = fs::metadata(&path)?;
        let mut item = Item {
            source: path,
            size: meta.len(),
            modified: meta.modified().unwrap_or(UNIX_EPOCH),
            known: false,
        };
        item.known = history.contains(&item);
        items.push(item);
    }
    Ok(Plan {
        source: source.to_path_buf(),
        ignored: all.saturating_sub(items.len()),
        items,
    })
}

/// Nom de dossier proposé : `AAAA/AAAA-MM-JJ_`, d'après la date de prise de vue de la
/// plus ancienne image à copier (lue dans ses métadonnées, heure locale ; à défaut, sa
/// date de modification en UTC). Le `_` final invite à compléter (« _Self ») ; il est
/// retiré s'il reste seul ([`destination`]). `None` si rien à copier.
pub fn suggest_folder(plan: &Plan, skip_known: bool) -> Option<String> {
    let first = plan.to_copy(skip_known).min_by_key(|i| i.modified)?;
    let kind = media::kind_of(&first.source)?;
    let file = MediaFile {
        path: first.source.clone(),
        kind,
    };
    let date = crate::metadata::read(&file)
        .ok()
        .and_then(|m| m.date)
        .and_then(|d| d.get(0..10).map(str::to_string))
        .unwrap_or_else(|| utc_date(first.modified));
    Some(format!("{}/{date}_", &date[0..4]))
}

/// Date UTC « AAAA-MM-JJ » (calendrier grégorien, sans dépendance de dates).
fn utc_date(t: SystemTime) -> String {
    let days = (t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() / 86_400) as i64;
    // Algorithme de H. Hinnant (days_from_civil inversé).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Dossier de destination : `root` / `folder`, sans `_` ni espaces en fin de nom (un
/// suffixe proposé mais laissé vide). Erreur si `folder` est vide, absolu, ou remonte
/// (`..`) : on ne copie que sous la racine.
pub fn destination(root: &Path, folder: &str) -> io::Result<PathBuf> {
    let folder = folder.trim().trim_end_matches(['_', ' ', '-']);
    let relative = Path::new(folder);
    let invalid = folder.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)));
    if invalid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("nom de dossier invalide : « {folder} »"),
        ));
    }
    Ok(root.join(relative))
}

/// Avancement d'un déchargement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Fichiers traités (copiés, ignorés ou en erreur).
    pub files_done: usize,
    /// Fichiers à traiter.
    pub files_total: usize,
    /// Octets copiés.
    pub bytes_done: u64,
    /// Octets à copier.
    pub bytes_total: u64,
    /// Fichier en cours.
    pub current: Option<PathBuf>,
}

/// Bilan d'un déchargement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Dossier de destination.
    pub destination: PathBuf,
    /// Copies réussies (et vérifiées si demandé) : (original, copie).
    pub copied: Vec<(PathBuf, PathBuf)>,
    /// Ignorés : déjà déchargés (historique), ou copie identique déjà présente.
    pub skipped: Vec<PathBuf>,
    /// Échecs : (original, message).
    pub errors: Vec<(PathBuf, String)>,
    /// Interrompu à la demande.
    pub cancelled: bool,
    /// Durée.
    pub elapsed: Duration,
}

/// Copie les images de `plan` dans `destination` (créé au besoin), à plat (les
/// sous-dossiers de la carte, `DCIM/100_FUJI`…, ne sont pas reproduits). Un nom déjà pris
/// par un *autre* fichier reçoit un suffixe (`DSCF0001-2.RAF`) ; une copie identique déjà
/// présente est ignorée. `cancel` interrompt entre deux fichiers (la copie en cours est
/// abandonnée et effacée).
pub fn run(
    plan: &Plan,
    destination: &Path,
    settings: &OffloadSettings,
    history: &mut History,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&Progress),
) -> Report {
    let start = std::time::Instant::now();
    let mut report = Report {
        destination: destination.to_path_buf(),
        ..Report::default()
    };
    let todo: Vec<&Item> = plan.items.iter().collect();
    let mut p = Progress {
        files_total: todo.len(),
        bytes_total: plan.bytes(settings.skip_known),
        ..Progress::default()
    };
    if let Err(e) = fs::create_dir_all(destination) {
        report
            .errors
            .push((destination.to_path_buf(), e.to_string()));
        report.elapsed = start.elapsed();
        return report;
    }
    for item in todo {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        p.current = Some(item.source.clone());
        progress(&p);
        if settings.skip_known && item.known {
            report.skipped.push(item.source.clone());
        } else {
            match copy_one(
                item,
                destination,
                settings.verify,
                cancel,
                &mut p,
                &mut progress,
            ) {
                Ok(Outcome::Copied(copy)) => {
                    if let Err(e) = history.record(item, &copy) {
                        report
                            .errors
                            .push((item.source.clone(), format!("historique : {e}")));
                    }
                    report.copied.push((item.source.clone(), copy));
                }
                Ok(Outcome::AlreadyThere(copy)) => {
                    let _ = history.record(item, &copy);
                    report.skipped.push(item.source.clone());
                }
                Err(e) if cancel.load(Ordering::Relaxed) => {
                    let _ = e;
                    report.cancelled = true;
                    break;
                }
                Err(e) => report.errors.push((item.source.clone(), e.to_string())),
            }
        }
        p.files_done += 1;
    }
    p.current = None;
    progress(&p);
    report.elapsed = start.elapsed();
    report
}

enum Outcome {
    Copied(PathBuf),
    AlreadyThere(PathBuf),
}

/// Taille des lectures.
const CHUNK: usize = 1 << 20;

/// Empreinte BLAKE3 d'un fichier.
fn hash_file(path: &Path) -> io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

fn copy_one(
    item: &Item,
    dir: &Path,
    verify: bool,
    cancel: &AtomicBool,
    p: &mut Progress,
    progress: &mut impl FnMut(&Progress),
) -> io::Result<Outcome> {
    let name = item
        .source
        .file_name()
        .ok_or_else(|| io::Error::other("fichier sans nom"))?;
    // Nom libre : le nom d'origine, ou un suffixe si un autre fichier l'occupe déjà.
    let mut target = dir.join(name);
    let mut n = 1;
    while target.exists() {
        if fs::metadata(&target)?.len() == item.size
            && hash_file(&target)? == hash_file(&item.source)?
        {
            return Ok(Outcome::AlreadyThere(target));
        }
        n += 1;
        let stem = Path::new(name)
            .file_stem()
            .unwrap_or(name)
            .to_string_lossy();
        let ext = Path::new(name)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        target = dir.join(format!("{stem}-{n}{ext}"));
    }
    let tmp = dir.join(format!(
        ".{}.tri-tmp",
        target.file_name().unwrap().to_string_lossy()
    ));
    let result = (|| {
        let mut input = File::open(&item.source)?;
        let mut output = File::create(&tmp)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "interrompu"));
            }
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            output.write_all(&buf[..n])?;
            p.bytes_done += n as u64;
            progress(p);
        }
        output.sync_all()?;
        output.set_modified(item.modified)?;
        drop(output);
        if verify && hash_file(&tmp)? != hasher.finalize() {
            return Err(io::Error::other(
                "la copie relue diffère de l'original (support défectueux ?)",
            ));
        }
        fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map(|()| Outcome::Copied(target))
}

/// Supprime de la carte les originaux des copies réussies de `report` (et seulement
/// eux : ni les ignorés, ni les échecs). À ne proposer qu'après confirmation. Renvoie les
/// échecs (carte protégée en écriture…).
pub fn delete_sources(report: &Report) -> Vec<(PathBuf, String)> {
    report
        .copied
        .iter()
        .filter_map(|(source, copy)| {
            // Dernière sécurité : la copie doit toujours être là, de même taille.
            let same = fs::metadata(source)
                .and_then(|s| fs::metadata(copy).map(|c| s.len() == c.len()))
                .unwrap_or(false);
            if !same {
                return Some((
                    source.clone(),
                    "copie absente ou différente : conservé".into(),
                ));
            }
            fs::remove_file(source)
                .err()
                .map(|e| (source.clone(), e.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        let at = |secs: u64| utc_date(UNIX_EPOCH + Duration::from_secs(secs));
        assert_eq!(at(0), "1970-01-01");
        assert_eq!(at(951_782_400), "2000-02-29", "année bissextile");
        assert_eq!(at(1_792_108_800), "2026-10-16");
    }

    #[test]
    fn destinations_stay_under_the_root() {
        let root = Path::new("/photos");
        assert_eq!(
            destination(root, "2026/2026-10-16_Self").unwrap(),
            PathBuf::from("/photos/2026/2026-10-16_Self")
        );
        assert_eq!(
            destination(root, "2026/2026-10-16_").unwrap(),
            PathBuf::from("/photos/2026/2026-10-16"),
            "suffixe laissé vide"
        );
        for bad in ["", "  ", "../ailleurs", "/abs", "a/../../b"] {
            assert!(destination(root, bad).is_err(), "« {bad} »");
        }
    }
}
