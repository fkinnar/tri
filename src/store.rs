//! Données de tri d'un dossier de photos (issue n° 378, décidé le 11/11/2026).
//!
//! Ce que `tri` retient d'un dossier (mémoire des notes, filtres et tri, curseurs, choix
//! de couvertures…) va dans un petit fichier par sorte, rangé selon **un seul
//! interrupteur** : « garder les données de tri dans ce dossier / sur ce disque ».
//!
//! - **Activé** (le fichier [`KEEP`] dans le `.tri/` d'un dossier parent, ou du dossier
//!   lui-même ; voir [`holder`]) : chaque dossier en dessous garde ses fichiers dans son
//!   propre `.tri/`, et le cache de tout ce qui est dessous va dans `<racine>/.tri/cache/`
//!   ([`crate::cache`]). Un disque qu'on emporte sur une autre machine (ou un dossier
//!   synchronisé) garde tout : noms relatifs seulement, jamais de chemin absolu.
//! - **Désactivé** (par défaut) : rien n'est écrit dans le dossier des photos ; les
//!   fichiers vont dans le dossier de données de `tri`, sous une empreinte du chemin du
//!   dossier (`folders/<empreinte>/`). Même repli pour un dossier en lecture seule.
//!
//! Le repère est un fichier (pas la présence de `.tri/cache/`) : qui exclut le cache
//! d'une synchronisation (Syncthing…) garde l'interrupteur sur l'autre machine.
//!
//! Pas de verrou (il ne protège rien entre deux machines hors ligne) : chaque écriture
//! relit le fichier juste avant, ne change que sa propre clé, et remplace le fichier d'un
//! coup. Voir `doc/en/store.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Dossier caché de `tri` dans un dossier de photos.
pub const DIR: &str = ".tri";
/// Repère de l'interrupteur, dans le `.tri/` de sa racine.
pub const KEEP: &str = "keep";
/// Cache gardé avec les photos, dans le `.tri/` de la racine de l'interrupteur.
pub const CACHE: &str = "cache";
/// Mémoire des notes ([`crate::reconcile::Memory`]).
pub const NOTES: &str = "notes.json";
/// Filtres, tri et rendu des vignettes, retenus par l'interface.
pub const VIEW: &str = "view.json";
/// Valeurs des curseurs du dossier ([`value`]).
pub const TUNING: &str = "tuning.json";
/// Couvertures de piles et développements préférés ([`chosen`]).
pub const CHOICES: &str = "choices.json";

/// Fichiers de données d'un dossier (ce qui se déplace entre `.tri/` et le repli).
const FILES: [&str; 4] = [NOTES, VIEW, TUNING, CHOICES];
/// Dossier des replis, dans le dossier de données.
const FOLDERS: &str = "folders";
/// Chemin du dossier d'un repli (pour retrouver ceux d'un disque), dans le repli.
const FOLDER_FILE: &str = "folder";
/// Cache gardé avec les photos avant le 11/11/2026 (`<racine>/.tri-cache`).
pub const LEGACY_CACHE: &str = ".tri-cache";

/// Contenu du repère [`KEEP`] (lu par personne : il explique le fichier à qui l'ouvre).
const KEEP_TEXT: &str = "tri keeps its data for the folders below here (cache in .tri/cache, can be excluded \
     from a sync).\n";

// --- Dossier de données -------------------------------------------------------------------

/// Dossier de données de `tri`, pour les replis ([`set_data_dir`]).
static DATA_DIR: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// Dossier de données de `tri`, donné une fois au démarrage par l'interface ou la ligne
/// de commande, pour les fonctions qui ne le reçoivent pas (mémoire des notes, mise en
/// ordre d'un dossier ouvert). Sans lui, un dossier dont l'interrupteur est désactivé
/// garde ses données dans son `.tri/` (comme avant le 11/11/2026), plutôt que nulle part.
pub fn set_data_dir(dir: Option<PathBuf>) {
    if let Ok(mut d) = DATA_DIR.write() {
        *d = dir;
    }
}

