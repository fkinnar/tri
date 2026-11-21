//! Calculs choisis sur un dossier, en fond (#555) : chacun une tâche, avec sa ligne dans
//! la barre d'état, interruptible, que le dossier soit ouvert ou non. Le travail est celui
//! du moteur ([`tri::compute`]) : mêmes clés de cache que l'interface, ce qui y est déjà
//! est sauté (sauf « recréer »).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use tri::compute::{Kind, Summary};
use tri::scheduler::CachedPreviews;

/// Un calcul en cours sur un dossier.
pub struct ComputeTask {
    /// Numéro de la tâche (bouton d'arrêt de la barre d'état).
    pub id: u64,
    /// Dossier calculé.
    pub folder: PathBuf,
    /// Ce qui est calculé.
    pub kind: Kind,
    done: Arc<AtomicUsize>,
    /// Images concernées, connues une fois le dossier parcouru (0 avant).
    total: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Summary, String>>>,
}

impl ComputeTask {
    /// Lance `kind` sur les images de `folder` (le dossier seul, sans ses sous-dossiers)
    /// dans un fil à part ; `recreate` : efface d'abord du cache ce que le calcul remplit.
    /// `repaint` après chaque image.
    pub fn start(
        id: u64,
        folder: PathBuf,
        kind: Kind,
        recreate: bool,
        producer: CachedPreviews,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> ComputeTask {
        let done = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (d, t, c, dir) = (done.clone(), total.clone(), cancel.clone(), folder.clone());
        let handle = std::thread::spawn(move || {
            if recreate {
                tri::compute::clear(&producer.cache, &dir, kind).map_err(|e| e.to_string())?;
            }
            let files = tri::media::scan(&dir, 1).map_err(|e| e.to_string())?;
            let step = |n: usize, of: usize| {
                d.store(n, Ordering::Relaxed);
                t.store(of, Ordering::Relaxed);
                repaint();
            };
            Ok(tri::compute::run(&files, kind, &producer, &c, &step))
        });
        ComputeTask {
            id,
            folder,
            kind,
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

    /// Interrompt la tâche (le calcul en cours est abandonné s'il le permet).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Bilan, une fois la tâche finie (une seule fois) ; `None` tant qu'elle tourne.
    pub fn finished(&mut self) -> Option<Result<Summary, String>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        self.handle
            .take()?
            .join()
            .unwrap_or_else(|_| Err("panic".into()))
            .into()
    }
}

/// Nom d'un calcul à l'écran (barre d'état, bilans).
pub fn kind_label(kind: Kind) -> String {
    use tri::palette::SourceKind;
    match kind {
        Kind::Previews => t!("compute-previews"),
        Kind::Raw => t!("compute-raw"),
        Kind::Exposure => t!("compute-exposure"),
        Kind::Sharpness => t!("compute-sharpness"),
        Kind::Signatures => t!("compute-signatures"),
        Kind::Palette(SourceKind::Camera | SourceKind::Cover) => t!("compute-palette-camera"),
        Kind::Palette(SourceKind::Raw) => t!("compute-palette-raw"),
        Kind::Palette(SourceKind::Developed(engine)) => {
            t!("compute-palette-developed", engine = engine.name())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_computes_the_folder_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        for name in ["a.jpg", "b.jpg"] {
            image::RgbImage::from_pixel(32, 24, image::Rgb([40, 90, 200]))
                .save(photos.join(name))
                .unwrap();
        }
        let cache = tri::cache::Cache::open(dir.path().join("cache")).unwrap();
        let producer = || CachedPreviews {
            cache: cache.clone(),
            settings: tri::preview::Settings::default(),
            developers: tri::develop::Developers::none().into(),
        };
        let run = |recreate| {
            let mut task = ComputeTask::start(
                1,
                photos.clone(),
                Kind::Previews,
                recreate,
                producer(),
                || {},
            );
            loop {
                if let Some(result) = task.finished() {
                    assert_eq!(task.progress(), (2, 2));
                    break result.unwrap();
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        assert_eq!(run(false).computed, 2);
        assert_eq!(run(false).cached, 2, "déjà en cache : sautées");
        assert_eq!(run(true).computed, 2, "recréées");
    }
}
