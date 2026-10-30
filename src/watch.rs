//! Surveillance du dossier ouvert : images ajoutées, retirées ou modifiées, sidecars XMP
//! modifiés par un autre logiciel (darktable…).
//!
//! Par scrutation (liste du dossier, taille et date de chaque fichier, à intervalle
//! réglable) plutôt que par les notifications du système : pas de dépendance, et le même
//! comportement sur un disque local, une carte SD ou un NAS (où les notifications manquent
//! souvent). Même principe que la surveillance des volumes ([`crate::places::Monitor`]).
//! Voir `doc/en/watch.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// Réglages de la surveillance (section `[watch]` du fichier de configuration).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatchSettings {
    /// Intervalle de scrutation du dossier ouvert (ms) ; 0 : pas de surveillance.
    pub interval_ms: u64,
    /// Part du temps (en %) que la surveillance peut passer à relever le dossier : un
    /// relevé lent (NAS, partage GVFS) espace les suivants d'autant.
    pub max_share: u64,
}

impl Default for WatchSettings {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            max_share: 10,
        }
    }
}

impl WatchSettings {
    /// Rythme de la surveillance.
    pub fn pace(&self) -> Pace {
        Pace {
            interval: Duration::from_millis(self.interval_ms),
            max_share: self.max_share,
        }
    }
}

/// Rythme de la surveillance : un relevé toutes les `interval` au moins, et jamais plus
/// de `max_share` % du temps à relever (retour du 29/10/2026 : sur un partage GVFS, un
/// relevé prenait des minutes et le suivant repartait aussitôt, le réseau saturé).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    /// Intervalle minimal entre deux relevés.
    pub interval: Duration,
    /// Part du temps (en %) passée à relever, au plus (100 : pas d'espacement).
    pub max_share: u64,
}

impl From<Duration> for Pace {
    fn from(interval: Duration) -> Pace {
        Pace {
            interval,
            max_share: 100,
        }
    }
}

impl Pace {
    /// Attente avant le relevé suivant, après un relevé qui a duré `took`.
    pub fn wait_after(&self, took: Duration) -> Duration {
        let share = self.max_share.clamp(1, 100) as u32;
        let spaced = (took * 100 / share).saturating_sub(took);
        self.interval.max(spaced)
    }
}

/// Taille et date de modification d'un fichier.
type Stamp = (u64, Option<SystemTime>);

/// État d'un dossier : ses images et ses sidecars, avec taille et date. Les autres
/// fichiers (`.DS_Store`…) sont ignorés.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    images: HashMap<PathBuf, Stamp>,
    sidecars: HashMap<PathBuf, Stamp>,
}

/// Photo dont `sidecar`, créé (`created`) ou supprimé, ajoute ou retire un développement
/// ([`crate::developments`]), si elle est dans `images`.
fn development_photo(
    sidecar: &Path,
    images: &HashMap<PathBuf, Stamp>,
    created: bool,
) -> Option<PathBuf> {
    let name = sidecar.file_name()?.to_string_lossy().to_lowercase();
    let in_folder = |image: &str| {
        images
            .keys()
            .find(|p| {
                p.parent() == sidecar.parent()
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().to_lowercase() == image)
            })
            .cloned()
    };
    if let Some(image) = name.strip_suffix(".rrdata") {
        return in_folder(image).or_else(|| in_folder(image.rsplit_once('.')?.0));
    }
    let image = name.strip_suffix(".xmp")?;
    if let Some(photo) = in_folder(image) {
        // Le XMP de la photo : un développement darktable s'il en vient (supprimé : on ne
        // peut plus le lire, la liste est relue).
        return (!created || crate::developments::file_is_darktable(sidecar)).then_some(photo);
    }
    let (original, _) = crate::media::clone_name(image)?;
    in_folder(&original)
}

/// Vrai si `path` est un sidecar : XMP, ou RapidRAW (`.rrdata`), casse ignorée.
fn is_sidecar(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xmp") || e.eq_ignore_ascii_case("rrdata"))
}