/// Dossier de données donné par [`set_data_dir`].
pub fn data_dir() -> Option<PathBuf> {
    DATA_DIR.read().ok().and_then(|d| d.clone())
}

// --- Interrupteur ---------------------------------------------------------------------------

/// Génération des racines : changée à chaque interrupteur posé ou retiré ([`changed`]).
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Racine de chaque dossier déjà vu, et la génération où elle a été lue.
type Memo = (u64, HashMap<PathBuf, Option<PathBuf>>);
static MEMO: Mutex<Option<Memo>> = Mutex::new(None);

/// Racine de l'interrupteur qui couvre `folder` (absolu) : le plus proche dossier parent
/// (ou `folder` lui-même) dont le `.tri/` contient [`KEEP`] ; `None` : désactivé. Gardée
/// en mémoire par dossier (un accès disque par parent, lent sur un NAS).
pub fn holder(folder: &Path) -> Option<PathBuf> {
    let generation = GENERATION.load(Ordering::Relaxed);
    let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
    let memo = memo.get_or_insert_with(|| (generation, HashMap::new()));
    if memo.0 != generation {
        *memo = (generation, HashMap::new());
    }
    memo.1
        .entry(folder.to_path_buf())
        .or_insert_with(|| {
            folder
                .ancestors()
                .find(|a| a.join(DIR).join(KEEP).is_file())
                .map(Path::to_path_buf)
        })
        .clone()
}

/// Vrai si l'interrupteur couvre `folder` ([`holder`]).
pub fn kept(folder: &Path) -> bool {
    holder(folder).is_some()
}

/// Oublie les racines retenues par [`holder`] (interrupteur posé ou retiré, ici ou par une
/// synchronisation).
pub fn changed() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// `.tri/` de `folder`.
pub fn own_dir(folder: &Path) -> PathBuf {
    folder.join(DIR)
}

/// Cache gardé dans `root` (racine de l'interrupteur).
pub fn cache_dir(root: &Path) -> PathBuf {
    root.join(DIR).join(CACHE)
}

/// Repli de `folder` dans le dossier de données `data` : nommé d'après le chemin du
/// dossier (même empreinte que les replis de la mémoire des notes d'avant).
fn fallback_dir(data: &Path, folder: &Path) -> PathBuf {
    data.join(FOLDERS).join(hash(folder))
}

fn hash(folder: &Path) -> String {
    let hash = blake3::hash(folder.to_string_lossy().as_bytes()).to_hex();
    hash[..32].to_string()
}

/// Où lire le fichier `name` de `folder`, dans l'ordre : son `.tri/` d'abord si
/// l'interrupteur le couvre (le repli : écrit pendant que le dossier était en lecture
/// seule), le repli d'abord sinon.
fn read_places(data: Option<&Path>, folder: &Path, name: &str) -> Vec<PathBuf> {
    let own = own_dir(folder).join(name);
    match data {
        None => vec![own],
        Some(data) => {
            let fallback = fallback_dir(data, folder).join(name);
            if kept(folder) {
                vec![own, fallback]
            } else {
                vec![fallback, own]
            }
        }
    }
}

// --- Lecture et écriture --------------------------------------------------------------------

/// Contenu du fichier `name` des données de `folder` (absolu), s'il y en a un ; `data` :
/// dossier de données, pour le repli.
pub fn read(data: Option<&Path>, folder: &Path, name: &str) -> Option<Vec<u8>> {
    read_places(data, folder, name)
        .into_iter()
        .find_map(|p| std::fs::read(p).ok())
}

