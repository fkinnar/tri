//! File de travail en tâche de fond : remplit le cache pour tout un dossier, en servant
//! d'abord ce que l'utilisateur regarde.
//!
//! - un worker *lourd*, unique (rendus LibRaw : déjà multithreadés en interne) ;
//! - des workers *légers* en parallèle (vignettes, images plates) ;
//! - des priorités recalculées à chaque prise de tâche : image affichée, puis voisines,
//!   puis le reste du dossier par distance ;
//! - une progression par niveau, et des événements envoyés à un callback.
//!
//! Le calcul lui-même est délégué à un [`Producer`] : [`CachedPreviews`] en vrai, un faux
//! producteur dans les tests. Voir `doc/scheduler.md`.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::cache::{Cache, Level};
use crate::media::{MediaFile, MediaKind};
use crate::preview::{self, Outcome, PreviewError, Sizes};

/// Calcule et stocke les images. Implémenté par [`CachedPreviews`].
pub trait Producer: Send + Sync + 'static {
    /// Vrai si l'image est déjà disponible (rien à faire).
    fn is_done(&self, file: &MediaFile, level: Level) -> bool;
    /// Calcule l'image et la stocke. Doit renvoyer `Err(Failure::Cancelled)` au plus vite
    /// une fois `cancel` levé.
    fn produce(&self, file: &MediaFile, level: Level, cancel: &AtomicBool) -> Result<(), Failure>;
}

/// Échec d'une tâche.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Tâche annulée (changement de dossier, d'image affichée...).
    Cancelled,
    /// Erreur, avec son message.
    Error(String),
}

/// Le vrai producteur : [`preview::ensure`] dans un [`Cache`].
pub struct CachedPreviews {
    /// Cache à remplir.
    pub cache: Cache,
    /// Tailles des aperçus.
    pub sizes: Sizes,
}

impl Producer for CachedPreviews {
    fn is_done(&self, file: &MediaFile, level: Level) -> bool {
        preview::key(file, level, &self.sizes).is_ok_and(|k| self.cache.contains(&k))
    }

    fn produce(&self, file: &MediaFile, level: Level, cancel: &AtomicBool) -> Result<(), Failure> {
        match preview::ensure(&self.cache, file, level, &self.sizes, cancel) {
            Ok(Outcome::Cached | Outcome::Computed) => Ok(()),
            Err(PreviewError::Cancelled) => Err(Failure::Cancelled),
            Err(e) => Err(Failure::Error(e.to_string())),
        }
    }
}

/// Événement envoyé au callback de l'interface. `generation` identifie le dossier
/// (renvoyé par [`Scheduler::set_folder`]) ; `index` est la position dans sa liste.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// L'image est disponible (dans le cache).
    Ready {
        /// Dossier concerné.
        generation: u64,
        /// Position du fichier.
        index: usize,
        /// Chemin du fichier.
        path: PathBuf,
        /// Niveau disponible.
        level: Level,
    },
    /// Le calcul a échoué.
    Failed {
        /// Dossier concerné.
        generation: u64,
        /// Position du fichier.
        index: usize,
        /// Chemin du fichier.
        path: PathBuf,
        /// Niveau concerné.
        level: Level,
        /// Message d'erreur.
        error: String,
    },
    /// Avancement d'un niveau calculé en fond (vignettes ou aperçus standard).
    Progress {
        /// Dossier concerné.
        generation: u64,
        /// Niveau.
        level: Level,
        /// Avancement.
        progress: Progress,
    },
}

/// Avancement d'un niveau : tâches terminées (réussies, déjà en cache ou en échec) sur total.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Terminées.
    pub done: usize,
    /// Total.
    pub total: usize,
}

