//! Un dossier ouvert : ses images, leurs notes, et la liste à afficher selon les filtres.
//!
//! C'est ce qu'une interface (graphique, TUI) ou la ligne de commande manipule quand on
//! « ouvre un dossier » : parcours ([`crate::media::scan`]), notes
//! ([`crate::notes::NoteBook`]), puis filtres par type et par notes, combinés ici une fois
//! pour toutes. Voir `doc/en/folder.md`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};

use crate::develop::Engine;
use crate::filter::{Filter, Groups, KindFilter, ungrouped};
use crate::media::{self, MediaFile};
use crate::notes::{NoteBook, NotesRule};
use crate::xmp::Convention;

/// Dossier ouvert.
#[derive(Debug, Default)]
pub struct Folder {
    /// Dossier, en chemin absolu (vide si aucun dossier n'est ouvert).
    pub dir: PathBuf,
    /// Images du dossier, dans l'ordre de [`media::scan`].
    pub files: Vec<MediaFile>,
    /// Notes de ces images.
    pub notes: NoteBook,
}

impl Folder {
    /// Ouvre `dir` : parcourt ses images sur `depth` niveaux (1 : le dossier seul) et lit
    /// leurs notes ; `convention` : nom d'un sidecar créé par une note. Le chemin est
    /// rendu absolu (sans résoudre les liens). Erreur si `dir` n'est pas un dossier
    /// lisible.
    pub fn open(dir: &Path, depth: usize, convention: Convention) -> io::Result<Folder> {
        let dir = std::path::absolute(dir)?;
        let files = media::scan(&dir, depth)?;
        settle(&dir, &files);
        let notes = NoteBook::load_files(&files, convention);
        Ok(Folder { dir, files, notes })
    }

    /// Comme [`Folder::open`], sans lire les notes : la liste des images seulement, rapide
    /// même sur un disque lent ou un NAS ; les notes se lisent ensuite en fond
    /// ([`Loading`]) et restent vides en attendant ([`NoteBook::pending`]). Issue n° 15 :
    /// 2 820 images sur un disque dur externe, 15 s sans rien à l'écran.
    pub fn scan(dir: &Path, depth: usize, convention: Convention) -> io::Result<Folder> {
        Folder::scan_counting(
            dir,
            depth,
            convention,
            std::time::Duration::MAX,
            &AtomicBool::new(false),
            &AtomicUsize::new(0),
        )
    }

    /// Comme [`Folder::scan`], interruptible (`stop`) et avec le nombre d'images trouvées
    /// (`found`) : à lancer dans un fil à part pour un dossier lent
    /// ([`media::scan_counting`]).
    pub fn scan_counting(
        dir: &Path,
        depth: usize,
        convention: Convention,
        slow: std::time::Duration,
        stop: &AtomicBool,
        found: &AtomicUsize,
    ) -> io::Result<Folder> {
        let dir = std::path::absolute(dir)?;
        // Le dossier lui-même tout de suite (ses filtres et curseurs se lisent à
        // l'ouverture) ; ses sous-dossiers avec les notes ([`Loading`]).
        crate::store::settle(crate::store::data_dir().as_deref(), &dir);
        let files = media::scan_counting(&dir, depth, slow, stop, found)?;
        let notes = NoteBook::pending(files.iter().map(|f| f.path.as_path()), convention);
        Ok(Folder { dir, files, notes })
    }
}

/// Met en ordre les données de tri ([`crate::store::settle`]) de `dir` et des dossiers de
/// `files` (sous-dossiers parcourus), avant d'en lire les notes.
fn settle(dir: &Path, files: &[MediaFile]) {
    let data = crate::store::data_dir();
    let mut dirs: std::collections::BTreeSet<&Path> =
        files.iter().filter_map(|f| f.path.parent()).collect();
    dirs.insert(dir);
    for d in dirs {
        crate::store::settle(data.as_deref(), d);
    }
}

/// Ce que lit [`Loading`] en fond.
pub struct Loaded {
    /// Les notes des images.
    pub notes: NoteBook,
    /// État du dossier pour sa surveillance ([`crate::watch::FolderWatch::start_from`]),
    /// relevé dans la foulée plutôt qu'au démarrage de la surveillance (un accès disque
    /// par fichier).
    pub snapshot: Option<crate::watch::Snapshot>,
}

/// Lecture en fond de ce qu'un dossier ouvert par [`Folder::scan`] attend : ses notes
/// et l'état de sa surveillance, avec la progression.
pub struct Loading {
    done: Arc<AtomicUsize>,
    total: usize,
    result: Receiver<Loaded>,
}

