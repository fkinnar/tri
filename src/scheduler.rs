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
//! producteur dans les tests. Voir `doc/en/scheduler.md`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::cache::{Cache, Level};
use crate::media::{MediaFile, MediaKind};
use crate::preview::{self, Outcome, PreviewError, Settings};

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
    /// Réglages de calcul (tailles, rendu, seuils).
    pub settings: Settings,
    /// Logiciels de développement, pour les niveaux [`Level::Developed`] (aucun : pas de
    /// rendu développé).
    pub developers: crate::develop::Developers,
}

impl CachedPreviews {
    /// Niveau développé : rendu par le logiciel `engine` dans un fichier temporaire (sous
    /// le cache), puis le JPEG rangé tel quel dans le cache.
    fn develop(
        &self,
        file: &MediaFile,
        engine: crate::develop::Engine,
        cancel: &AtomicBool,
    ) -> Result<(), Failure> {
        use crate::develop::DevelopError;
        let failure = |e: DevelopError| match e {
            DevelopError::Cancelled => Failure::Cancelled,
            e => Failure::Error(e.to_string()),
        };
        let key = preview::key(file, Level::Developed(engine), &self.settings)
            .map_err(|e| Failure::Error(e.to_string()))?;
        if self.cache.contains(&key) {
            return Ok(());
        }
        let tool = self
            .developers
            .get(engine)
            .ok_or_else(|| failure(DevelopError::Unavailable))?;
        let sidecar = engine
            .sidecar(&file.path)
            .ok_or_else(|| failure(DevelopError::NotDeveloped))?;
        let work = &self.developers.work_dir;
        std::fs::create_dir_all(work).map_err(|e| Failure::Error(e.to_string()))?;
        let out = work.join(format!(
            "rendu-{}-{}-{:?}.jpg",
            engine.id(),
            std::process::id(),
            std::thread::current().id()
        ));
        tool.render(
            &file.path,
            &sidecar,
            &out,
            self.settings.sizes.standard,
            cancel,
        )
        .map_err(failure)?;
        let bytes = std::fs::read(&out).map_err(|e| Failure::Error(e.to_string()));
        let _ = std::fs::remove_file(&out);
        self.cache
            .put_bytes(&key, &bytes?)
            .map_err(|e| Failure::Error(e.to_string()))
    }
}

impl Producer for CachedPreviews {
    fn is_done(&self, file: &MediaFile, level: Level) -> bool {
        preview::key(file, level, &self.settings).is_ok_and(|k| self.cache.contains(&k))
    }