impl Snapshot {
    /// État actuel de `dir` (le dossier seul, sans sous-dossiers).
    pub fn of(dir: &Path) -> std::io::Result<Snapshot> {
        Snapshot::of_depth(dir, 1)
    }

    /// État actuel de `dir` sur `depth` niveaux (1 : le dossier seul), comme
    /// `media::scan` : dossiers cachés écartés.
    pub fn of_depth(dir: &Path, depth: usize) -> std::io::Result<Snapshot> {
        let mut snapshot = Snapshot::default();
        std::fs::read_dir(dir)?;
        let entries = walkdir::WalkDir::new(dir)
            .min_depth(1)
            .max_depth(depth.max(1))
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !crate::media::is_hidden_dir(e))
            .flatten();
        for entry in entries {
            let path = entry.path().to_path_buf();
            let sidecar = is_sidecar(&path);
            if !sidecar && crate::media::kind_of(&path).is_none() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let stamp = (meta.len(), meta.modified().ok());
            if sidecar {
                snapshot.sidecars.insert(path, stamp);
            } else {
                snapshot.images.insert(path, stamp);
            }
        }
        Ok(snapshot)
    }

    /// Ce qui a changé de `self` à `newer`.
    pub fn changes(&self, newer: &Snapshot) -> Changes {
        fn diff(old: &HashMap<PathBuf, Stamp>, new: &HashMap<PathBuf, Stamp>) -> Vec<PathBuf> {
            let mut changed: Vec<PathBuf> = new
                .iter()
                .filter(|(p, s)| old.get(*p) != Some(s))
                .map(|(p, _)| p.clone())
                .chain(old.keys().filter(|p| !new.contains_key(*p)).cloned())
                .collect();
            changed.sort();
            changed
        }
        let mut images = diff(&self.images, &newer.images);
        // Développement créé ou supprimé ailleurs (clone darktable `IMG_01.RAF.xmp`,
        // `IMG.RAF.xmp` écrit par darktable, `IMG.RAF.rrdata` ou une copie virtuelle) : une
        // vignette de plus ou de moins, la liste est à relire (la photo est signalée).
        let appeared = newer
            .sidecars
            .keys()
            .filter(|p| !self.sidecars.contains_key(*p))
            .map(|p| (p, true));
        let gone = self
            .sidecars
            .keys()
            .filter(|p| !newer.sidecars.contains_key(*p))
            .map(|p| (p, false));
        for (sidecar, created) in appeared.chain(gone) {
            if let Some(photo) = development_photo(sidecar, &newer.images, created) {
                images.push(photo);
            }
        }
        // Le XMP de notes d'une photo réécrit par darktable (importée après le tri) : son
        // développement darktable apparaît.
        for (sidecar, stamp) in &newer.sidecars {
            let changed = self.sidecars.get(sidecar).is_some_and(|old| old != stamp);
            let xmp = sidecar
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
            if !changed || !xmp {
                continue;
            }
            let Some(photo) = sidecar.file_stem().map(|_| sidecar.with_extension("")) else {
                continue;
            };
            let known = crate::developments::Development {
                engine: crate::develop::Engine::Darktable,
                copy: crate::developments::Copy::Main,
                photo: photo.clone(),
                sidecar: sidecar.clone(),
            };
            if newer.images.contains_key(&photo)
                && crate::developments::get(&known.path()).is_none()
                && crate::developments::file_is_darktable(sidecar)
            {
                images.push(photo);
            }
        }
        images.sort();
        images.dedup();
        Changes {
            images,
            sidecars: diff(&self.sidecars, &newer.sidecars),
        }
    }
}

/// Changements d'un dossier : fichiers ajoutés, retirés ou modifiés, triés.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Images (la liste du dossier est à relire).
    pub images: Vec<PathBuf>,
    /// Sidecars XMP (les notes de leurs images sont à relire).
    pub sidecars: Vec<PathBuf>,
}

