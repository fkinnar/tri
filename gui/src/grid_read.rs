//! Lecture des vignettes de la grille dans le cache, hors du fil de l'interface.
//!
//! Retour du 19/11/2026 : un dossier gardant son cache dans iCloud (monté sous Linux), la
//! grille lisait chaque vignette sur le fil de l'interface, et une lecture lente figeait
//! toute la fenêtre (un panneau élargi par accident ne se laissait plus remettre en
//! place). Le budget par image (`[gui] load_budget_ms`) ne protège que *entre* deux
//! lectures, pas pendant une lecture qui traîne.
//!
//! Les lectures se font donc dans quelques fils ([`GridReader::new`]) ; l'interface les
//! demande, attend leur résultat au plus jusqu'à la fin de son budget (sur un disque
//! local, comme avant : la vignette arrive dans la même image), puis reprend à l'image
//! suivante, réveillée par le fil quand une lecture se termine.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use eframe::egui;
use tri::bitmap::Bitmap;
use tri::cache::{Cache, Key, Level};

/// Une vignette demandée : son fichier et le niveau montré.
pub type Wanted = (PathBuf, Level);

/// Ce qu'il faut lire pour une vignette : l'entrée elle-même (`key`), ou pour un rendu
/// RAW ou développé sa copie à la taille des vignettes (`small`), faite et gardée au
/// besoin à partir de l'entrée, réduite à `side`.
pub struct Request {
    pub wanted: Wanted,
    pub key: Key,
    pub small: Option<Key>,
    pub side: u32,
}

#[derive(Default)]
struct Queue {
    /// Demandes en attente, la plus récente servie d'abord (les vignettes visibles en
    /// dernier, après un défilement).
    requests: Vec<Request>,
    stop: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}

/// Fils de lecture des vignettes ([module](self)).
pub struct GridReader {
    shared: Arc<Shared>,
    results: Receiver<(Wanted, Option<Bitmap>)>,
    /// Demandées, pas encore arrivées.
    pending: HashSet<Wanted>,
    /// Arrivées, pas encore prises par la grille.
    done: HashMap<Wanted, Option<Bitmap>>,
    /// Au plus autant de résultats gardés ([`GridReader::keep`]).
    keep_max: usize,
    threads: Vec<JoinHandle<()>>,
}

impl GridReader {
    /// Démarre `threads` fils (au moins un) qui lisent dans `cache` ; `repaint` réveille
    /// l'interface à chaque lecture terminée ; `keep_max` : résultats gardés au plus en
    /// attendant la grille.
    pub fn new(
        cache: Cache,
        threads: usize,
        keep_max: usize,
        repaint: Option<egui::Context>,
    ) -> GridReader {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        });
        let (tx, results) = mpsc::channel();
        let threads = (0..threads.max(1))
            .map(|_| {
                let (shared, tx, cache, repaint) =
                    (shared.clone(), tx.clone(), cache.clone(), repaint.clone());
                std::thread::Builder::new()
                    .name("tri-grid-read".into())
                    .spawn(move || work(&shared, &tx, &cache, repaint.as_ref()))
                    .expect("fil de lecture des vignettes")
            })
            .collect();
        GridReader {
            shared,
            results,
            pending: HashSet::new(),
            done: HashMap::new(),
            keep_max: keep_max.max(1),
            threads,
        }
    }

    /// Demande `request`, sauf si elle l'est déjà ou si son résultat attend.
    pub fn request(&mut self, request: Request) {
        if self.pending.contains(&request.wanted) || self.done.contains_key(&request.wanted) {
            return;
        }
        self.pending.insert(request.wanted.clone());
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.requests.push(request);
        drop(queue);
        self.shared.wake.notify_one();
    }

    /// Résultat de `wanted` : `Some(Some(_))` lue, `Some(None)` absente ou illisible,
    /// `None` pas encore là. Attend au plus jusqu'à `deadline` s'il a été demandé.
    pub fn take(&mut self, wanted: &Wanted, deadline: Instant) -> Option<Option<Bitmap>> {
        loop {
            self.collect();
            if let Some(result) = self.done.remove(wanted) {
                return Some(result);
            }
            if !self.pending.contains(wanted) {
                return None;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            match self.results.recv_timeout(left) {
                Ok((w, result)) => {
                    self.pending.remove(&w);
                    self.keep(w, result);
                }
                Err(_) => return None,
            }
        }
    }

    /// Vrai si `wanted` est demandée et pas encore arrivée.
    pub fn is_pending(&self, wanted: &Wanted) -> bool {
        self.pending.contains(wanted)
    }

    /// Garde un résultat jusqu'à ce que la grille le prenne. Ceux d'images qu'on a
    /// dépassées en défilant ne sont jamais pris : au-delà de `keep_max`, tous jetés
    /// (redemandés au besoin).
    fn keep(&mut self, wanted: Wanted, result: Option<Bitmap>) {
        if self.done.len() >= self.keep_max {
            self.done.clear();
        }
        self.done.insert(wanted, result);
    }

    /// Range les résultats arrivés.
    fn collect(&mut self) {
        while let Ok((wanted, result)) = self.results.try_recv() {
            // Demande abandonnée entre-temps (`clear`) : ignorée.
            if self.pending.remove(&wanted) {
                self.keep(wanted, result);
            }
        }
    }
}