/// Écrit (`Some`) ou efface (`None`) le fichier `name` des données de `folder` (absolu) :
/// dans son `.tri/` si l'interrupteur le couvre (le repli si le dossier est en lecture
/// seule), sinon dans le repli du dossier de données `data` (son `.tri/` sans dossier de
/// données). L'autre copie, devenue périmée, est effacée ; un `.tri/` vidé disparaît.
pub fn write(
    data: Option<&Path>,
    folder: &Path,
    name: &str,
    bytes: Option<&[u8]>,
) -> io::Result<()> {
    let own = own_dir(folder).join(name);
    let fallback = data.map(|d| fallback_dir(d, folder));
    let Some(bytes) = bytes else {
        remove_file(&own);
        remove_empty_own(folder);
        if let Some(f) = &fallback {
            remove_file(&f.join(name));
            remove_empty_fallback(f);
        }
        return Ok(());
    };
    let into_fallback = |e: Option<io::Error>| -> io::Result<()> {
        let Some(f) = &fallback else {
            return Err(e.unwrap_or_else(|| io::Error::other("no data folder")));
        };
        write_fallback(f, folder, name, bytes)
    };
    if fallback.is_none() || kept(folder) {
        match write_own(folder, name, bytes) {
            Ok(()) => {
                if let Some(f) = &fallback {
                    remove_file(&f.join(name));
                    remove_empty_fallback(f);
                }
                Ok(())
            }
            // Lecture seule (carte verrouillée, NAS) : dans le dossier de données.
            Err(e) => into_fallback(Some(e)),
        }
    } else {
        into_fallback(None)?;
        remove_file(&own);
        remove_empty_own(folder);
        Ok(())
    }
}

fn write_own(folder: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let dir = own_dir(folder);
    write_atomic(&dir.join(name), bytes)?;
    hide(&dir);
    Ok(())
}

fn write_fallback(dir: &Path, folder: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    write_atomic(&dir.join(name), bytes)?;
    let marker = dir.join(FOLDER_FILE);
    if !marker.is_file() {
        write_atomic(&marker, crate::folder::path_bytes(folder).as_slice())?;
    }
    Ok(())
}

/// Remplace `path` d'un coup (fichier temporaire, puis renommage) : une synchronisation ou
/// un autre processus ne voit jamais un fichier à moitié écrit.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn remove_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Retire le `.tri/` de `folder` s'il est vide.
fn remove_empty_own(folder: &Path) {
    let _ = std::fs::remove_dir(own_dir(folder));
}

/// Retire un repli qui ne garde plus rien (seulement son [`FOLDER_FILE`]).
fn remove_empty_fallback(dir: &Path) {
    if FILES.iter().all(|f| !dir.join(f).exists()) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Le fichier JSON `name` des données de `folder` ; absent ou illisible : la valeur par
/// défaut.
pub fn read_json<T: DeserializeOwned + Default>(
    data: Option<&Path>,
    folder: &Path,
    name: &str,
) -> T {
    read(data, folder, name)
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Relit le fichier JSON `name` des données de `folder`, le change par `change`, et
/// l'écrit ; revenu à la valeur par défaut : effacé.
pub fn update_json<T>(
    data: Option<&Path>,
    folder: &Path,
    name: &str,
    change: impl FnOnce(&mut T),
) -> io::Result<()>
where
    T: Serialize + DeserializeOwned + Default + PartialEq,
{
    let mut value: T = read_json(data, folder, name);
    change(&mut value);
    if value == T::default() {
        return write(data, folder, name, None);
    }
    let json = serde_json::to_vec_pretty(&value).map_err(io::Error::other)?;
    write(data, folder, name, Some(&json))
}

// --- Curseurs et choix ----------------------------------------------------------------------

/// Valeur `key` des curseurs de `folder` (seuil des piles, sévérité du flou…), s'il y en
/// a une.
pub fn value(data: Option<&Path>, folder: &Path, key: &str) -> Option<f32> {
    read_json::<BTreeMap<String, f32>>(data, folder, TUNING)
        .get(key)
        .copied()
}

/// Retient `value` pour la clé `key` des curseurs de `folder` ; `None` : l'oublie (valeur
/// par défaut).
pub fn set_value(
    data: Option<&Path>,
    folder: &Path,
    key: &str,
    value: Option<f32>,
) -> io::Result<()> {
    update_json::<BTreeMap<String, f32>>(data, folder, TUNING, |values| match value {
        Some(v) => {
            values.insert(key.to_string(), v);
        }
        None => {
            values.remove(key);
        }
    })
}

/// Sorte de choix retenu par image ([`chosen`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Couverture d'une pile de rafale.
    Cover,
    /// Développement préféré d'une photo (chemin propre, voir
    /// [`crate::developments::Development::path`]).
    Development,
}

/// Contenu de [`CHOICES`] : des noms de fichiers du dossier, sans chemin.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Choices {
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    covers: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    developments: BTreeSet<String>,
}