impl Changes {
    /// Vrai si rien n'a changé.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty() && self.sidecars.is_empty()
    }
}

/// Surveillance d'un dossier dans un fil à part. Un changement n'est signalé qu'une fois
/// le dossier **stable** pendant un intervalle : une copie en cours (fichiers qui
/// arrivent, taille qui grandit) donne un seul signal, à la fin. S'arrête quand on la
/// lâche.
pub struct FolderWatch {
    stop: Arc<AtomicBool>,
    /// Dernier état signalé (ou resynchronisé), auquel on compare.
    known: Arc<Mutex<Snapshot>>,
    dir: PathBuf,
    depth: usize,
    thread: Option<JoinHandle<()>>,
}

impl FolderWatch {
    /// Surveille `dir` toutes les `interval` ; `on_change` reçoit les changements depuis le
    /// dernier signal. Un dossier illisible (carte retirée) ne signale rien.
    pub fn start(
        dir: &Path,
        interval: impl Into<Pace>,
        on_change: impl Fn(Changes) + Send + 'static,
    ) -> FolderWatch {
        FolderWatch::start_depth(dir, 1, interval, on_change)
    }

    /// [`FolderWatch::start`], sur `depth` niveaux de sous-dossiers (1 : le dossier seul ;
    /// dossier ouvert avec ses sous-dossiers).
    pub fn start_depth(
        dir: &Path,
        depth: usize,
        interval: impl Into<Pace>,
        on_change: impl Fn(Changes) + Send + 'static,
    ) -> FolderWatch {
        let known = Snapshot::of_depth(dir, depth).unwrap_or_default();
        FolderWatch::start_from(dir, depth, interval, known, on_change)
    }

    /// [`FolderWatch::start_depth`] à partir de l'état `known`, déjà relevé (en fond, avec
    /// les notes du dossier : [`crate::folder::Loading`]) : rien à relire sur le disque au
    /// démarrage, un accès par fichier sur un disque lent (issue n° 15).
    pub fn start_from(
        dir: &Path,
        depth: usize,
        interval: impl Into<Pace>,
        known: Snapshot,
        on_change: impl Fn(Changes) + Send + 'static,
    ) -> FolderWatch {
        let pace = interval.into();
        let stop = Arc::new(AtomicBool::new(false));
        let known = Arc::new(Mutex::new(known));
        let thread = {
            let (stop, known, dir) = (stop.clone(), known.clone(), dir.to_path_buf());
            std::thread::Builder::new()
                .name("tri-watch".into())
                .spawn(move || {
                    // Dernier état vu, pour juger de la stabilité.
                    let mut last: Option<Snapshot> = None;
                    let mut wait = pace.interval;
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::park_timeout(wait);
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let started = std::time::Instant::now();
                        let read = Snapshot::of_depth(&dir, depth);
                        wait = pace.wait_after(started.elapsed());
                        let Ok(now) = read else {
                            last = None;
                            continue;
                        };
                        let stable = last.as_ref() == Some(&now);
                        last = Some(now.clone());
                        if !stable {
                            continue;
                        }
                        let changes = {
                            let mut known = known.lock().unwrap();
                            let changes = known.changes(&now);
                            *known = now;
                            changes
                        };
                        if !changes.is_empty() {
                            // Relevés retenus des fichiers changés : à refaire (les rendus
                            // d'une image développée ailleurs se recalculent).
                            crate::cache::forget_stamps(
                                changes
                                    .images
                                    .iter()
                                    .chain(&changes.sidecars)
                                    .map(PathBuf::as_path),
                            );
                            on_change(changes);
                        }
                    }
                })
                .ok()
        };
        FolderWatch {
            stop,
            known,
            dir: dir.to_path_buf(),
            depth,
            thread,
        }
    }

    /// Dossier surveillé.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Niveaux surveillés (1 : le dossier seul).
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Prend l'état actuel pour référence, sans rien signaler : après un changement fait
    /// par `tri` lui-même (renommage, corbeille, dossier relu), déjà pris en compte.
    pub fn sync(&self) {
        if let Ok(now) = Snapshot::of_depth(&self.dir, self.depth) {
            *self.known.lock().unwrap() = now;
        }
    }
}