    fn produce(&self, file: &MediaFile, level: Level, cancel: &AtomicBool) -> Result<(), Failure> {
        if let Level::Developed(engine) = level {
            return self.develop(file, engine, cancel);
        }
        match preview::ensure(&self.cache, file, level, &self.settings, cancel) {
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
    /// Une demande ([`Scheduler::request`]) a été abandonnée parce que l'image affichée
    /// s'est éloignée : elle ne sera pas calculée, il faudra la redemander si besoin.
    Dropped {
        /// Dossier concerné.
        generation: u64,
        /// Position du fichier.
        index: usize,
        /// Chemin du fichier.
        path: PathBuf,
        /// Niveau concerné.
        level: Level,
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
    /// voisines passent toujours). Mesures dans `doc/en/scheduler.md`.
    pub thumbnails_first: bool,
    /// Rendus RAW (aperçus standard des RAW) : tout le dossier en fond, ou seulement à la
    /// demande ([`Scheduler::request`]).
    pub raw_previews: RawPreviews,
    /// Analyse d'exposition ([`Level::Exposure`]) de tout le dossier en fond, après
    /// vignettes et aperçus boîtier et avant les rendus RAW de fond ; sinon seulement à
    /// la demande.
    pub exposure: bool,
    /// Score de netteté ([`Level::Sharpness`]) de tout le dossier en fond (workers
    /// légers : JPEG boîtier), après vignettes et aperçus boîtier ; sinon à la demande.
    pub sharpness: bool,
    /// Empreintes de regroupement ([`Level::Similarity`]) de tout le dossier en fond
    /// (workers légers), juste après les vignettes.
    pub similarity: bool,
}

/// Politique de calcul des rendus RAW (aperçus standard des RAW). Les aperçus des images
/// plates sont toujours calculés en fond : c'est leur seul aperçu plein écran.
///
/// Dans le fichier de configuration : `"background"` ou `"on-demand"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawPreviews {
    /// Tout le dossier en fond (après vignettes et aperçus boîtier).
    #[default]
    Background,
    /// Seulement à la demande : l'interface les demande pour l'image affichée et ses
    /// voisines quand l'utilisateur regarde le RAW.
    OnDemand,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Self {
            light_workers: (cores / 2).max(1),
            thumbnails_first: true,
            raw_previews: RawPreviews::default(),
            exposure: true,
            sharpness: true,
            similarity: true,
        }
    }
}

/// Niveaux dont l'avancement est suivi (calculés en fond) ; `Full` ne l'est qu'à la demande.
const TRACKED: [Level; 6] = [
    Level::Thumbnail,
    Level::Embedded,
    Level::Standard,
    Level::Exposure,
    Level::Sharpness,
    Level::Similarity,
];

/// Niveaux calculés en fond pour un fichier : l'aperçu boîtier n'existe que pour les RAW,
/// leur rendu RAW dépend de la politique, les analyses des réglages `exposure` et
/// `sharpness`.
fn background_levels(
    kind: MediaKind,
    raw_previews: RawPreviews,
    (exposure, sharpness): (bool, bool),
) -> Vec<Level> {
    let mut levels = match (kind, raw_previews) {
        (MediaKind::Raw, RawPreviews::Background) => {
            vec![Level::Thumbnail, Level::Embedded, Level::Standard]
        }
        (MediaKind::Raw, RawPreviews::OnDemand) => vec![Level::Thumbnail, Level::Embedded],
        (MediaKind::Flat, _) => vec![Level::Thumbnail, Level::Standard],
    };
    if exposure {
        levels.push(Level::Exposure);
    }
    if sharpness {
        levels.push(Level::Sharpness);
    }
    levels
}

/// Niveaux « 1:1 » : les plus urgents (l'utilisateur attend devant l'image).
fn is_zoom_level(level: Level) -> bool {
    matches!(level, Level::Full | Level::EmbeddedFull)
}

/// Images affichées : l'image principale d'abord, puis les autres par ordre d'importance.
/// Par défaut ([`Scheduler::set_focus`]) l'image et ses voisines, suivante puis précédente ;
/// en comparaison, toutes les images comparées ([`Scheduler::set_shown`]).
fn default_group(focus: usize, len: usize) -> Vec<usize> {
    [focus, focus + 1, focus.wrapping_sub(1)]
        .into_iter()
        .filter(|&i| i < len)
        .collect()
}

/// Rang de `index` parmi les images affichées `group` (0 : l'image principale).
fn rank_in(group: &[usize], index: usize) -> Option<usize> {
    group.iter().position(|&i| i == index)
}

/// Une tâche : un fichier (par sa position) et un niveau.
type Job = (usize, Level);

/// Vrai si la tâche passe par le worker lourd (LibRaw : rendu, ou décompression pour
/// l'analyse d'exposition, elle aussi parallélisée en interne).
fn is_heavy(kind: MediaKind, level: Level) -> bool {
    // Développé : un logiciel externe qui occupe toute la machine, RAW ou image plate.
    matches!(level, Level::Developed(_))
        || kind == MediaKind::Raw
            && matches!(level, Level::Standard | Level::Full | Level::Exposure)
}

/// Classe de priorité des aperçus boîtier de fond (ni l'image affichée, ni ses voisines).
const EMBEDDED_BACKGROUND: u8 = 4;
/// Classe de priorité des analyses d'exposition de fond : avant les rendus RAW de fond
/// (~0,4 s chacune, elles alimentent les badges de la grille).
const EXPOSURE_BACKGROUND: u8 = 5;
/// Classe de priorité des aperçus standard de fond.
const STANDARD_BACKGROUND: u8 = 6;

/// Priorité d'une tâche (la plus petite passe en premier). Ordre :
/// 1. l'image affichée : d'abord ce qui s'affiche ajusté (aperçu boîtier, rendu RAW
///    ~0,5 s), puis son analyse d'exposition (histogramme, ~0,4 s), puis son 1:1 (100 %
///    RAW ~4,5 s) : en mode RAW, l'aspect RAW apparaît vite, la netteté du 100 % suit ;
/// 2. ses voisines (suivante, puis précédente) : ajusté, analyse, puis 1:1
///    (préchargement) ;
/// 3. les vignettes, dans l'ordre du dossier ;
/// 4. les autres aperçus boîtier, puis les autres analyses, puis les autres aperçus
///    standard, par distance à l'image affichée (en avant d'abord à distance égale).
///
/// (Aperçus boîtier et 1:1 du JPEG passent par les workers légers, rendus RAW par le
/// worker lourd : chaque file ne voit que ses propres niveaux.)
#[cfg(test)]
fn priority(job: Job, focus: Option<usize>) -> (u8, usize) {
    let group = focus
        .map(|f| default_group(f, usize::MAX))
        .unwrap_or_default();
    priority_in(job, focus, &group)
}

/// [`priority`] pour le groupe d'images affichées `group` (voir [`default_group`]).
fn priority_in((index, level): Job, focus: Option<usize>, group: &[usize]) -> (u8, usize) {
    // Étape pour l'image affichée et ses voisines : ajusté (0), analyse (1), 1:1 (2).
    let (stage, background_class) = match level {
        Level::Thumbnail => return (3, index),
        Level::Embedded => (0, EMBEDDED_BACKGROUND),
        Level::Standard => (0, STANDARD_BACKGROUND),
        Level::Exposure | Level::Sharpness => (1, EXPOSURE_BACKGROUND),
        // Empreintes : vite faites, et le regroupement de la grille en dépend.
        Level::Similarity => (1, EMBEDDED_BACKGROUND),
        Level::EmbeddedFull | Level::Full => (2, STANDARD_BACKGROUND),
        // Développé (plusieurs secondes) : comme un ajusté, à la demande.
        Level::Developed(_) => (0, STANDARD_BACKGROUND),
    };
    let near = rank_in(group, index);
    // Autres images affichées : voisines (2) en plein écran, plus en comparaison.
    let others = group.len().saturating_sub(1).max(2);
    match (near, focus) {
        // Image affichée : ajusté, analyse, 1:1 (0, 1, 2) ; puis les autres, étape par
        // étape : ajusté (3, 4…), analyse, 1:1.
        (Some(0), _) => (0, stage),
        (Some(rank), _) => (0, 3 + stage * others + (rank - 1)),
        (None, Some(f)) => (
            background_class,
            index.abs_diff(f) * 2 + usize::from(index < f),
        ),
        (None, None) => (background_class, index),
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
    raw_previews: RawPreviews,
    exposure: bool,
    sharpness: bool,
    similarity: bool,
    /// Tâches demandées hors du travail de fond ([`Scheduler::request`]) : elles ne
    /// comptent pas dans la progression.
    on_demand: HashSet<Job>,
    generation: u64,
    files: Vec<MediaFile>,
    focus: Option<usize>,
    /// Images affichées, `focus` en tête (voir [`default_group`]).
    group: Vec<usize>,
    pending: HashSet<Job>,
    running: Vec<Running>,
    progress: [Progress; 6],
    paused: bool,
    shutdown: bool,
    /// Événements en attente de livraison, dans l'ordre où ils se sont produits.
    outbox: VecDeque<Event>,
    /// Vrai pendant un appel au callback.
    in_callback: bool,
}

impl State {
    /// Niveaux calculés en fond pour un fichier de type `kind`, selon les réglages.
    fn background_levels(&self, kind: MediaKind) -> Vec<Level> {
        let mut levels =
            background_levels(kind, self.raw_previews, (self.exposure, self.sharpness));
        if self.similarity {
            levels.push(Level::Similarity);
        }
        levels
    }

    /// Prend la tâche la plus prioritaire pour ce type de worker.
    fn take(&mut self, heavy: bool) -> Option<(Job, MediaFile, Arc<AtomicBool>)> {
        if self.paused {
            return None;
        }
        let (focus, group) = (self.focus, &self.group);
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
            EXPOSURE_BACKGROUND | STANDARD_BACKGROUND => thumbnails_left || embedded_left,
            _ => false,
        };
        let job = *self
            .pending
            .iter()
            .filter(|(i, l)| is_heavy(self.files[*i].kind, *l) == heavy)
            .filter(|job| !blocked(priority_in(**job, focus, group).0))
            .min_by_key(|job| (priority_in(**job, focus, group), job.0))?;
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
                raw_previews: config.raw_previews,
                exposure: config.exposure,
                sharpness: config.sharpness,
                similarity: config.similarity,
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
        s.outbox.clear(); // événements de l'ancien dossier pas encore livrés
        let on_demand = std::mem::take(&mut s.on_demand);
        s.generation += 1;
        s.focus = None;
        s.group.clear();
        s.pending = (0..files.len())
            .flat_map(|i| {
                s.background_levels(files[i].kind)
                    .into_iter()
                    .map(move |l| (i, l))
            })
            .collect();
        // Travail de fond en cours pour une image qui reste (même dossier refiltré, issue
        // n° 73) : il continue, à sa nouvelle place ; le reste est annulé. Sans cela,
        // chaque refiltrage abandonnait l'analyse en cours.
        let index_of: HashMap<&Path, usize> = files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.path.as_path(), i))
            .collect();
        let generation = s.generation;
        let mut running = std::mem::take(&mut s.running);
        for r in &mut running {
            let level = r.job.1;
            let moved = s
                .files
                .get(r.job.0)
                .and_then(|f| index_of.get(f.path.as_path()).copied())
                .filter(|&i| !on_demand.contains(&r.job) && s.pending.contains(&(i, level)));
            match moved {
                Some(i) => {
                    r.job.0 = i;
                    r.generation = generation;
                    s.pending.remove(&(i, level));
                }
                None => r.cancel.store(true, Ordering::Relaxed),
            }
        }
        s.running = running;
        let total = |level: Level| {
            files
                .iter()
                .filter(|f| s.background_levels(f.kind).contains(&level))
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

    /// Ajoute `files` à la fin du dossier en cours sans rien interrompre : leur travail de
    /// fond rejoint la file, les totaux d'avancement grandissent (signalés par
    /// [`Event::Progress`]). Sert au déchargement, qui calcule chaque photo dès qu'elle est
    /// copiée (issue n° 40). Les positions continuent celles du dossier.
    pub fn add_files(&self, files: Vec<MediaFile>) {
        let mut s = self.shared.lock();
        let start = s.files.len();
        let mut grown = [false; TRACKED.len()];
        for (k, file) in files.iter().enumerate() {
            for level in s.background_levels(file.kind) {
                s.pending.insert((start + k, level));
                if let Some(i) = TRACKED.iter().position(|&l| l == level) {
                    s.progress[i].total += 1;
                    grown[i] = true;
                }
            }
        }
        s.files.extend(files);
        let generation = s.generation;
        for (i, &level) in TRACKED.iter().enumerate().filter(|(i, _)| grown[*i]) {
            let progress = s.progress[i];
            s.outbox.push_back(Event::Progress {
                generation,
                level,
                progress,
            });
        }
        drop(s);
        self.shared.work.notify_all();
        deliver(&self.shared);
    }

    /// Indique l'image affichée : son aperçu passe en tête, puis ses voisines. Les demandes
    /// pour des images qui ne sont plus voisines sont abandonnées (un 1:1 en cours est
    /// annulé) et signalées par [`Event::Dropped`].
    pub fn set_focus(&self, index: usize) {
        let len = self.shared.lock().files.len();
        self.set_shown(&default_group(index, len));
    }

    /// Indique les images affichées, la principale d'abord (comparaison côte à côte :
    /// toutes les images comparées, l'active en tête) : leurs aperçus passent en tête, et
    /// [`Scheduler::request`] les accepte toutes. Comme [`Scheduler::set_focus`], abandonne
    /// les demandes pour les images qui ne sont plus affichées.
    pub fn set_shown(&self, images: &[usize]) {
        let mut s = self.shared.lock();
        let len = s.files.len();
        let group: Vec<usize> = images.iter().copied().filter(|&i| i < len).collect();
        let Some(&index) = group.first() else {
            return;
        };
        if s.group == group {
            return;
        }
        s.focus = Some(index);
        s.group = group;
        let dropped: Vec<Job> = s
            .on_demand
            .iter()
            .filter(|&&(i, _)| rank_in(&s.group, i).is_none())
            .filter(|j| s.pending.contains(j))
            .copied()
            .collect();
        let generation = s.generation;
        for job in dropped {
            s.pending.remove(&job);
            s.on_demand.remove(&job);
            let path = s.files[job.0].path.clone();
            s.outbox.push_back(Event::Dropped {
                generation,
                index: job.0,
                path,
                level: job.1,
            });
        }
        // Un 1:1 en cours pour une image éloignée est annulé (signalé à sa fin).
        let group = s.group.clone();
        s.cancel_running(|r| !is_zoom_level(r.job.1) || rank_in(&group, r.job.0).is_some());
        drop(s);
        self.shared.work.notify_all();
        deliver(&self.shared);
    }

    /// Demande une image hors du travail de fond, pour une image affichée (l'image
    /// courante, ses voisines, ou une image comparée, voir [`Scheduler::set_shown`]) : 1:1 (`Full`, `EmbeddedFull` ; ceux des voisines servent au préchargement
    /// quand on navigue zoomé) ou rendu RAW (`Standard`) en politique
    /// [`RawPreviews::OnDemand`]. Ne compte pas dans la progression ; signalée par `Ready`
    /// (immédiatement si déjà en cache), `Failed`, ou `Dropped` si l'image affichée
    /// s'éloigne avant.
    ///
    /// Ignorée sans image affichée, si `index` n'est pas affichée, ou si la tâche est déjà
    /// prévue.
    pub fn request(&self, index: usize, level: Level) {
        let mut s = self.shared.lock();
        if s.focus.is_none() {
            return;
        }
        let job = (index, level);
        if rank_in(&s.group, index).is_none()
            || index >= s.files.len()
            || s.pending.contains(&job)
            || s.running.iter().any(|r| r.job == job)
        {
            return;
        }
        s.pending.insert(job);
        s.on_demand.insert(job);
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
            // La tâche a pu changer de place (dossier refiltré, voir `set_folder`).
            let Running {
                job, generation, ..
            } = s.running.swap_remove(pos);
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
    let on_demand = s.on_demand.remove(&(index, level));
    let mut events = match result {
        Ok(()) => vec![Event::Ready {
            generation,
            index,
            path,
            level,
        }],
        // Annulée sans changement de dossier : une demande abandonnée (l'image affichée
        // s'est éloignée) ; à signaler pour qu'elle puisse être redemandée.
        Err(Failure::Cancelled) if on_demand => {
            return vec![Event::Dropped {
                generation,
                index,
                path,
                level,
            }];
        }
        Err(Failure::Cancelled) => return Vec::new(),
        Err(Failure::Error(error)) => vec![Event::Failed {
            generation,
            index,
            path,
            level,
            error,
        }],
    };
    if let Some(i) = TRACKED
        .iter()
        .position(|&l| l == level)
        .filter(|_| !on_demand)
    {
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
        let sorted = |mut jobs: Vec<Job>| {
            jobs.sort_by_key(|j| (priority(*j, f), j.0));
            jobs
        };
        // Worker lourd (rendus RAW) : image affichée ajustée puis à 100 %, puis voisines.
        assert_eq!(
            sorted(vec![
                (9, Level::Standard),
                (4, Level::Full),
                (4, Level::Standard),
                (6, Level::Full),
                (6, Level::Standard),
                (5, Level::Full),
                (5, Level::Standard),
                (3, Level::Standard),
                (7, Level::Standard),
            ]),
            [
                (5, Level::Standard),
                (5, Level::Full),
                (6, Level::Standard),
                (4, Level::Standard),
                (6, Level::Full),
                (4, Level::Full),
                (7, Level::Standard),
                (3, Level::Standard),
                (9, Level::Standard),
            ]
        );
        // Workers légers : aperçus boîtier proches, 1:1 du JPEG, vignettes, reste du dossier.
        assert_eq!(
            sorted(vec![
                (0, Level::Thumbnail),
                (9, Level::Embedded),
                (5, Level::EmbeddedFull),
                (6, Level::Embedded),
                (8, Level::Thumbnail),
                (5, Level::Embedded),
            ]),
            [
                (5, Level::Embedded),
                (5, Level::EmbeddedFull),
                (6, Level::Embedded),
                (0, Level::Thumbnail),
                (8, Level::Thumbnail),
                (9, Level::Embedded),
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
        use RawPreviews::*;
        assert!(
            background_levels(MediaKind::Raw, Background, (true, false)).contains(&Level::Embedded)
        );
        assert!(
            !background_levels(MediaKind::Flat, Background, (true, false))
                .contains(&Level::Embedded)
        );
        for kind in [MediaKind::Raw, MediaKind::Flat] {
            for policy in [Background, OnDemand] {
                let levels = background_levels(kind, policy, (true, false));
                assert!(!levels.contains(&Level::Full), "1:1 à la demande");
                assert!(!levels.contains(&Level::EmbeddedFull), "1:1 à la demande");
            }
        }
    }

    #[test]
    fn on_demand_policy_keeps_raw_renders_out_of_the_background() {
        use RawPreviews::*;
        assert!(
            background_levels(MediaKind::Raw, Background, (true, false)).contains(&Level::Standard)
        );
        assert!(
            !background_levels(MediaKind::Raw, OnDemand, (true, false)).contains(&Level::Standard)
        );
        assert!(
            background_levels(MediaKind::Flat, OnDemand, (true, false)).contains(&Level::Standard),
            "les images plates gardent leur aperçu"
        );
        assert!(
            !is_heavy(MediaKind::Raw, Level::EmbeddedFull),
            "1:1 du JPEG : léger"
        );
        assert_eq!(
            priority((3, Level::EmbeddedFull), Some(3)),
            (0, 2),
            "après l'ajusté et l'analyse"
        );
    }

    #[test]
    fn sharpness_scores_are_light_background_work() {
        use RawPreviews::*;
        for kind in [MediaKind::Raw, MediaKind::Flat] {
            assert!(background_levels(kind, OnDemand, (false, true)).contains(&Level::Sharpness));
            assert!(!background_levels(kind, OnDemand, (false, false)).contains(&Level::Sharpness));
            assert!(
                !is_heavy(kind, Level::Sharpness),
                "JPEG boîtier : workers légers"
            );
        }
        let focus = Some(5);
        assert!(priority((20, Level::Embedded), focus) < priority((20, Level::Sharpness), focus));
        assert!(priority((5, Level::Embedded), focus) < priority((5, Level::Sharpness), focus));
    }

    #[test]
    fn exposure_analysis_is_scheduled_like_a_raw_render() {
        use RawPreviews::*;
        for policy in [Background, OnDemand] {
            assert!(
                background_levels(MediaKind::Raw, policy, (true, false)).contains(&Level::Exposure)
            );
            assert!(
                !background_levels(MediaKind::Raw, policy, (false, false))
                    .contains(&Level::Exposure)
            );
        }
        assert!(
            background_levels(MediaKind::Flat, OnDemand, (true, false)).contains(&Level::Exposure)
        );
        assert!(
            is_heavy(MediaKind::Raw, Level::Exposure),
            "décompression LibRaw"
        );
        assert!(!is_heavy(MediaKind::Flat, Level::Exposure));
        // Image affichée : ajusté, puis analyse, puis 1:1 ; puis les voisines.
        let focus = Some(5);
        let order = [
            (5, Level::Standard),
            (5, Level::Exposure),
            (5, Level::Full),
            (6, Level::Standard),
            (4, Level::Standard),
            (6, Level::Exposure),
            (4, Level::Exposure),
            (6, Level::Full),
            (4, Level::Full),
        ];
        for pair in order.windows(2) {
            assert!(
                priority(pair[0], focus) < priority(pair[1], focus),
                "{:?} avant {:?}",
                pair[0],
                pair[1]
            );
        }
        // En fond : analyses avant les rendus RAW, après les aperçus boîtier.
        assert!(priority((20, Level::Embedded), focus) < priority((20, Level::Exposure), focus));
        assert!(priority((20, Level::Exposure), focus) < priority((7, Level::Standard), focus));
    }

    #[test]
    fn progress_fraction() {
        assert_eq!(Progress { done: 1, total: 4 }.fraction(), 0.25);
        assert_eq!(Progress::default().fraction(), 1.0);
    }
}