impl Drop for GridReader {
    fn drop(&mut self) {
        self.shared
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stop = true;
        self.shared.wake.notify_all();
        // Pas d'attente des fils : une lecture lente (iCloud) ne doit pas retenir la
        // fermeture ; ils s'arrêtent d'eux-mêmes après leur lecture.
        self.threads.clear();
    }
}

fn work(
    shared: &Shared,
    tx: &Sender<(Wanted, Option<Bitmap>)>,
    cache: &Cache,
    repaint: Option<&egui::Context>,
) {
    loop {
        let request = {
            let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if queue.stop {
                    return;
                }
                if let Some(request) = queue.requests.pop() {
                    break request;
                }
                queue = shared.wake.wait(queue).unwrap_or_else(|e| e.into_inner());
            }
        };
        let bitmap = read(cache, &request, |small, bitmap| {
            let _ = cache.put(small, bitmap);
        });
        if tx.send((request.wanted, bitmap)).is_err() {
            return;
        }
        if let Some(ctx) = repaint {
            ctx.request_repaint();
        }
    }
}

/// Lit la vignette demandée : la vignette elle-même, ou la copie à la taille des
/// vignettes d'un rendu plus grand, faite la première fois et confiée à `keep` pour être
/// gardée (11/11/2026 : au lancement, chaque vignette relisait un aperçu de 2560 px).
pub fn read(cache: &Cache, request: &Request, keep: impl FnOnce(&Key, &Bitmap)) -> Option<Bitmap> {
    if request.wanted.1 == Level::Thumbnail {
        return cache.get(&request.key);
    }
    if let Some(small) = &request.small
        && let Some(bitmap) = cache.get(small)
    {
        return Some(bitmap);
    }
    // Dans l'espace de travail (un rendu développé y est converti par le cache).
    let bitmap = cache.get_fit(&request.key, request.side)?.fit(request.side);
    if let Some(small) = &request.small {
        keep(small, &bitmap);
    }
    Some(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_thumbnail_is_read_in_the_background_and_taken_once() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let photo = dir.path().join("a.JPG");
        std::fs::write(&photo, b"jpeg").unwrap();
        let key = Key::new(&photo, Level::Thumbnail, "p").unwrap();
        let bitmap = Bitmap {
            width: 2,
            height: 1,
            data: vec![255; 6],
        };
        cache.put(&key, &bitmap).unwrap();
        let mut reader = GridReader::new(cache, 2, 16, None);
        let wanted = (photo.clone(), Level::Thumbnail);
        assert_eq!(reader.take(&wanted, Instant::now()), None, "not asked");
        reader.request(Request {
            wanted: wanted.clone(),
            key: key.clone(),
            small: None,
            side: 400,
        });
        assert!(reader.is_pending(&wanted));
        let deadline = Instant::now() + Duration::from_secs(5);
        let got = reader.take(&wanted, deadline).expect("arrived");
        assert_eq!(got.map(|b| (b.width, b.height)), Some((2, 1)));
        assert_eq!(reader.take(&wanted, Instant::now()), None, "taken");
        // Absente : arrive comme telle.
        let other = Key::new(&photo, Level::Thumbnail, "q").unwrap();
        let wanted = (dir.path().join("b.JPG"), Level::Thumbnail);
        reader.request(Request {
            wanted: wanted.clone(),
            key: other,
            small: None,
            side: 400,
        });
        assert_eq!(reader.take(&wanted, deadline), Some(None));
    }
}