impl Loading {
    /// Lance la lecture des notes de `folder` (convention de ses notes), et l'état de sa
    /// surveillance sur `depth` niveaux si `watch`.
    pub fn start(folder: &Folder, depth: usize, watch: bool) -> Loading {
        let done = Arc::new(AtomicUsize::new(0));
        let (tx, result) = mpsc::channel();
        let total = folder.files.len();
        let files = folder.files.clone();
        let dir = folder.dir.clone();
        let convention = folder.notes.convention();
        let counter = done.clone();
        let spawned = std::thread::Builder::new()
            .name("tri-folder".into())
            .spawn(move || {
                settle(&dir, &files);
                let notes = NoteBook::load_counting(&files, convention, &counter);
                let snapshot = watch
                    .then(|| crate::watch::Snapshot::of_depth(&dir, depth).ok())
                    .flatten();
                let _ = tx.send(Loaded { notes, snapshot });
            });
        if spawned.is_err() {
            // Pas de fil : lecture sur place, plutôt que rien.
            let (tx, rx) = mpsc::channel();
            let notes = NoteBook::load_counting(&folder.files, convention, &done);
            let _ = tx.send(Loaded {
                notes,
                snapshot: None,
            });
            return Loading {
                done,
                total,
                result: rx,
            };
        }
        Loading {
            done,
            total,
            result,
        }
    }

    /// Images lues, sur le total.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.done.load(Ordering::Relaxed).min(self.total),
            self.total,
        )
    }

    /// Le résultat, une fois la lecture finie.
    pub fn try_take(&self) -> Option<Loaded> {
        self.result.try_recv().ok()
    }
}

impl Folder {
    /// Vrai si aucun dossier n'est ouvert.
    pub fn is_none(&self) -> bool {
        self.dir.as_os_str().is_empty()
    }

    /// Images à afficher : filtre par type, puis filtre sur les notes.
    pub fn filtered(&self, kind: KindFilter, rule: &NotesRule) -> Groups {
        rule.filter(&self.notes)
            .apply(kind.apply(ungrouped(self.files.clone())))
    }
}

/// Résumé d'un dossier (issue n° 388) : de quoi il est fait, sur **tout** le dossier,
/// pas la vue filtrée (cliquer une ligne pour filtrer, plus tard, ne doit pas changer les
/// autres). Les développements (clones, copies, principaux montrés à part) sont comptés à
/// part des photos, par logiciel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Photos (sans les développements).
    pub photos: usize,
    /// Photos par extension (en majuscules), RAW d'abord, puis par nombre décroissant.
    pub types: Vec<TypeCount>,
    /// Paires RAW+JPEG (ou autre image plate) : même nom, même dossier.
    pub pairs: usize,
    /// Développements par logiciel, dans l'ordre de [`Engine::ALL`] (ceux qui en ont).
    pub developments: Vec<(Engine, usize)>,
    /// Photos sans aucune note (ni étoile, ni drapeau, ni couleur).
    pub untouched: usize,
    /// Photos avec des étoiles.
    pub starred: usize,
    /// Photos acceptées.
    pub accepted: usize,
    /// Photos rejetées.
    pub rejected: usize,
    /// Photos avec au moins un libellé couleur.
    pub colored: usize,
    /// Photos par libellé couleur, dans l'ordre de [`crate::xmp::Color::ALL`] (celles qui
    /// en ont ; une photo à plusieurs couleurs compte pour chacune).
    pub colors: Vec<(crate::xmp::Color, usize)>,
    /// Images dont les notes sont en conflit (sources en désaccord).
    pub conflicts: usize,
}

/// Nombre de photos d'une extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeCount {
    /// Extension, en majuscules (« RAF », « JPG »).
    pub extension: String,
    /// RAW ou image plate.
    pub kind: media::MediaKind,
    /// Nombre de photos.
    pub count: usize,
}

