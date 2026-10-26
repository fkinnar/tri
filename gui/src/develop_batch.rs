//! Rendus développés d'un dossier entier, en fond (issue n° 69) : chaque image qui a un
//! sidecar du logiciel de développement (darktable aujourd'hui) est rendue l'une après
//! l'autre et rangée dans le cache, comme `Maj+B` le fait pour une seule. Un rendu prend
//! plusieurs secondes et occupe toute la machine : un seul à la fois, interruptible.
//!
//! Le travail passe par le producteur du moteur ([`CachedPreviews`], niveau
//! [`Level::Developed`]) : un autre logiciel de rendu branché là (RawTherapee, ART…)
//! servira ici sans rien changer.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use tri::cache::Level;
use tri::media::MediaFile;
use tri::scheduler::{CachedPreviews, Failure, Producer};

/// Bilan d'un lot : rendus faits, déjà en cache, échecs, interrompu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub rendered: usize,
    pub cached: usize,
    pub failed: usize,
    pub cancelled: bool,
}

/// Lot de rendus en cours.
pub struct DevelopBatch {
    /// Dossier dont les images sont rendues.
    pub folder: PathBuf,
    done: Arc<AtomicUsize>,
    /// Images à rendre (celles qui ont un sidecar), connues une fois les sidecars cherchés.
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Summary>>,
}

impl DevelopBatch {
    /// Lance le rendu de `files` (dossier `folder`) dans un fil à part ; `repaint` est
    /// appelé après chaque image (progression à l'écran).
    pub fn start(
        folder: PathBuf,
        files: Vec<MediaFile>,
        producer: CachedPreviews,
        force: Option<tri::develop::Engine>,
        repaint: impl Fn() + Send + 'static,
    ) -> DevelopBatch {
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (d, t, c) = (done.clone(), total.clone(), cancel.clone());
        let handle = std::thread::spawn(move || {
            // Seulement les images développées, chacune dans le logiciel de son
            // développement ; sans développement, elle garde le rendu LibRaw.
            let todo: Vec<(MediaFile, tri::develop::Engine)> = files
                .into_iter()
                .filter_map(|f| {
                    let engine = match force {
                        Some(e) => e.sidecar(&f.path).map(|_| e)?,
                        None => producer.developers.resolve(&f.path)?,
                    };
                    Some((f, engine))
                })
                .collect();
            t.store(todo.len(), Ordering::Relaxed);
            repaint();
            let mut summary = Summary::default();
            for (file, engine) in &todo {
                let level = Level::Developed(*engine);
                if c.load(Ordering::Relaxed) {
                    summary.cancelled = true;
                    break;
                }
                if producer.is_done(file, level) {
                    summary.cached += 1;
                } else {
                    match producer.produce(file, level, &c) {
                        Ok(()) => summary.rendered += 1,
                        Err(Failure::Cancelled) => {
                            summary.cancelled = true;
                            break;
                        }
                        Err(Failure::Error(_)) => summary.failed += 1,
                    }
                }
                d.fetch_add(1, Ordering::Relaxed);
                repaint();
            }
            summary
        });
        DevelopBatch {
            folder,
            done,
            total,
            cancel,
            handle: Some(handle),
        }
    }

    /// Avancement : (faites, à faire).
    pub fn progress(&self) -> (usize, usize) {
        (
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Interrompt le lot (le rendu en cours est abandonné).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Bilan, une fois le lot fini (une seule fois) ; `None` tant qu'il tourne.
    pub fn finished(&mut self) -> Option<Summary> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        self.handle.take()?.join().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Logiciel de développement dont le rendu échoue toujours.
    #[derive(Debug)]
    struct Failing;

    impl tri::develop::Developer for Failing {
        fn engine(&self) -> tri::develop::Engine {
            tri::develop::Engine::Darktable
        }

        fn render(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::path::Path,
            _: u32,
            _: &AtomicBool,
        ) -> Result<(), tri::develop::DevelopError> {
            Err(tri::develop::DevelopError::Failed("essai".into()))
        }
    }

    use tri::media::MediaKind;

    #[test]
    fn only_developed_images_are_rendered_and_failures_are_counted() {
        // Deux RAW, un seul développé (sidecar darktable à historique) ; sans darktable,
        // son rendu échoue : compté, le lot va jusqu'au bout.
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str| MediaFile {
            path: dir.path().join(name),
            kind: MediaKind::Raw,
        };
        let (a, b) = (file("a.RAF"), file("b.RAF"));
        std::fs::write(&a.path, b"raw").unwrap();
        std::fs::write(&b.path, b"raw").unwrap();
        std::fs::write(
            dir.path().join("a.RAF.xmp"),
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:darktable="http://darktable.sf.net/" darktable:history_end="3"/></rdf:RDF></x:xmpmeta>"#,
        )
        .unwrap();
        // darktable « installé », mais son rendu échoue.
        let producer = CachedPreviews {
            cache: tri::cache::Cache::open(dir.path().join("cache")).unwrap(),
            settings: tri::preview::Settings::default(),
            developers: tri::develop::Developers::none().with(Arc::new(Failing)),
        };
        let mut batch =
            DevelopBatch::start(dir.path().to_path_buf(), vec![a, b], producer, None, || {});
        let summary = loop {
            if let Some(s) = batch.finished() {
                break s;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(batch.progress(), (1, 1), "une seule image développée");
        assert_eq!(
            summary,
            Summary {
                failed: 1,
                ..Summary::default()
            }
        );
    }

    #[test]
    #[ignore]
    fn a_real_folder_is_developed_with_darktable() {
        // `TRI_TEST_DEVELOP` : un dossier de RAW dont certains ont un sidecar darktable,
        // darktable installé. Les développés sont rendus, les autres passés.
        let dir = PathBuf::from(std::env::var("TRI_TEST_DEVELOP").unwrap());
        let files = tri::media::scan(&dir, 1).unwrap();
        let developed = files
            .iter()
            .filter(|f| tri::develop::darktable_sidecar(&f.path).is_some())
            .count();
        let cache = tempfile::tempdir().unwrap();
        let producer = CachedPreviews {
            cache: tri::cache::Cache::open(cache.path()).unwrap(),
            settings: tri::preview::Settings::default(),
            developers: tri::develop::Developers::find(
                &tri::develop::DevelopSettings::default(),
                &cache.path().join("develop"),
                false,
            ),
        };
        let mut batch = DevelopBatch::start(dir, files, producer, None, || {});
        let summary = loop {
            if let Some(s) = batch.finished() {
                break s;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert_eq!(batch.progress(), (developed, developed));
        assert_eq!(summary.rendered, developed, "{summary:?}");
        assert_eq!(summary.failed, 0);
    }
}