impl Choices {
    fn of(&mut self, choice: Choice) -> &mut BTreeSet<String> {
        match choice {
            Choice::Cover => &mut self.covers,
            Choice::Development => &mut self.developments,
        }
    }
}

/// Images de `folder` choisies (`choice`), en chemins complets.
pub fn chosen(data: Option<&Path>, folder: &Path, choice: Choice) -> Vec<PathBuf> {
    let mut choices: Choices = read_json(data, folder, CHOICES);
    choices.of(choice).iter().map(|n| folder.join(n)).collect()
}

/// Oublie `forget` puis retient `add` (des images d'un même dossier) comme choix
/// `choice`.
pub fn choose(
    data: Option<&Path>,
    choice: Choice,
    forget: &[PathBuf],
    add: Option<&Path>,
) -> io::Result<()> {
    let mut by_folder: BTreeMap<PathBuf, (Vec<String>, Option<String>)> = BTreeMap::new();
    let split = |p: &Path| {
        (
            p.parent().unwrap_or(Path::new("")).to_path_buf(),
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    };
    for p in forget {
        let (dir, name) = split(p);
        by_folder.entry(dir).or_default().0.push(name);
    }
    if let Some(p) = add {
        let (dir, name) = split(p);
        by_folder.entry(dir).or_default().1 = Some(name);
    }
    for (folder, (forget, add)) in by_folder {
        update_json::<Choices>(data, &folder, CHOICES, |choices| {
            let set = choices.of(choice);
            for name in &forget {
                set.remove(name);
            }
            if let Some(name) = add {
                set.insert(name);
            }
        })?;
    }
    Ok(())
}

// --- Mise en ordre --------------------------------------------------------------------------

/// Met en ordre les données de `folder` (absolu) à son ouverture :
///
/// - un cache gardé avec les photos à l'ancienne ([`LEGACY_CACHE`], dans `folder` ou un
///   parent) devient `.tri/cache` avec son repère [`KEEP`] (c'était un choix explicite) ;
/// - les fichiers rangés du mauvais côté (interrupteur changé ici ou par une
///   synchronisation, `.tri/notes.json` écrit d'office avant le 11/11/2026) passent de
///   l'autre ; s'ils sont des deux côtés, le plus récent gagne ;
/// - ce que le dossier de données gardait pour ce dossier sous son chemin absolu (avant
///   le 11/11/2026) rejoint ses fichiers.
///
/// Au mieux : ce qui ne peut pas bouger (lecture seule) reste où il est.
pub fn settle(data: Option<&Path>, folder: &Path) {
    upgrade_legacy_cache(folder);
    let Some(data) = data else {
        return;
    };
    adopt_legacy(data, folder);
    let own = own_dir(folder);
    let fallback = fallback_dir(data, folder);
    let (from, to_own) = if kept(folder) {
        (&fallback, true)
    } else {
        (&own, false)
    };
    if !from.is_dir() {
        return;
    }
    for name in FILES {
        let source = from.join(name);
        let Ok(bytes) = std::fs::read(&source) else {
            continue;
        };
        let target = if to_own {
            own.join(name)
        } else {
            fallback.join(name)
        };
        if modified(&target).is_some_and(|t| modified(&source).is_none_or(|s| t >= s)) {
            // L'autre côté est plus récent : la copie d'ici est périmée.
            remove_file(&source);
            continue;
        }
        let written = if to_own {
            write_own(folder, name, &bytes)
        } else {
            write_fallback(&fallback, folder, name, &bytes)
        };
        if written.is_ok() {
            remove_file(&source);
        }
    }
    if to_own {
        remove_empty_fallback(&fallback);
    } else {
        remove_empty_own(folder);
    }
}

fn modified(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// [`LEGACY_CACHE`] dans `folder` ou un parent : renommé en `.tri/cache`, repère posé.
fn upgrade_legacy_cache(folder: &Path) {
    for root in folder.ancestors() {
        let legacy = root.join(LEGACY_CACHE);
        if !legacy.is_dir() {
            continue;
        }
        let cache = cache_dir(root);
        let result = std::fs::create_dir_all(own_dir(root))
            .and_then(|()| {
                if cache.exists() {
                    // Déjà là (deux versions de `tri`) : on garde le nouveau.
                    std::fs::remove_dir_all(&legacy)
                } else {
                    std::fs::rename(&legacy, &cache)
                }
            })
            .and_then(|()| write_atomic(&root.join(DIR).join(KEEP), KEEP_TEXT.as_bytes()));
        if result.is_ok() {
            hide(&own_dir(root));
            changed();
        }
    }
}

/// Ce que le dossier de données gardait pour `folder` avant le 11/11/2026, rangé par
/// chemin absolu dans des fichiers communs : passé dans les fichiers du dossier, et
/// retiré des anciens.
fn adopt_legacy(data: &Path, folder: &Path) {
    // Mémoire des notes d'un dossier en lecture seule : `notes/<empreinte>.json`.
    let old_notes = data.join("notes").join(format!("{}.json", hash(folder)));
    if let Ok(bytes) = std::fs::read(&old_notes)
        && read(Some(data), folder, NOTES).is_none()
        && write(Some(data), folder, NOTES, Some(&bytes)).is_ok()
    {
        remove_file(&old_notes);
    }
    // Une ligne par dossier : « valeur<TAB>chemin ».
    let wanted = folder.to_string_lossy().into_owned();
    for (file, key) in LEGACY_VALUES {
        let path = data.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (mine, rest): (Vec<&str>, Vec<&str>) = text
            .lines()
            .partition(|l| l.split_once('\t').is_some_and(|(_, p)| p == wanted));
        let Some((value, _)) = mine.last().and_then(|l| l.split_once('\t')) else {
            continue;
        };
        let adopted = match key {
            Legacy::View => write(Some(data), folder, VIEW, Some(value.as_bytes())),
            Legacy::Value(key) => match value.parse::<f32>() {
                Ok(v) => set_value(Some(data), folder, key, Some(v)),
                Err(_) => Ok(()),
            },
        };
        if adopted.is_ok() {
            rewrite_lines(&path, &rest);
        }
    }
    // Une image par ligne, tous dossiers confondus.
    for (file, choice) in [
        ("stack-covers.txt", Choice::Cover),
        ("preferred-developments.txt", Choice::Development),
    ] {
        let path = data.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (mine, rest): (Vec<&str>, Vec<&str>) = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .partition(|l| Path::new(l).parent() == Some(folder));
        if mine.is_empty() {
            continue;
        }
        let adopted = update_json::<Choices>(Some(data), folder, CHOICES, |choices| {
            for line in &mine {
                if let Some(name) = Path::new(line).file_name() {
                    choices
                        .of(choice)
                        .insert(name.to_string_lossy().into_owned());
                }
            }
        });
        if adopted.is_ok() {
            rewrite_lines(&path, &rest);
        }
    }
}

/// Ce que contenait un ancien fichier « une ligne par dossier ».
#[derive(Clone, Copy)]
enum Legacy {
    View,
    Value(&'static str),
}

/// Anciens fichiers « une ligne par dossier » et où va leur valeur. Les clés des curseurs
/// sont celles d'aujourd'hui (voir [`crate::similarity`], [`crate::sharpness`],
/// [`crate::exposure`]).
const LEGACY_VALUES: [(&str, Legacy); 7] = [
    ("folder-views", Legacy::View),
    ("group-strictness.tsv", Legacy::Value("group-strictness")),
    ("group-pauses.tsv", Legacy::Value("group-pauses")),
    ("group-curve.tsv", Legacy::Value("group-curve")),
    ("blur-strictness.tsv", Legacy::Value("blur-strictness")),
    (
        "clip-over-strictness.tsv",
        Legacy::Value("clip-over-strictness"),
    ),
    (
        "clip-under-strictness.tsv",
        Legacy::Value("clip-under-strictness"),
    ),
];

/// Réécrit `path` avec `lines` ; plus rien : supprimé.
fn rewrite_lines(path: &Path, lines: &[&str]) {
    if lines.is_empty() {
        remove_file(path);
    } else {
        let _ = write_atomic(path, (lines.join("\n") + "\n").as_bytes());
    }
}

// --- Poser et retirer l'interrupteur --------------------------------------------------------

/// Pose l'interrupteur sur `root` : repère [`KEEP`], puis les données des dossiers en
/// dessous gardées dans le dossier de données y passent (chacune dans le `.tri/` de son
/// dossier). Le cache est l'affaire de [`crate::cache::Cache::use_disk`], qui appelle
/// cette fonction.
pub fn keep(data: Option<&Path>, root: &Path) -> io::Result<()> {
    write_atomic(&root.join(DIR).join(KEEP), KEEP_TEXT.as_bytes())?;
    hide(&own_dir(root));
    changed();
    if let Some(data) = data {
        for folder in fallbacks(data) {
            if folder.starts_with(root) && folder.is_dir() {
                settle(Some(data), &folder);
            }
        }
    }
    Ok(())
}

/// Retire l'interrupteur de `root` : repère enlevé, puis les données des dossiers en
/// dessous repartent dans le dossier de données (s'il y en a un). Le cache est l'affaire
/// de [`crate::cache::Cache::release_disk`], qui appelle cette fonction.
pub fn release(data: Option<&Path>, root: &Path) -> io::Result<()> {
    match std::fs::remove_file(root.join(DIR).join(KEEP)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    changed();
    if data.is_some() {
        let walk = walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| e.file_type().is_dir() && (e.depth() == 0 || !hidden(e)));
        for entry in walk.flatten() {
            if entry.path().join(DIR).is_dir() {
                settle(data, entry.path());
            }
        }
    }
    remove_empty_own(root);
    Ok(())
}

fn hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

/// Dossiers qui ont un repli dans le dossier de données `data`.
fn fallbacks(data: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(data.join(FOLDERS)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| std::fs::read(e.path().join(FOLDER_FILE)).ok())
        .filter_map(crate::folder::bytes_path)
        .collect()
}

/// Après le déplacement (ou le renommage) du dossier `from` en `to` : les replis de ses
/// données et de celles de ses sous-dossiers suivent (un `.tri/` est déjà parti avec son
/// dossier).
pub fn relocate(data: &Path, from: &Path, to: &Path) {
    for folder in fallbacks(data) {
        let Ok(rest) = folder.strip_prefix(from) else {
            continue;
        };
        // `join("")` ajouterait un « / » final : une autre empreinte.
        let moved = if rest.as_os_str().is_empty() {
            to.to_path_buf()
        } else {
            to.join(rest)
        };
        let (old, new) = (fallback_dir(data, &folder), fallback_dir(data, &moved));
        if new.exists() {
            continue;
        }
        if std::fs::rename(&old, &new).is_ok() {
            let _ = write_atomic(
                &new.join(FOLDER_FILE),
                crate::folder::path_bytes(&moved).as_slice(),
            );
        }
        settle(Some(data), &moved);
    }
}

/// Cache `dir` sous Windows (ailleurs, le point au début du nom suffit).
pub fn hide(dir: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, SetFileAttributesW,
        };
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY : chaîne terminée par 0, valable le temps des appels.
        unsafe {
            let attributes = GetFileAttributesW(wide.as_ptr());
            if attributes != u32::MAX && attributes & FILE_ATTRIBUTE_HIDDEN == 0 {
                SetFileAttributesW(wide.as_ptr(), attributes | FILE_ATTRIBUTE_HIDDEN);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un dossier de photos et un dossier de données, à part.
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("disque/séance");
        let data = dir.path().join("data");
        std::fs::create_dir_all(&photos).unwrap();
        (dir, photos, data)
    }

    #[test]
    fn off_by_default_nothing_is_written_next_to_the_photos() {
        let (_dir, photos, data) = setup();
        set_value(Some(&data), &photos, "group-strictness", Some(0.4)).unwrap();
        assert_eq!(value(Some(&data), &photos, "group-strictness"), Some(0.4));
        assert!(!own_dir(&photos).exists(), "rien dans le dossier");
        let fallback = fallback_dir(&data, &photos);
        assert!(fallback.join(TUNING).is_file());
        assert_eq!(
            fallbacks(&data),
            std::slice::from_ref(&photos),
            "chemin retrouvable"
        );
        // Revenu au défaut : tout disparaît.
        set_value(Some(&data), &photos, "group-strictness", None).unwrap();
        assert!(!fallback.exists());
    }

    #[test]
    fn without_a_data_folder_the_folder_keeps_its_data() {
        let (_dir, photos, _) = setup();
        write(None, &photos, VIEW, Some(b"{}")).unwrap();
        assert!(own_dir(&photos).join(VIEW).is_file());
        write(None, &photos, VIEW, None).unwrap();
        assert!(!own_dir(&photos).exists(), "`.tri/` vide retiré");
    }

    #[test]
    fn the_switch_moves_every_folder_s_data_to_the_drive_and_back() {
        let (dir, photos, data_dir) = setup();
        let disk = dir.path().join("disque");
        let other = disk.join("autre");
        std::fs::create_dir_all(&other).unwrap();
        let data = Some(data_dir.as_path());
        set_value(data, &photos, "blur-strictness", Some(-0.5)).unwrap();
        write(data, &other, VIEW, Some(b"{\"inverted\":true}")).unwrap();
        // Posé à la racine du disque : chaque dossier reçoit ses fichiers dans son `.tri/`.
        keep(data, &disk).unwrap();
        assert_eq!(holder(&photos), Some(disk.clone()));
        assert!(own_dir(&photos).join(TUNING).is_file());
        assert!(own_dir(&other).join(VIEW).is_file());
        assert!(fallbacks(&data_dir).is_empty(), "plus rien sur la machine");
        assert_eq!(value(data, &photos, "blur-strictness"), Some(-0.5));
        // Une écriture ensuite va sur le disque, systématiquement.
        set_value(data, &photos, "group-pauses", Some(0.1)).unwrap();
        assert!(!fallback_dir(&data_dir, &photos).exists());
        // Retiré : tout revient sur la machine, plus de `.tri/` nulle part.
        release(data, &disk).unwrap();
        assert_eq!(holder(&photos), None);
        assert!(!own_dir(&photos).exists() && !own_dir(&other).exists());
        assert!(!own_dir(&disk).exists());
        assert_eq!(value(data, &photos, "group-pauses"), Some(0.1));
        assert_eq!(
            read(data, &other, VIEW).as_deref(),
            Some(&b"{\"inverted\":true}"[..])
        );
    }

    #[test]
    fn choices_are_kept_by_name_in_each_image_s_folder() {
        let (dir, photos, data) = setup();
        let data = Some(data.as_path());
        let sub = dir.path().join("disque/séance/b");
        let (a1, a2, b1) = (
            photos.join("a1.RAF"),
            photos.join("a2.RAF"),
            sub.join("b1.RAF"),
        );
        choose(data, Choice::Cover, std::slice::from_ref(&a2), Some(&a1)).unwrap();
        choose(data, Choice::Cover, &[], Some(&b1)).unwrap();
        choose(data, Choice::Cover, std::slice::from_ref(&a1), Some(&a2)).unwrap();
        assert_eq!(chosen(data, &photos, Choice::Cover), [a2]);
        assert_eq!(chosen(data, &sub, Choice::Cover), [b1]);
        assert!(chosen(data, &photos, Choice::Development).is_empty());
        let json = String::from_utf8(read(data, &photos, CHOICES).unwrap()).unwrap();
        assert!(
            json.contains("\"a2.RAF\"") && !json.contains("séance"),
            "{json}"
        );
    }

    #[test]
    fn old_places_are_tidied_on_open() {
        let (dir, photos, data_dir) = setup();
        let disk = dir.path().join("disque");
        let wanted = photos.to_string_lossy().into_owned();
        // Ce que le dossier de données gardait par chemin absolu.
        std::fs::create_dir_all(data_dir.join("notes")).unwrap();
        std::fs::write(
            data_dir.join("folder-views"),
            format!("{{\"inverted\":true}}\t{wanted}\n{{}}\t/ailleurs\n"),
        )
        .unwrap();
        std::fs::write(
            data_dir.join("group-strictness.tsv"),
            format!("0.3\t{wanted}\n"),
        )
        .unwrap();
        std::fs::write(
            data_dir.join("stack-covers.txt"),
            format!("{wanted}/a.RAF\n/ailleurs/b.RAF\n"),
        )
        .unwrap();
        // `.tri/notes.json` écrit d'office avant le 11/11/2026 : l'interrupteur reste
        // désactivé, il part dans le dossier de données.
        std::fs::create_dir_all(own_dir(&photos)).unwrap();
        std::fs::write(own_dir(&photos).join(NOTES), b"{\"version\":1}").unwrap();
        let data = Some(data_dir.as_path());
        settle(data, &photos);
        assert!(!own_dir(&photos).exists());
        assert_eq!(
            read(data, &photos, NOTES).as_deref(),
            Some(&b"{\"version\":1}"[..])
        );
        assert_eq!(
            read(data, &photos, VIEW).as_deref(),
            Some(&b"{\"inverted\":true}"[..])
        );
        assert_eq!(value(data, &photos, "group-strictness"), Some(0.3));
        assert_eq!(chosen(data, &photos, Choice::Cover), [photos.join("a.RAF")]);
        let d = &data_dir;
        assert_eq!(
            std::fs::read_to_string(d.join("folder-views")).unwrap(),
            "{}\t/ailleurs\n",
            "les autres dossiers restent"
        );
        assert!(!d.join("group-strictness.tsv").exists(), "vidé : supprimé");
        assert_eq!(
            std::fs::read_to_string(d.join("stack-covers.txt")).unwrap(),
            "/ailleurs/b.RAF\n"
        );
        // Un cache gardé à l'ancienne (choix explicite) : interrupteur posé, données sur
        // le disque.
        std::fs::create_dir_all(disk.join(LEGACY_CACHE).join("v4")).unwrap();
        settle(data, &photos);
        assert!(!disk.join(LEGACY_CACHE).exists());
        assert!(cache_dir(&disk).join("v4").is_dir());
        assert_eq!(holder(&photos), Some(disk.clone()));
        assert!(own_dir(&photos).join(VIEW).is_file());
        assert!(own_dir(&photos).join(NOTES).is_file());
    }

    #[test]
    fn a_renamed_folder_keeps_its_data_on_the_machine() {
        let (dir, photos, data) = setup();
        let data = data.as_path();
        set_value(Some(data), &photos, "group-curve", Some(2.0)).unwrap();
        let moved = dir.path().join("disque/renommée");
        std::fs::rename(&photos, &moved).unwrap();
        relocate(data, &photos, &moved);
        assert_eq!(value(Some(data), &moved, "group-curve"), Some(2.0));
        assert_eq!(value(Some(data), &photos, "group-curve"), None);
        assert_eq!(fallbacks(data), [moved]);
    }
}