impl FolderWatch {
    /// Arrête la surveillance sans attendre la fin d'un relevé en cours (fermeture de
    /// l'application).
    pub fn detach(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
        }
    }
}

impl Drop for FolderWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_read_spaces_the_next_ones() {
        let pace = WatchSettings::default().pace();
        let ms = Duration::from_millis;
        assert_eq!(
            pace.wait_after(ms(5)),
            ms(1000),
            "dossier local : l'intervalle"
        );
        assert_eq!(
            pace.wait_after(ms(3000)),
            ms(27_000),
            "10 % du temps au plus"
        );
        let flat = Pace::from(ms(20));
        assert_eq!(
            flat.wait_after(ms(3000)),
            ms(20),
            "100 % : pas d'espacement"
        );
    }
    use std::sync::mpsc;

    #[test]
    fn a_darktable_clone_created_or_deleted_elsewhere_changes_the_images() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("A.RAF"), b"x").unwrap();
        std::fs::write(d.join("A.RAF.xmp"), b"<x/>").unwrap();
        let before = Snapshot::of(d).unwrap();
        std::fs::write(d.join("A_01.RAF.xmp"), b"<x/>").unwrap();
        let after = Snapshot::of(d).unwrap();
        let changes = before.changes(&after);
        assert_eq!(
            changes.images,
            [d.join("A.RAF")],
            "clone créé : la photo est à relire"
        );
        assert_eq!(changes.sidecars, [d.join("A_01.RAF.xmp")]);
        assert_eq!(after.changes(&before).images, [d.join("A.RAF")], "supprimé");
        // Simple note changée : pas une image de plus.
        std::fs::write(d.join("A.RAF.xmp"), b"<x a=\"1\"/>").unwrap();
        assert!(after.changes(&Snapshot::of(d).unwrap()).images.is_empty());
    }

    #[test]
    fn snapshots_see_images_and_sidecars_only() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("A.RAF"), b"raw").unwrap();
        std::fs::write(d.join(".DS_Store"), b"x").unwrap();
        std::fs::write(d.join("notes.txt"), b"x").unwrap();
        let before = Snapshot::of(d).unwrap();
        std::fs::write(d.join("B.jpg"), b"jpeg").unwrap();
        std::fs::write(d.join("A.RAF.xmp"), b"<x/>").unwrap();
        std::fs::write(d.join("A.RAF"), b"raw, plus long").unwrap();
        std::fs::write(d.join("autre.txt"), b"x").unwrap();
        let after = Snapshot::of(d).unwrap();
        let changes = before.changes(&after);
        assert_eq!(changes.images, [d.join("A.RAF"), d.join("B.jpg")]);
        assert_eq!(changes.sidecars, [d.join("A.RAF.xmp")]);
        std::fs::remove_file(d.join("B.jpg")).unwrap();
        let removed = after.changes(&Snapshot::of(d).unwrap());
        assert_eq!(removed.images, [d.join("B.jpg")]);
        assert!(removed.sidecars.is_empty());
        assert!(after.changes(&after).is_empty());
    }

    #[test]
    fn a_change_is_reported_once_the_folder_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_path_buf();
        let (tx, rx) = mpsc::channel();
        let watch = FolderWatch::start(&d, Duration::from_millis(20), move |c| {
            let _ = tx.send(c);
        });
        std::fs::write(d.join("A.jpg"), b"jpeg").unwrap();
        let changes = rx.recv_timeout(Duration::from_secs(5)).expect("signalé");
        assert_eq!(changes.images, [d.join("A.jpg")]);
        // Changement fait par tri lui-même, puis resynchronisé : rien n'est signalé.
        std::fs::write(d.join("B.jpg"), b"jpeg").unwrap();
        watch.sync();
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        drop(watch);
    }
}