impl Summary {
    /// Résumé de `files` : `notes` donne les notes d'une image, `engine_of` le logiciel
    /// d'un développement (`None` : une photo), `in_conflict` si ses notes sont en
    /// conflit.
    pub fn of<'a>(
        files: &[MediaFile],
        notes: impl Fn(&Path) -> &'a crate::xmp::Notes,
        engine_of: impl Fn(&Path) -> Option<Engine>,
        in_conflict: impl Fn(&Path) -> bool,
    ) -> Summary {
        use crate::xmp::Flag;
        use std::collections::HashMap;
        let mut summary = Summary::default();
        let mut types: HashMap<(String, media::MediaKind), usize> = HashMap::new();
        let mut developments: HashMap<Engine, usize> = HashMap::new();
        let mut colors: HashMap<crate::xmp::Color, usize> = HashMap::new();
        for file in files {
            if let Some(engine) = engine_of(&file.path) {
                *developments.entry(engine).or_default() += 1;
                continue;
            }
            summary.photos += 1;
            let extension = file
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            *types.entry((extension, file.kind)).or_default() += 1;
            let n = notes(&file.path);
            match n.flag {
                Flag::Accepted => summary.accepted += 1,
                Flag::Rejected => summary.rejected += 1,
                Flag::None => {}
            }
            if n.stars > 0 {
                summary.starred += 1;
            }
            if !n.colors.is_empty() {
                summary.colored += 1;
            }
            for &c in &n.colors {
                *colors.entry(c).or_default() += 1;
            }
            if n.stars == 0 && n.flag == Flag::None && n.colors.is_empty() {
                summary.untouched += 1;
            }
            if in_conflict(&file.path) {
                summary.conflicts += 1;
            }
        }
        summary.pairs = crate::pairs::camera_jpegs(files).len();
        let mut types: Vec<TypeCount> = types
            .into_iter()
            .map(|((extension, kind), count)| TypeCount {
                extension,
                kind,
                count,
            })
            .collect();
        types.sort_by(|a, b| {
            (
                a.kind != media::MediaKind::Raw,
                std::cmp::Reverse(a.count),
                &a.extension,
            )
                .cmp(&(
                    b.kind != media::MediaKind::Raw,
                    std::cmp::Reverse(b.count),
                    &b.extension,
                ))
        });
        summary.types = types;
        summary.colors = crate::xmp::Color::ALL
            .into_iter()
            .filter_map(|c| colors.get(&c).map(|&n| (c, n)))
            .collect();
        summary.developments = Engine::ALL
            .into_iter()
            .filter_map(|e| developments.get(&e).map(|&n| (e, n)))
            .collect();
        summary
    }
}

impl Folder {
    /// Résumé du dossier ouvert ([`Summary`]), développements d'après le registre
    /// ([`crate::developments`]).
    pub fn summary(&self) -> Summary {
        Summary::of(
            &self.files,
            |p| self.notes.notes(p),
            |p| crate::developments::get(p).map(|d| d.engine),
            |p| self.notes.conflict(p).is_some(),
        )
    }
}

/// Fichier du choix « descendre dans les sous-dossiers », dans le dossier de données.
const SUBFOLDERS: &str = "subfolders";

/// Vrai si les sous-dossiers du dossier ouvert sont parcourus aussi (case du panneau des
/// dossiers, gardée d'une session à l'autre ; voir [`remember_subfolders`]).
pub fn subfolders(data: &Path) -> bool {
    std::fs::read_to_string(data.join(SUBFOLDERS)).is_ok_and(|t| t.trim() == "on")
}

/// Retient le choix « descendre dans les sous-dossiers ».
pub fn remember_subfolders(data: &Path, on: bool) -> io::Result<()> {
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join(SUBFOLDERS), if on { "on\n" } else { "off\n" })
}

/// Mémorise `dir` comme dernier dossier ouvert (fichier `last-folder` du dossier de
/// données `data`, voir `EngineSettings::data_dir`), pour le rouvrir au prochain
/// lancement.
pub fn remember(data: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join("last-folder"), path_bytes(dir))
}

/// Dernier dossier ouvert ([`remember`]) dans le dossier de données `data`, s'il existe
/// toujours.
pub fn last(data: &Path) -> Option<PathBuf> {
    remembered(data).filter(|p| p.is_dir())
}

/// Dernier dossier ouvert ([`remember`]), qu'il existe ou non (disque débranché depuis :
/// l'interface le dit, et le rouvre quand il revient, issue n° 66).
pub fn remembered(data: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(data.join("last-folder")).ok()?;
    bytes_path(bytes)
}