impl Progress {
    /// Fraction terminée, entre 0 et 1 (1 si rien à faire).
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

/// Réglages du scheduler.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Nombre de workers légers (vignettes, images plates). Le worker lourd est toujours
    /// unique.
    pub light_workers: usize,
    /// Ordre « le plus rapide d'abord » pour le travail de fond : vignettes, puis aperçus
    /// boîtier, puis rendus RAW. Les aperçus boîtier de fond attendent la fin des vignettes,
    /// les aperçus standard de fond attendent la fin des deux (l'image affichée et ses
    /// voisines passent toujours). Mesures dans `doc/scheduler.md`.
    pub thumbnails_first: bool,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Self {
            light_workers: (cores / 2).max(1),
            thumbnails_first: true,
        }
    }
}

/// Niveaux dont l'avancement est suivi (calculés en fond) ; `Full` ne l'est qu'à la demande.
const TRACKED: [Level; 3] = [Level::Thumbnail, Level::Embedded, Level::Standard];

/// Niveaux calculés en fond pour un fichier : l'aperçu boîtier n'existe que pour les RAW.
fn background_levels(kind: MediaKind) -> &'static [Level] {
    match kind {
        MediaKind::Raw => &TRACKED,
        MediaKind::Flat => &[Level::Thumbnail, Level::Standard],
    }
}

/// Une tâche : un fichier (par sa position) et un niveau.
type Job = (usize, Level);

/// Vrai si la tâche passe par le worker lourd (rendu LibRaw).
fn is_heavy(kind: MediaKind, level: Level) -> bool {
    kind == MediaKind::Raw && matches!(level, Level::Standard | Level::Full)
}

/// Classe de priorité des aperçus boîtier de fond (ni l'image affichée, ni ses voisines).
const EMBEDDED_BACKGROUND: u8 = 4;
/// Classe de priorité des aperçus standard de fond.
const STANDARD_BACKGROUND: u8 = 5;

/// Priorité d'une tâche (la plus petite passe en premier). Ordre :
/// 1. le 1:1 demandé ;
/// 2. les aperçus boîtier de l'image affichée, de la suivante, de la précédente
///    (presque instantanés) ;
/// 3. leurs aperçus standard, dans le même ordre ;
/// 4. les vignettes, dans l'ordre du dossier ;
/// 5. les autres aperçus boîtier, puis les autres aperçus standard, par distance à l'image
///    affichée (en avant d'abord à distance égale).
fn priority((index, level): Job, focus: Option<usize>) -> (u8, usize) {
    let (near_class, background_class) = match level {
        Level::Full => return (0, 0),
        Level::Thumbnail => return (3, index),
        Level::Embedded => (1, EMBEDDED_BACKGROUND),
        Level::Standard => (2, STANDARD_BACKGROUND),
    };
    let Some(f) = focus else {
        return (background_class, index);
    };
    if index == f {
        (near_class, 0)
    } else if index == f + 1 {
        (near_class, 1)
    } else if index + 1 == f {
        (near_class, 2)
    } else {
        (
            background_class,
            index.abs_diff(f) * 2 + usize::from(index < f),
        )
    }
}

struct Running {
    job: Job,
    generation: u64,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    thumbnails_first: bool,
    generation: u64,
    files: Vec<MediaFile>,
    focus: Option<usize>,
    pending: HashSet<Job>,
    running: Vec<Running>,
    progress: [Progress; 3],
    paused: bool,
    shutdown: bool,
    /// Événements en attente de livraison, dans l'ordre où ils se sont produits.
    outbox: VecDeque<Event>,
    /// Vrai pendant un appel au callback.
    in_callback: bool,
}

impl State {
    /// Prend la tâche la plus prioritaire pour ce type de worker.
    fn take(&mut self, heavy: bool) -> Option<(Job, MediaFile, Arc<AtomicBool>)> {
        if self.paused {
            return None;
        }
        let focus = self.focus;
        // Travail de fond du plus rapide au plus lent : la grille d'abord, puis les aperçus
        // boîtier ; pas de rendu LibRaw de fond (qui occupe tous les cœurs) avant.
        let left = |level: Level| {
            self.thumbnails_first
                && (self.pending.iter().any(|j| j.1 == level)
                    || self.running.iter().any(|r| r.job.1 == level))
        };
        let (thumbnails_left, embedded_left) = (left(Level::Thumbnail), left(Level::Embedded));
        let blocked = |class: u8| match class {
            EMBEDDED_BACKGROUND => thumbnails_left,
            STANDARD_BACKGROUND => thumbnails_left || embedded_left,
            _ => false,
        };
        let job = *self
            .pending
            .iter()
            .filter(|(i, l)| is_heavy(self.files[*i].kind, *l) == heavy)
            .filter(|job| !blocked(priority(**job, focus).0))
            .min_by_key(|job| (priority(**job, focus), job.0))?;
        self.pending.remove(&job);
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.push(Running {
            job,
            generation: self.generation,
            cancel: cancel.clone(),
        });
        Some((job, self.files[job.0].clone(), cancel))
    }

