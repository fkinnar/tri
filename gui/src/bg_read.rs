//! Lectures du cache d'analyse hors du fil de l'interface (empreintes de similarité,
//! scores de netteté, parts saturées / bouchées des badges).
//!
//! Retour du 19/11/2026 (trace d'un dossier gardant son cache dans iCloud, sous Linux) :
//! des images de l'interface de 1 à 5 s, toutes dans l'étape « événements » ; chaque
//! empreinte annoncée par le moteur était relue dans le cache sur le fil de l'interface,
//! et une lecture sur iCloud peut prendre une seconde. Elles se font désormais dans un fil
//! par sorte de lecture, et l'interface prend les résultats à chaque image ; de même,
//! quelques écritures (vue du dossier, piles retenues) passent par un fil d'écriture
//! ([`BgWrites`]), dans l'ordre.

use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

use eframe::egui;

type Job<K, T> = (K, Box<dyn FnOnce() -> Option<T> + Send>);

/// File des lectures demandées, partagée avec le fil ; `closed` quand l'interface lâche
/// ses demandes.
struct Queue<K, T> {
    jobs: Mutex<(VecDeque<Job<K, T>>, bool)>,
    ready: Condvar,
}

/// Un fil qui fait des lectures demandées par clé `K` et rend leurs résultats `T`.
pub struct BgReads<K, T> {
    queue: Arc<Queue<K, T>>,
    results: Receiver<(K, Option<T>)>,
    pending: HashSet<K>,
}

impl<K, T> BgReads<K, T>
where
    K: Clone + Eq + Hash + Send + 'static,
    T: Send + 'static,
{
    /// Démarre le fil (`name` : son nom) ; `repaint` réveille l'interface à chaque
    /// résultat. Lectures dans l'ordre des demandes.
    pub fn new(name: &str, repaint: Option<egui::Context>) -> BgReads<K, T> {
        Self::start(name, repaint, false)
    }

    /// Comme [`BgReads::new`], la dernière demande lue d'abord : pour ce qui suit l'image
    /// sélectionnée (retour du 21/11/2026 : en avançant vite sur un disque lent, l'image
    /// où l'on s'arrête attendait derrière la lecture de toutes celles traversées).
    pub fn newest_first(name: &str, repaint: Option<egui::Context>) -> BgReads<K, T> {
        Self::start(name, repaint, true)
    }

    fn start(name: &str, repaint: Option<egui::Context>, newest_first: bool) -> BgReads<K, T> {
        let queue = Arc::new(Queue {
            jobs: Mutex::new((VecDeque::new(), false)),
            ready: Condvar::new(),
        });
        let (outbox, results) = mpsc::channel();
        let shared = queue.clone();
        let _ = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                loop {
                    let job = {
                        let mut jobs = shared.jobs.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            let next = if newest_first {
                                jobs.0.pop_back()
                            } else {
                                jobs.0.pop_front()
                            };
                            if let Some(job) = next {
                                break job;
                            }
                            // S'arrête quand l'interface lâche ses demandes (autre dossier,
                            // fermeture).
                            if jobs.1 {
                                return;
                            }
                            jobs = shared.ready.wait(jobs).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let (key, read) = job;
                    if outbox.send((key, read())).is_err() {
                        return;
                    }
                    if let Some(ctx) = &repaint {
                        ctx.request_repaint();
                    }
                }
            });
        BgReads {
            queue,
            results,
            pending: HashSet::new(),
        }
    }

    /// Demande la lecture `read` pour `key`, sauf si elle est déjà demandée.
    pub fn request(&mut self, key: K, read: impl FnOnce() -> Option<T> + Send + 'static) {
        if self.pending.insert(key.clone()) {
            let mut jobs = self.queue.jobs.lock().unwrap_or_else(|e| e.into_inner());
            jobs.0.push_back((key, Box::new(read)));
            drop(jobs);
            self.queue.ready.notify_one();
        }
    }

    /// Vrai si `key` est demandée et pas encore arrivée.
    pub fn is_pending(&self, key: &K) -> bool {
        self.pending.contains(key)
    }

    /// Vrai si aucune lecture n'attend.
    pub fn idle(&self) -> bool {
        self.pending.is_empty()
    }

    /// Résultats arrivés depuis le dernier appel (`None` : rien dans le cache).
    pub fn take(&mut self) -> Vec<(K, Option<T>)> {
        let arrived: Vec<_> = self.results.try_iter().collect();
        for (key, _) in &arrived {
            self.pending.remove(key);
        }
        arrived
    }
}