/// Octets d'un chemin : bruts sous Unix (les noms non UTF-8 survivent), UTF-8 ailleurs.
#[cfg(unix)]
pub(crate) fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
pub(crate) fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
pub(crate) fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
pub(crate) fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_types_pairs_developments_and_culling() {
        use crate::media::MediaKind::{Flat, Raw};
        use crate::xmp::{Color, Flag, Notes};
        let file = |p: &str, kind| MediaFile {
            path: PathBuf::from(p),
            kind,
        };
        let files = vec![
            file("/s/A.RAF", Raw),
            file("/s/A.JPG", Flat),
            file("/s/B.RAF", Raw),
            file("/s/C.RAF", Raw),
            file("/s/D.jpg", Flat),
            file("/s/E.NEF", Raw),
            file("/s/A@dt.RAF", Raw),
            file("/s/A_01@dt.RAF", Raw),
            file("/s/B@rr.RAF", Raw),
        ];
        let accepted = Notes {
            stars: 3,
            flag: Flag::Accepted,
            colors: Vec::new(),
        };
        let rejected = Notes {
            stars: 0,
            flag: Flag::Rejected,
            colors: Vec::new(),
        };
        let red = Notes {
            stars: 0,
            flag: Flag::None,
            colors: vec![Color::Red],
        };
        let empty = Notes::EMPTY;
        let summary = Summary::of(
            &files,
            |p| match p.to_str().unwrap() {
                "/s/A.RAF" => &accepted,
                "/s/B.RAF" => &rejected,
                "/s/C.RAF" => &red,
                _ => &empty,
            },
            |p| {
                let s = p.to_str().unwrap();
                if s.contains("@dt") {
                    Some(Engine::Darktable)
                } else if s.contains("@rr") {
                    Some(Engine::RapidRaw)
                } else {
                    None
                }
            },
            |p| p.ends_with("D.jpg"),
        );
        assert_eq!(summary.photos, 6, "sans les développements");
        let types: Vec<(&str, usize)> = summary
            .types
            .iter()
            .map(|t| (t.extension.as_str(), t.count))
            .collect();
        assert_eq!(types, [("RAF", 3), ("NEF", 1), ("JPG", 2)], "RAW d'abord");
        assert_eq!(summary.pairs, 1, "A.RAF + A.JPG");
        assert_eq!(
            summary.developments,
            [(Engine::Darktable, 2), (Engine::RapidRaw, 1)]
        );
        assert_eq!(
            (summary.accepted, summary.rejected, summary.starred),
            (1, 1, 1)
        );
        assert_eq!(summary.untouched, 3, "A.JPG, D.jpg, E.NEF");
        assert_eq!(summary.colored, 1);
        assert_eq!(summary.colors, [(Color::Red, 1)]);
        assert_eq!(summary.conflicts, 1);
    }

    #[test]
    fn a_folder_lists_at_once_and_reads_its_notes_in_the_background() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("A.jpg");
        image::RgbImage::new(2, 2).save(&image).unwrap();
        let notes = crate::xmp::Notes {
            stars: 4,
            ..Default::default()
        };
        crate::xmp::write(&image, &notes, Convention::Darktable).unwrap();
        let mut folder = Folder::scan(dir.path(), 1, Convention::Darktable).unwrap();
        assert_eq!(folder.files.len(), 1);
        assert!(folder.notes.is_pending());
        assert_eq!(folder.notes.notes(&image).stars, 0, "pas encore lues");
        let loading = Loading::start(&folder, 1, true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let loaded = loop {
            if let Some(loaded) = loading.try_take() {
                break loaded;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(loading.progress(), (1, 1));
        assert!(loaded.snapshot.is_some(), "état pour la surveillance");
        folder.notes.adopt(loaded.notes);
        assert_eq!(folder.notes.notes(&image).stars, 4);
        assert!(!folder.notes.is_pending());
    }

    #[test]
    fn last_folder_is_remembered_if_it_still_exists() {
        let data = tempfile::tempdir().unwrap();
        let photos = tempfile::tempdir().unwrap();
        assert_eq!(last(data.path()), None);
        remember(data.path(), photos.path()).unwrap();
        assert_eq!(last(data.path()).as_deref(), Some(photos.path()));
        let gone = photos.path().to_path_buf();
        drop(photos);
        assert_eq!(last(data.path()), None, "dossier disparu : rien");
        assert_eq!(remembered(data.path()), Some(gone), "mais toujours retenu");
    }

    #[test]
    fn subfolders_are_listed_on_demand_and_the_choice_is_kept() {
        let data = tempfile::tempdir().unwrap();
        let photos = tempfile::tempdir().unwrap();
        let p = photos.path();
        for d in ["2026/jour 1", "2026/.cache", "a/b/c"] {
            std::fs::create_dir_all(p.join(d)).unwrap();
        }
        for f in [
            "x.jpg",
            "2026/jour 1/y.jpg",
            "2026/.cache/z.jpg",
            "a/b/c/w.jpg",
        ] {
            std::fs::write(p.join(f), b"").unwrap();
        }
        let names = |depth| {
            let folder = Folder::open(p, depth, Convention::Darktable).unwrap();
            folder
                .files
                .iter()
                .map(|f| f.path.strip_prefix(p).unwrap().to_path_buf())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(1), [PathBuf::from("x.jpg")]);
        assert_eq!(
            names(3),
            [PathBuf::from("2026/jour 1/y.jpg"), PathBuf::from("x.jpg")],
            "dossiers cachés écartés, 3 niveaux au plus"
        );
        assert!(!subfolders(data.path()), "pas encore choisi : non");
        remember_subfolders(data.path(), true).unwrap();
        assert!(subfolders(data.path()));
        remember_subfolders(data.path(), false).unwrap();
        assert!(!subfolders(data.path()));
    }

    #[test]
    fn default_is_no_folder() {
        let folder = Folder::default();
        assert!(folder.is_none());
        assert!(
            folder
                .filtered(KindFilter::All, &NotesRule::default())
                .is_empty()
        );
    }
}