    fn cancel_running(&self, keep: impl Fn(&Running) -> bool) {
        for r in self.running.iter().filter(|r| !keep(r)) {
            r.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn is_idle(&self) -> bool {
        self.running.is_empty()
            && self.outbox.is_empty()
            && !self.in_callback
            && (self.pending.is_empty() || self.paused)
    }
}

struct Shared {
    state: Mutex<State>,
    /// Réveille les workers (nouvelle tâche, reprise, arrêt).
    work: Condvar,
    /// Réveille ceux qui attendent la fin du travail ([`Scheduler::wait_idle`]).
    idle: Condvar,
    /// Un seul thread à la fois livre les événements : ordre préservé, callback jamais
    /// appelé en parallèle.
    delivery: Mutex<()>,
    on_event: Box<dyn Fn(Event) + Send + Sync>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Un callback qui panique ne doit pas bloquer tout le scheduler.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// File de travail en tâche de fond. Les threads s'arrêtent quand le `Scheduler` est
/// détruit (le travail en cours est annulé).
pub struct Scheduler {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl Scheduler {
    /// Démarre les workers. `on_event` est appelé depuis les threads de travail, jamais en
    /// tenant le verrou interne (il peut rappeler le scheduler), jamais deux fois en même
    /// temps, et dans l'ordre où les événements se sont produits : une progression ne
    /// recule jamais.
    pub fn new(
        producer: impl Producer,
        config: Config,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Scheduler {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                thumbnails_first: config.thumbnails_first,
                ..State::default()
            }),
            work: Condvar::new(),
            idle: Condvar::new(),
            delivery: Mutex::new(()),
            on_event: Box::new(on_event),
        });
        let producer = Arc::new(producer);
        let heavy = std::iter::once(true);
        let light = std::iter::repeat_n(false, config.light_workers.max(1));
        let threads = heavy
            .chain(light)
            .enumerate()
            .map(|(n, is_heavy)| {
                let (shared, producer) = (shared.clone(), producer.clone());
                let name = if is_heavy {
                    "tri-lourd".to_string()
                } else {
                    format!("tri-léger-{n}")
                };
                std::thread::Builder::new()
                    .name(name)
                    .spawn(move || worker(&shared, producer.as_ref(), is_heavy))
                    .expect("création d'un thread de travail")
            })
            .collect();
        Scheduler { shared, threads }
    }

    /// Remplace le dossier courant : annule tout le travail en cours, puis planifie vignettes
    /// et aperçus standard de tous les fichiers. Renvoie la génération du dossier, présente
    /// dans les événements. Les résultats du dossier précédent sont écartés et ses événements
    /// non livrés supprimés ; seul un événement en cours de livraison au moment de l'appel
    /// peut encore arriver : l'interface ignore ceux dont la génération n'est pas la sienne.
    pub fn set_folder(&self, files: Vec<MediaFile>) -> u64 {
        let mut s = self.shared.lock();
        s.cancel_running(|_| false);
        s.outbox.clear(); // événements de l'ancien dossier pas encore livrés
        s.generation += 1;
        s.focus = None;
        s.pending = (0..files.len())
            .flat_map(|i| {
                background_levels(files[i].kind)
                    .iter()
                    .map(move |&l| (i, l))
            })
            .collect();
        let total = |level: Level| {
            files
                .iter()
                .filter(|f| background_levels(f.kind).contains(&level))
                .count()
        };
        s.progress = TRACKED.map(|l| Progress {
            done: 0,
            total: total(l),
        });
        s.files = files;
        let generation = s.generation;
        drop(s);
        self.shared.work.notify_all();
        generation
    }

    /// Indique l'image affichée : son aperçu passe en tête, puis ses voisines. Un rendu 1:1
    /// demandé pour une autre image est abandonné (et annulé s'il a commencé).
    pub fn set_focus(&self, index: usize) {
        let mut s = self.shared.lock();
        if index >= s.files.len() || s.focus == Some(index) {
            return;
        }
        s.focus = Some(index);
        s.pending.retain(|&(i, l)| l != Level::Full || i == index);
        s.cancel_running(|r| r.job.1 != Level::Full || r.job.0 == index);
        drop(s);
        self.shared.work.notify_all();
    }

    /// Demande le rendu 1:1 de l'image affichée (sans effet sans image affichée).
    pub fn request_full(&self) {
        let mut s = self.shared.lock();
        let Some(focus) = s.focus else { return };
        let already = s.running.iter().any(|r| r.job == (focus, Level::Full));
        if !already {
            s.pending.insert((focus, Level::Full));
        }
        drop(s);
        self.shared.work.notify_all();
    }

    /// Met en pause (aucune nouvelle tâche n'est prise ; celles en cours se terminent) ou
    /// reprend le travail.
    pub fn set_paused(&self, paused: bool) {
        self.shared.lock().paused = paused;
        self.shared.work.notify_all();
        self.shared.idle.notify_all();
    }

    /// Avancement d'un niveau calculé en fond (`Thumbnail`, `Embedded`, `Standard`) ; `None`
    /// pour `Full`. Le total de `Embedded` est le nombre de RAW.
    pub fn progress(&self, level: Level) -> Option<Progress> {
        let i = TRACKED.iter().position(|&l| l == level)?;
        Some(self.shared.lock().progress[i])
    }

    /// Attend que plus rien ne soit en cours ni en attente (ou que le scheduler soit en
    /// pause et sans tâche en cours), **événements compris** : au retour, le callback a reçu
    /// tous les événements des tâches terminées. Renvoie `false` si `timeout` est écoulé
    /// avant.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut s = self.shared.lock();
        while !s.is_idle() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            s = self
                .shared
                .idle
                .wait_timeout(s, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        let mut s = self.shared.lock();
        s.shutdown = true;
        s.cancel_running(|_| false);
        drop(s);
        self.shared.work.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn worker(shared: &Shared, producer: &dyn Producer, heavy: bool) {
    loop {
        let (job, file, cancel) = {
            let mut s = shared.lock();
            loop {
                if s.shutdown {
                    return;
                }
                if let Some(taken) = s.take(heavy) {
                    break taken;
                }
                s = shared.work.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };

        let result = if producer.is_done(&file, job.1) {
            Ok(())
        } else {
            producer.produce(&file, job.1, &cancel)
        };

        {
            let mut s = shared.lock();
            let pos = s
                .running
                .iter()
                .position(|r| Arc::ptr_eq(&r.cancel, &cancel))
                .expect("tâche en cours introuvable");
            let generation = s.running.swap_remove(pos).generation;
            if generation == s.generation {
                let events = finish(&mut s, job, file, result);
                s.outbox.extend(events);
            } // sinon : dossier remplacé entre-temps, résultat ignoré
        }
        // Une tâche terminée peut débloquer les autres workers (fin des vignettes).
        shared.work.notify_all();
        deliver(shared);
    }
}

/// Livre les événements en attente, dans l'ordre, hors du verrou d'état (le callback peut
/// rappeler le scheduler). Un seul thread livre à la fois ; les autres attendent puis
/// trouvent la file vide.
fn deliver(shared: &Shared) {
    let _turn = shared.delivery.lock().unwrap_or_else(|e| e.into_inner());
    // Remet `in_callback` à faux même si le callback panique : wait_idle ne doit pas rester
    // bloqué.
    struct EndOfCallback<'a>(&'a Shared);
    impl Drop for EndOfCallback<'_> {
        fn drop(&mut self) {
            let mut s = self.0.lock();
            s.in_callback = false;
            if s.is_idle() {
                self.0.idle.notify_all();
            }
        }
    }
    loop {
        let event = {
            let mut s = shared.lock();
            let Some(event) = s.outbox.pop_front() else {
                if s.is_idle() {
                    shared.idle.notify_all();
                }
                return;
            };
            s.in_callback = true;
            event
        };
        let _end = EndOfCallback(shared);
        (shared.on_event)(event);
    }
}

/// Met à jour l'état après une tâche et prépare les événements à émettre.
fn finish(
    s: &mut State,
    (index, level): Job,
    file: MediaFile,
    result: Result<(), Failure>,
) -> Vec<Event> {
    let generation = s.generation;
    let path = file.path;
    let mut events = match result {
        Ok(()) => vec![Event::Ready {
            generation,
            index,
            path,
            level,
        }],
        // Annulée sans changement de dossier : un 1:1 abandonné, rien à signaler.
        Err(Failure::Cancelled) => return Vec::new(),
        Err(Failure::Error(error)) => vec![Event::Failed {
            generation,
            index,
            path,
            level,
            error,
        }],
    };
    if let Some(i) = TRACKED.iter().position(|&l| l == level) {
        s.progress[i].done += 1;
        events.push(Event::Progress {
            generation,
            level,
            progress: s.progress[i],
        });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order_around_focus() {
        let f = Some(5);
        let mut jobs = vec![
            (9, Level::Standard),
            (4, Level::Standard),
            (6, Level::Standard),
            (5, Level::Standard),
            (0, Level::Thumbnail),
            (3, Level::Standard),
            (7, Level::Standard),
            (5, Level::Full),
            (8, Level::Thumbnail),
            (9, Level::Embedded),
            (6, Level::Embedded),
            (5, Level::Embedded),
        ];
        jobs.sort_by_key(|j| (priority(*j, f), j.0));
        assert_eq!(
            jobs,
            [
                (5, Level::Full),
                (5, Level::Embedded),
                (6, Level::Embedded),
                (5, Level::Standard),
                (6, Level::Standard),
                (4, Level::Standard),
                (0, Level::Thumbnail),
                (8, Level::Thumbnail),
                (9, Level::Embedded),
                (7, Level::Standard),
                (3, Level::Standard),
                (9, Level::Standard),
            ]
        );
    }

    #[test]
    fn without_focus_standards_follow_folder_order() {
        assert!(priority((1, Level::Standard), None) < priority((2, Level::Standard), None));
        assert!(priority((9, Level::Thumbnail), None) < priority((0, Level::Standard), None));
    }

    #[test]
    fn heavy_jobs_are_raw_renders() {
        assert!(
            !is_heavy(MediaKind::Raw, Level::Embedded),
            "aperçu boîtier : léger"
        );
        assert!(is_heavy(MediaKind::Raw, Level::Standard));
        assert!(is_heavy(MediaKind::Raw, Level::Full));
        assert!(!is_heavy(MediaKind::Raw, Level::Thumbnail));
        assert!(!is_heavy(MediaKind::Flat, Level::Standard));
        assert!(!is_heavy(MediaKind::Flat, Level::Full));
    }

    #[test]
    fn embedded_previews_only_for_raw_files() {
        assert!(background_levels(MediaKind::Raw).contains(&Level::Embedded));
        assert!(!background_levels(MediaKind::Flat).contains(&Level::Embedded));
        for kind in [MediaKind::Raw, MediaKind::Flat] {
            assert!(
                !background_levels(kind).contains(&Level::Full),
                "1:1 à la demande"
            );
        }
    }

    #[test]
    fn progress_fraction() {
        assert_eq!(Progress { done: 1, total: 4 }.fraction(), 0.25);
        assert_eq!(Progress::default().fraction(), 1.0);
    }
}