impl<K, T> Drop for BgReads<K, T> {
    fn drop(&mut self) {
        let mut jobs = self.queue.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.0.clear();
        jobs.1 = true;
        drop(jobs);
        self.queue.ready.notify_all();
    }
}

/// Un fil qui fait des écritures dans l'ordre où elles sont demandées (toutes, sans en
/// fusionner : la dernière écrite est la dernière demandée). Ses copies partagent le même
/// fil (un autre fil peut attendre ses écritures, [`BgWrites::flush`]).
#[derive(Clone)]
pub struct BgWrites {
    jobs: Sender<Box<dyn FnOnce() + Send>>,
    pending: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl BgWrites {
    /// Démarre le fil (`name` : son nom).
    pub fn new(name: &str) -> BgWrites {
        let (jobs, inbox) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let pending = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let left = pending.clone();
        let _ = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                for job in inbox {
                    job();
                    left.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            });
        BgWrites { jobs, pending }
    }

    /// Demande l'écriture `write`, faite après celles déjà demandées.
    pub fn run(&self, write: impl FnOnce() + Send + 'static) {
        self.pending
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.jobs.send(Box::new(write)).is_err() {
            self.pending
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Attend que les écritures demandées soient faites, au plus `timeout` (fermeture de
    /// `tri`, tests) ; vrai si elles le sont.
    pub fn flush(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while self.pending.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn reads_happen_in_the_background_once_per_key() {
        let mut reads: BgReads<u32, String> = BgReads::new("test", None);
        let (go, wait) = mpsc::channel::<()>();
        reads.request(1, move || {
            let _ = wait.recv();
            Some("un".into())
        });
        // Déjà demandée : pas une deuxième fois.
        reads.request(1, || panic!("lue deux fois"));
        reads.request(2, || None);
        assert!(reads.is_pending(&1));
        assert!(reads.take().is_empty(), "la première lecture attend");
        go.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < 2 && Instant::now() < deadline {
            got.extend(reads.take());
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(got, vec![(1, Some("un".to_string())), (2, None)]);
        assert!(!reads.is_pending(&1));
    }

    #[test]
    fn newest_first_reads_the_last_request_first() {
        // Retour du 21/11/2026 : en avançant vite, l'image où l'on s'arrête passe avant
        // celles traversées.
        let mut reads: BgReads<u32, u32> = BgReads::newest_first("test", None);
        let (go, wait) = mpsc::channel::<()>();
        let (started, running) = mpsc::channel::<()>();
        // La première occupe le fil le temps que les autres arrivent.
        reads.request(0, move || {
            let _ = started.send(());
            let _ = wait.recv();
            Some(0)
        });
        running.recv().unwrap();
        for k in 1..=5 {
            reads.request(k, move || Some(k));
        }
        go.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got = Vec::new();
        while got.len() < 6 && Instant::now() < deadline {
            got.extend(reads.take().into_iter().map(|(k, _)| k));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(got, vec![0, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn writes_happen_in_order() {
        let writes = BgWrites::new("test");
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        for i in 0..20 {
            let log = log.clone();
            writes.run(move || log.lock().unwrap().push(i));
        }
        assert!(writes.flush(Duration::from_secs(5)));
        assert_eq!(*log.lock().unwrap(), (0..20).collect::<Vec<_>>());
    }
}
