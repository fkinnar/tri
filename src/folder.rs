//! Un dossier ouvert : ses images, leurs notes, et la liste à afficher selon les filtres.
//!
//! C'est ce qu'une interface (graphique, TUI) ou la ligne de commande manipule quand on
//! « ouvre un dossier » : parcours ([`crate::media::scan`]), notes
//! ([`crate::notes::NoteBook`]), puis filtres par type et par notes, combinés ici une fois
//! pour toutes. Voir `doc/en/folder.md`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};

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
        let notes = NoteBook::load_files(&files, convention);
        Ok(Folder { dir, files, notes })
    }

    /// Comme [`Folder::open`], sans lire les notes : la liste des images seulement, rapide
    /// même sur un disque lent ou un NAS ; les notes se lisent ensuite en fond
    /// ([`Loading`]) et restent vides en attendant ([`NoteBook::pending`]). Issue n° 15 :
    /// 2 820 images sur un disque dur externe, 15 s sans rien à l'écran.
    pub fn scan(dir: &Path, depth: usize, convention: Convention) -> io::Result<Folder> {
        let dir = std::path::absolute(dir)?;
        let files = media::scan(&dir, depth)?;
        let notes = NoteBook::pending(files.iter().map(|f| f.path.as_path()), convention);
        Ok(Folder { dir, files, notes })
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

/// Valeur retenue pour le dossier `dir` dans le fichier `file` du dossier de données
/// `data` (une ligne par dossier : valeur, tabulation, chemin), s'il y en a une : seuil
/// des piles, sévérité du flou… réglés par un curseur et gardés par dossier.
pub fn folder_value(data: &Path, file: &str, dir: &Path) -> Option<f32> {
    let text = std::fs::read_to_string(data.join(file)).ok()?;
    let wanted = dir.to_string_lossy();
    text.lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, path)| *path == wanted)
        .and_then(|(v, _)| v.parse().ok())
}

/// Retient `value` pour le dossier `dir` dans le fichier `file` du dossier de données
/// `data` ; `None` : l'oublie (valeur par défaut).
pub fn remember_folder_value(
    data: &Path,
    file: &str,
    dir: &Path,
    value: Option<f32>,
) -> io::Result<()> {
    let path = data.join(file);
    let wanted = dir.to_string_lossy().into_owned();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.split_once('\t').is_some_and(|(_, p)| p != wanted))
        .map(str::to_string)
        .collect();
    if let Some(v) = value {
        lines.push(format!("{v}\t{wanted}"));
    }
    std::fs::create_dir_all(data)?;
    std::fs::write(path, lines.join("\n") + "\n")
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
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

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
