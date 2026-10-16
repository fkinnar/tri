//! Chargement des grandes images (aperçus, 1:1) dans un thread à part : décoder un aperçu
//! prend ~10 ms et un 1:1 de 26 Mpx plus de 150 ms, de quoi saccader l'interface si c'était
//! fait au moment de dessiner.
//!
//! L'interface déclare ce qu'elle veut (image affichée d'abord, puis ses voisines) ; la
//! liste remplace la précédente, donc une image dépassée n'est jamais chargée pour rien.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use eframe::egui::{self, ColorImage};
use tri::cache::{Cache, Level};
use tri::media::{MediaFile, MediaKind};
use tri::preview::{self, Sizes};

/// Une image chargée (ou absente du cache).
pub struct Loaded {
    /// Fichier source.
    pub path: PathBuf,
    /// Niveau chargé.
    pub level: Level,
    /// Pixels, prêts pour une texture ; `None` si l'entrée n'est pas (plus) dans le cache.
    pub image: Option<ColorImage>,
    /// Dimensions d'origine (pixels), même si l'image a dû être réduite pour tenir dans une
    /// texture.
    pub original: [usize; 2],
    /// Pour le 1:1 du JPEG boîtier : part du capteur qu'il représente (côté long du JPEG /
    /// côté long du rendu complet), pour un affichage honnête.
    pub sensor_ratio: Option<f32>,
}

#[derive(Default)]
struct State {
    wanted: Vec<(MediaFile, Level)>,
    shutdown: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

/// Thread de chargement. S'arrête à la destruction.
pub struct Loader {
    shared: Arc<Shared>,
    results: Receiver<Loaded>,
    thread: Option<JoinHandle<()>>,
}

impl Loader {
    /// Démarre le thread. `max_side` : plus grand côté accepté par le GPU pour une texture.
    pub fn new(cache: Cache, sizes: Sizes, max_side: usize, ctx: egui::Context) -> Loader {
        let shared = Arc::new(Shared {
            state: Mutex::default(),
            wake: Condvar::new(),
        });
        let (tx, results) = mpsc::channel();
        let worker = shared.clone();
        let thread = std::thread::Builder::new()
            .name("tri-chargement".into())
            .spawn(move || run(&worker, &cache, &sizes, max_side, &tx, &ctx))
            .expect("création du thread de chargement");
        Loader {
            shared,
            results,
            thread: Some(thread),
        }
    }

    /// Remplace la liste des images voulues, par ordre de priorité.
    pub fn want(&self, wanted: Vec<(MediaFile, Level)>) {
        let mut s = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        s.wanted = wanted;
        drop(s);
        self.shared.wake.notify_one();
    }

    /// Images chargées depuis le dernier appel.
    pub fn drain(&self) -> Vec<Loaded> {
        self.results.try_iter().collect()
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shutdown = true;
        self.shared.wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    shared: &Shared,
    cache: &Cache,
    sizes: &Sizes,
    max_side: usize,
    tx: &Sender<Loaded>,
    ctx: &egui::Context,
) {
    loop {
        let (file, level) = {
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if s.shutdown {
                    return;
                }
                if !s.wanted.is_empty() {
                    break s.wanted.remove(0);
                }
                s = shared.wake.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };
        let bitmap = preview::key(&file, level, sizes)
            .ok()
            .and_then(|k| cache.get(&k));
        let original = bitmap
            .as_ref()
            .map_or([0, 0], |b| [b.width as usize, b.height as usize]);
        let image = bitmap.map(|b| {
            // Au-delà de la taille de texture maximale du GPU, on réduit (l'affichage
            // l'étire à sa taille d'origine).
            let b = b.fit(max_side as u32);
            ColorImage::from_rgb([b.width as usize, b.height as usize], &b.data)
        });
        let sensor_ratio = (level == Level::EmbeddedFull && file.kind == MediaKind::Raw)
            .then(|| tri::raw::RawFile::open(&file.path).ok())
            .flatten()
            .map(|raw| raw.size())
            .filter(|&(w, h)| w.max(h) > 0)
            .map(|(w, h)| original[0].max(original[1]) as f32 / w.max(h) as f32);
        let loaded = Loaded {
            path: file.path,
            level,
            image,
            original,
            sensor_ratio,
        };
        if tx.send(loaded).is_err() {
            return; // l'interface est partie
        }
        ctx.request_repaint();
    }
}
