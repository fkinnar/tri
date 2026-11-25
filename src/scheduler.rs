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
    pub developers: crate::develop::SharedDevelopers,
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
        let developers = self.developers.current();
        let tool = developers
            .get(engine)
            .ok_or_else(|| failure(DevelopError::Unavailable(engine)))?;
        let sidecar = engine
            .sidecar(&file.path)
            .ok_or_else(|| failure(DevelopError::NotDeveloped))?;
        let work = &developers.work_dir;
        std::fs::create_dir_all(work).map_err(|e| Failure::Error(e.to_string()))?;
        let out = work.join(format!(
            "rendu-{}-{}-{:?}.jpg",
            engine.id(),
            std::process::id(),
            std::thread::current().id()
        ));
        // Ses sidecars remis à l'identique si le logiciel y touche (un rendu ne note rien).
        let guard = crate::develop::SidecarGuard::protect(&file.path);
        let rendered = tool.render(
            &file.path,
            &sidecar,
            &out,
            self.settings.sizes.standard,
            cancel,
        );
        drop(guard);
        rendered.map_err(failure)?;
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
        // Palette d'un rendu fini : le rendu d'abord (logiciel externe), s'il manque.
        if let Level::Palette(crate::palette::Source::Developed(engine)) = level {
            self.develop(file, engine, cancel)?;
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
    /// Une analyse seulement lue dans le cache ([`Analysis::OnDemand`]) n'y est pas : elle
    /// n'est pas calculée (elle compte quand même dans l'avancement).
    Missing {
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
    /// Analyse d'exposition ([`Level::Exposure`]) de tout le dossier : en fond, après
    /// vignettes et aperçus boîtier et avant les rendus RAW de fond ; ou seulement lue
    /// dans le cache ([`Analysis`]).
    pub exposure: Analysis,
    /// Score de netteté ([`Level::Sharpness`]) de tout le dossier (workers légers : JPEG
    /// boîtier), après vignettes et aperçus boîtier.
    pub sharpness: Analysis,
    /// Empreintes de regroupement ([`Level::Similarity`]) de tout le dossier (workers
    /// légers), juste après les vignettes.
    pub similarity: Analysis,
    /// Palettes de couleurs ([`Level::Palette`]) de tout le dossier, tirées de la source
    /// [`Config::palette`].
    pub palettes: Analysis,
    /// Source des palettes de tout le dossier (#492) ; `None` : aucune (choisie dans
    /// l'interface des palettes, voir [`Scheduler::set_palette`]).
    pub palette: Option<crate::palette::SourceKind>,
}

/// Une analyse sur tout le dossier (#564) : calculée en fond, seulement lue dans le
/// cache, ou rien du tout.
///
/// Dans le fichier de configuration (`[analysis]`) : `"background"`, `"on-demand"` ou
/// `"off"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Analysis {
    /// Tout le dossier calculé en fond.
    #[default]
    Background,
    /// Rien n'est calculé en fond : ce qui est déjà dans le cache est livré
    /// ([`Event::Ready`]), ce qui manque est signalé ([`Event::Missing`]). Une demande
    /// ([`Scheduler::request`]) est toujours calculée.
    OnDemand,
    /// Rien : ni calcul ni lecture du cache pour le dossier (l'interface retire l'outil).
    Off,
}

/// Les analyses de tout le dossier (section `[analysis]`, #564) : chacune calculée en
/// fond, seulement lue dans le cache (« à la demande » : l'outil reste grisé tant que le
/// dossier n'est pas calculé), ou retirée avec ses outils.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Analyses {
    /// Exposition : histogrammes, zones, badges, filtre « bien exposées ».
    pub exposure: Analysis,
    /// Netteté : scores, badges, filtre « floues ».
    pub sharpness: Analysis,
    /// Piles : empreintes de regroupement.
    pub stacks: Analysis,
    /// Palettes de couleurs : recherches par palette et par couleur (`colours`).
    #[serde(rename = "colours")]
    pub palettes: Analysis,
}

impl Analysis {
    /// Vrai sauf [`Analysis::Off`].
    pub fn is_on(self) -> bool {
        self != Analysis::Off
    }

    /// Identifiant du fichier de configuration.
    pub fn id(self) -> &'static str {
        match self {
            Analysis::Background => "background",
            Analysis::OnDemand => "on-demand",
            Analysis::Off => "off",
        }
    }
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
            exposure: Analysis::Background,
            sharpness: Analysis::Background,
            similarity: Analysis::Background,
            palettes: Analysis::Background,
            palette: None,
        }
    }
}

/// Niveaux dont l'avancement est suivi (calculés en fond) ; `Full` ne l'est qu'à la demande.
/// Les palettes (#492) ont une case pour toutes leurs sources (une seule à la fois).
const TRACKED: [Level; 7] = [
    Level::Thumbnail,
    Level::Embedded,
    Level::Standard,
    Level::Exposure,
    Level::Sharpness,
    Level::Similarity,
    Level::Palette(crate::palette::Source::Camera),
];

/// Case d'avancement de `level` dans [`TRACKED`].
fn tracked_slot(level: Level) -> Option<usize> {
    match level {
        Level::Palette(_) => Some(TRACKED.len() - 1),
        _ => TRACKED.iter().position(|&l| l == level),
    }
}

/// Niveaux calculés en fond pour un fichier : l'aperçu boîtier n'existe que pour les RAW,
/// leur rendu RAW dépend de la politique, les analyses des réglages `exposure` et
/// `sharpness` (une analyse seulement lue dans le cache en fait partie : le balayeur la
/// livre, [`sweeper`]).
fn background_levels(
    kind: MediaKind,
    raw_previews: RawPreviews,
    (exposure, sharpness): (Analysis, Analysis),
) -> Vec<Level> {
    let mut levels = match (kind, raw_previews) {
        (MediaKind::Raw, RawPreviews::Background) => {
            vec![Level::Thumbnail, Level::Embedded, Level::Standard]
        }
        (MediaKind::Raw, RawPreviews::OnDemand) => vec![Level::Thumbnail, Level::Embedded],
        (MediaKind::Flat, _) => vec![Level::Thumbnail, Level::Standard],
    };
    if exposure.is_on() {
        levels.push(Level::Exposure);
    }
    if sharpness.is_on() {
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
pub fn is_heavy(kind: MediaKind, level: Level) -> bool {
    // Développé : un logiciel externe qui occupe toute la machine, RAW ou image plate.
    // Palette : celle d'un rendu RAW ou d'un logiciel peut devoir le faire d'abord.
    matches!(
        level,
        Level::Developed(_) | Level::Palette(crate::palette::Source::Developed(_))
    ) || kind == MediaKind::Raw
        && matches!(
            level,
            Level::Standard
                | Level::Full
                | Level::Exposure
                | Level::Palette(crate::palette::Source::Raw)
        )
}

/// Classe de priorité des aperçus boîtier de fond (ni l'image affichée, ni ses voisines).
const EMBEDDED_BACKGROUND: u8 = 4;
/// Classe de priorité des analyses d'exposition de fond : avant les rendus RAW de fond
/// (~0,4 s chacune, elles alimentent les badges de la grille).
const EXPOSURE_BACKGROUND: u8 = 5;
/// Classe de priorité des aperçus standard de fond.
const STANDARD_BACKGROUND: u8 = 6;
/// Classe de priorité des palettes de fond (#492) : après le reste, on les demande pour
/// chercher, pas pour trier.
const PALETTE_BACKGROUND: u8 = 7;

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
    priority_in(job, focus, &group, None)
}

/// [`priority`] pour le groupe d'images affichées `group` (voir [`default_group`]).
fn priority_in(
    (index, level): Job,
    focus: Option<usize>,
    group: &[usize],
    visible: Option<(usize, usize)>,
) -> (u8, usize) {
    // Étape pour l'image affichée et ses voisines : ajusté (0), analyse (1), 1:1 (2).
    let (stage, background_class) = match level {
        Level::Thumbnail => return (3, thumbnail_rank(index, visible)),
        Level::Embedded => (0, EMBEDDED_BACKGROUND),
        Level::Standard => (0, STANDARD_BACKGROUND),
        Level::Exposure | Level::Sharpness => (1, EXPOSURE_BACKGROUND),
        // Empreintes : vite faites, et le regroupement de la grille en dépend.
        Level::Similarity => (1, EMBEDDED_BACKGROUND),
        Level::Palette(_) => (1, PALETTE_BACKGROUND),
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

/// Rang d'une vignette : celles des rangées visibles d'abord (dans l'ordre), puis les
/// autres par distance aux rangées visibles, en avant d'abord à distance égale (retour du
/// 29/10/2026 : descendu à l'ascenseur sur un NAS, on attendait toutes les vignettes du
/// dessus) ; sans rangées connues, l'ordre du dossier.
fn thumbnail_rank(index: usize, visible: Option<(usize, usize)>) -> usize {
    match visible {
        None => index,
        Some((first, last)) if (first..=last).contains(&index) => index - first,
        Some((first, last)) => {
            let shown = last - first + 1;
            if index > last {
                shown + (index - last) * 2
            } else {
                shown + (first - index) * 2 + 1
            }
        }
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
    /// Mode léger du dossier (#597) : en fond, seulement les vignettes et les analyses ;
    /// les aperçus de l'image affichée et de ses voisines à la demande.
    light: bool,
    exposure: Analysis,
    sharpness: Analysis,
    similarity: Analysis,
    palettes: Analysis,
    palette: Option<crate::palette::SourceKind>,
    /// Tâches demandées hors du travail de fond ([`Scheduler::request`]) : elles ne
    /// comptent pas dans la progression.
    on_demand: HashSet<Job>,
    generation: u64,
    files: Vec<MediaFile>,
    focus: Option<usize>,
    /// Images affichées, `focus` en tête (voir [`default_group`]).
    group: Vec<usize>,
    /// Vignettes visibles dans la grille (première, dernière), servies d'abord.
    visible: Option<(usize, usize)>,
    pending: HashSet<Job>,
    /// Tâches en attente que le balayeur a trouvées absentes du cache ([`sweeper`]) : il
    /// ne les revérifie pas ; les workers les calculent à leur tour.
    checked: HashSet<Job>,
    running: Vec<Running>,
    progress: [Progress; 7],
    paused: bool,
    /// Travail de fond en attente (#555) : un calcul demandé explicitement sur un dossier
    /// passe d'abord ; l'image affichée, ses voisines et les vignettes visibles continuent.
    background_paused: bool,
    shutdown: bool,
    /// Événements en attente de livraison, dans l'ordre où ils se sont produits.
    outbox: VecDeque<Event>,
    /// Vrai pendant un appel au callback.
    in_callback: bool,
}

impl State {
    /// Niveaux calculés en fond pour `file`, selon les réglages.
    fn background_levels(&self, file: &MediaFile) -> Vec<Level> {
        let mut levels = background_levels(
            file.kind,
            self.raw_previews,
            (self.exposure, self.sharpness),
        );
        if self.light {
            levels.retain(|l| !matches!(l, Level::Embedded | Level::Standard));
        }
        if self.similarity.is_on() {
            levels.push(Level::Similarity);
        }
        if let Some(source) = self
            .palette
            .filter(|_| self.palettes.is_on())
            .and_then(|k| k.source_for(file))
        {
            levels.push(Level::Palette(source));
        }
        levels
    }

    /// Vrai si `job` est seulement à lire dans le cache : une analyse [`Analysis::OnDemand`]
    /// du travail de fond (une demande, elle, est calculée).
    fn cache_only(&self, job: &Job) -> bool {
        let mode = match job.1 {
            Level::Exposure => self.exposure,
            Level::Sharpness => self.sharpness,
            Level::Similarity => self.similarity,
            Level::Palette(_) => self.palettes,
            _ => return false,
        };
        mode == Analysis::OnDemand && !self.on_demand.contains(job)
    }

    /// Prend la tâche la plus prioritaire pour ce type de worker.
    fn take(&mut self, heavy: bool) -> Option<(Job, MediaFile, Arc<AtomicBool>)> {
        if self.paused {
            return None;
        }
        let (focus, group, visible) = (self.focus, &self.group, self.visible);
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
        let background_paused = self.background_paused;
        let job = *self
            .pending
            .iter()
            .filter(|(i, l)| is_heavy(self.files[*i].kind, *l) == heavy)
            .filter(|job| !self.cache_only(job))
            .filter(|job| {
                let class = priority_in(**job, focus, group, visible).0;
                !blocked(class) && !(background_paused && class >= EMBEDDED_BACKGROUND)
            })
            .min_by_key(|job| (priority_in(**job, focus, group, visible), job.0))?;
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
                palettes: config.palettes,
                palette: config.palette,
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
        let mut threads: Vec<JoinHandle<()>> = heavy
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
        let (sweep_shared, sweep_producer) = (shared.clone(), producer.clone());
        threads.push(
            std::thread::Builder::new()
                .name("tri-cache".to_string())
                .spawn(move || sweeper(&sweep_shared, sweep_producer.as_ref()))
                .expect("création d'un thread de travail"),
        );
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
        s.visible = None;
        s.checked.clear();
        s.pending = (0..files.len())
            .flat_map(|i| {
                s.background_levels(&files[i])
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
                .filter(|f| {
                    s.background_levels(f)
                        .iter()
                        .any(|&l| tracked_slot(l) == tracked_slot(level))
                })
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

    /// Change les analyses de tout le dossier (réglages appliqués, #564) ; elles valent au
    /// prochain [`Scheduler::set_folder`].
    pub fn set_analyses(&self, analyses: Analyses) {
        let mut s = self.shared.lock();
        s.exposure = analyses.exposure;
        s.sharpness = analyses.sharpness;
        s.similarity = analyses.stacks;
        s.palettes = analyses.palettes;
    }

    /// Mode léger du dossier (#597) : en fond, seulement les vignettes (et les analyses
    /// selon leur réglage) ; l'aperçu de l'image affichée et de ses voisines (boîtier pour
    /// un RAW, standard pour une image plate) est demandé dès qu'elles sont affichées
    /// ([`Scheduler::set_shown`]). Vaut au prochain [`Scheduler::set_folder`].
    pub fn set_light(&self, light: bool) {
        self.shared.lock().light = light;
    }

    /// Change la politique des rendus RAW de fond (la grille montre les rendus LibRaw,
    /// #114) ; elle vaut au prochain [`Scheduler::set_folder`].
    pub fn set_raw_previews(&self, raw_previews: RawPreviews) {
        self.shared.lock().raw_previews = raw_previews;
    }

    /// Palettes de tout le dossier en fond, tirées de la source `kind` (#492 ; `None` :
    /// plus aucune). Les palettes d'une autre source encore en attente sont abandonnées ;
    /// celles déjà dans le cache ne sont pas refaites.
    pub fn set_palette(&self, kind: Option<crate::palette::SourceKind>) {
        let mut s = self.shared.lock();
        if s.palette == kind {
            return;
        }
        s.palette = kind;
        let State {
            pending, on_demand, ..
        } = &mut *s;
        pending.retain(|j| !matches!(j.1, Level::Palette(_)) || on_demand.contains(j));
        let jobs: Vec<Job> = s
            .files
            .iter()
            .enumerate()
            .filter(|_| s.palettes.is_on())
            .filter_map(|(i, f)| Some((i, Level::Palette(kind?.source_for(f)?))))
            .collect();
        // Avancement des palettes : celles de cette source, à partir de zéro.
        let slot = TRACKED.len() - 1;
        s.progress[slot] = Progress {
            done: 0,
            total: jobs.len(),
        };
        let event = Event::Progress {
            generation: s.generation,
            level: TRACKED[slot],
            progress: s.progress[slot],
        };
        s.pending.extend(jobs);
        s.outbox.push_back(event);
        drop(s);
        self.shared.work.notify_all();
        deliver(&self.shared);
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
            for level in s.background_levels(file) {
                s.pending.insert((start + k, level));
                if let Some(i) = tracked_slot(level) {
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

    /// Indique les vignettes visibles dans la grille (`first` à `last`, positions
    /// comprises) : leurs vignettes sont vérifiées et calculées avant les autres.
    pub fn set_visible(&self, first: usize, last: usize) {
        self.shared.lock().visible = Some((first.min(last), first.max(last)));
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
        if s.light {
            // Mode léger : l'aperçu des images affichées n'est pas en fond, il est demandé.
            for &i in &s.group.clone() {
                let level = match s.files[i].kind {
                    MediaKind::Raw => Level::Embedded,
                    MediaKind::Flat => Level::Standard,
                };
                let job = (i, level);
                if !s.pending.contains(&job) && !s.running.iter().any(|r| r.job == job) {
                    s.pending.insert(job);
                    s.on_demand.insert(job);
                }
            }
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
        if rank_in(&s.group, index).is_none() || index >= s.files.len() {
            return;
        }
        // Une analyse seulement lue dans le cache (à la demande, mode léger) : la demande
        // la fait calculer.
        let upgrade = s.pending.contains(&job) && s.cache_only(&job);
        if !upgrade && (s.pending.contains(&job) || s.running.iter().any(|r| r.job == job)) {
            return;
        }
        s.pending.insert(job);
        s.on_demand.insert(job);
        drop(s);
        self.shared.work.notify_all();
    }

    /// Demande pour l'image affichée `index` tout ce que le travail de fond calculerait
    /// hors du mode léger : aperçus et analyses (#597, une image ouverte en plein écran
    /// a tout son cache, même dans un dossier de 2000 photos où l'on n'en garde que 25).
    pub fn request_all(&self, index: usize) {
        let levels = {
            let mut s = self.shared.lock();
            let Some(file) = s.files.get(index).cloned() else {
                return;
            };
            let light = std::mem::replace(&mut s.light, false);
            let levels = s.background_levels(&file);
            s.light = light;
            levels
        };
        for level in levels.into_iter().filter(|&l| l != Level::Thumbnail) {
            self.request(index, level);
        }
    }

    /// Met en pause (aucune nouvelle tâche n'est prise ; celles en cours se terminent) ou
    /// reprend le travail.
    pub fn set_paused(&self, paused: bool) {
        self.shared.lock().paused = paused;
        self.shared.work.notify_all();
        self.shared.idle.notify_all();
    }

    /// Met en attente (ou reprend) le travail de fond du dossier (#555) : un calcul
    /// demandé explicitement passe d'abord. L'image affichée, ses voisines et les
    /// vignettes restent servies.
    pub fn set_background_paused(&self, paused: bool) {
        self.shared.lock().background_paused = paused;
        self.shared.work.notify_all();
        self.shared.idle.notify_all();
    }

    /// Avancement d'un niveau calculé en fond (`Thumbnail`, `Embedded`, `Standard`) ; `None`
    /// pour `Full`. Le total de `Embedded` est le nombre de RAW.
    pub fn progress(&self, level: Level) -> Option<Progress> {
        let i = tracked_slot(level)?;
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

impl Scheduler {
    /// Arrête les workers sans les attendre (fermeture de l'application : un calcul en
    /// cours sur un NAS lent peut durer, retour du 29/10/2026). Ils finissent leur fichier
    /// en fond ou disparaissent avec le processus ; les entrées du cache s'écrivent sous
    /// un nom temporaire puis sont renommées, jamais à moitié.
    pub fn detach(&mut self) {
        let mut s = self.shared.lock();
        s.shutdown = true;
        s.cancel_running(|_| false);
        drop(s);
        self.shared.work.notify_all();
        self.threads.clear();
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

/// Balayeur du cache (issue n° 428) : livre tout de suite les tâches en attente déjà en
/// cache, sans attendre leur tour. Après un changement de clé du cache (la gestion des
/// couleurs, #423), un dossier connu recalculait toutes ses vignettes, et ses empreintes,
/// netteté et expositions, toujours en cache, attendaient derrière (« vignettes
/// d'abord ») : les piles se formaient pas à pas. Il prend les tâches pas encore vérifiées
/// dans l'ordre des priorités, sans le blocage « vignettes d'abord » ([`State::take`]) ;
/// une tâche absente du cache reste dans la file, marquée vérifiée, pour les workers.
/// Un seul fil, qui ne fait que regarder le cache (une lecture de métadonnées par tâche).
fn sweeper(shared: &Shared, producer: &dyn Producer) {
    loop {
        // Les tâches à vérifier, par priorité : la file telle qu'elle est maintenant.
        let (batch, generation) = {
            let mut s = shared.lock();
            loop {
                if s.shutdown {
                    return;
                }
                if !s.paused {
                    let (focus, group, visible) = (s.focus, &s.group, s.visible);
                    let mut batch: Vec<Job> = s
                        .pending
                        .iter()
                        .filter(|job| !s.checked.contains(job))
                        .copied()
                        .collect();
                    if !batch.is_empty() {
                        batch.sort_by_key(|job| (priority_in(*job, focus, group, visible), job.0));
                        break (batch, s.generation);
                    }
                }
                s = shared.work.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };
        for job in batch {
            // La tâche reste dans la file pendant la vérification : un worker qui choisit
            // à ce moment voit toujours l'ordre des priorités intact (et peut la prendre).
            let file = {
                let s = shared.lock();
                if s.shutdown {
                    return;
                }
                if s.paused || s.generation != generation {
                    break;
                }
                // Prise entre-temps par un worker, ou abandonnée.
                if !s.pending.contains(&job) {
                    continue;
                }
                s.files[job.0].clone()
            };
            let done = producer.is_done(&file, job.1);
            let missing = {
                let mut s = shared.lock();
                // Toujours la même tâche, toujours en attente : sinon un worker s'en charge.
                if s.generation != generation || !s.pending.contains(&job) {
                    continue;
                }
                let missing = !done && s.cache_only(&job);
                if done {
                    s.pending.remove(&job);
                    let events = finish(&mut s, job, file, Ok(()));
                    s.outbox.extend(events);
                } else if missing {
                    // Seulement lue dans le cache : absente, elle n'est pas calculée.
                    s.pending.remove(&job);
                    let events = missing_events(&mut s, job, file);
                    s.outbox.extend(events);
                } else {
                    s.checked.insert(job);
                }
                missing
            };
            if done || missing {
                shared.work.notify_all();
                deliver(shared);
            }
        }
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

/// Une analyse seulement lue dans le cache, absente ([`Analysis::OnDemand`]) : comptée
/// comme terminée, signalée par [`Event::Missing`].
fn missing_events(s: &mut State, (index, level): Job, file: MediaFile) -> Vec<Event> {
    let generation = s.generation;
    let mut events = vec![Event::Missing {
        generation,
        index,
        path: file.path,
        level,
    }];
    if let Some(i) = tracked_slot(level) {
        s.progress[i].done += 1;
        events.push(Event::Progress {
            generation,
            level,
            progress: s.progress[i],
        });
    }
    events
}

/// Met à jour l'état après une tâche et prépare les événements à émettre.
fn finish(
    s: &mut State,
    (index, level): Job,
    file: MediaFile,
    result: Result<(), Failure>,
) -> Vec<Event> {
    let generation = s.generation;
    // Palette prête (calculée ou déjà en cache) : notée dans l'index de recherche (#561).
    if let (Ok(()), Level::Palette(source)) = (&result, level) {
        crate::index::note(&file, source);
    }
    // Empreinte de similitude prête : notée aussi (« semblables » partout).
    if let (Ok(()), Level::Similarity) = (&result, level) {
        crate::index::note_signature(&file);
    }
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
        Err(Failure::Error(error)) => {
            // Au journal (issue n° 398) : une vignette ou un aperçu raté n'est montré que
            // par une case vide ou une mention, sans le fichier ni la raison.
            crate::log::warning(
                "preview",
                &format!("{level:?} failed: {error}"),
                &format!("image: {}", path.display()),
            );
            vec![Event::Failed {
                generation,
                index,
                path,
                level,
                error,
            }]
        }
    };
    if let Some(i) = tracked_slot(level).filter(|_| !on_demand) {
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

    const ON: Analysis = Analysis::Background;
    const OFF: Analysis = Analysis::Off;

    #[test]
    fn visible_thumbnails_come_first_then_by_distance() {
        // Retour du 29/10/2026 : descendu à l'ascenseur, les vignettes du dessus passaient
        // avant celles de l'écran.
        assert_eq!(
            thumbnail_rank(5, None),
            5,
            "sans rangées connues : l'ordre du dossier"
        );
        let visible = Some((40, 43));
        let mut order: Vec<usize> = (36..48).collect();
        order.sort_by_key(|&i| thumbnail_rank(i, visible));
        assert_eq!(order, [40, 41, 42, 43, 44, 39, 45, 38, 46, 37, 47, 36]);
        assert!(
            priority_in((40, Level::Thumbnail), None, &[], visible)
                < priority_in((0, Level::Thumbnail), None, &[], visible)
        );
    }

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
            background_levels(MediaKind::Raw, Background, (ON, OFF)).contains(&Level::Embedded)
        );
        assert!(
            !background_levels(MediaKind::Flat, Background, (ON, OFF)).contains(&Level::Embedded)
        );
        for kind in [MediaKind::Raw, MediaKind::Flat] {
            for policy in [Background, OnDemand] {
                let levels = background_levels(kind, policy, (ON, OFF));
                assert!(!levels.contains(&Level::Full), "1:1 à la demande");
                assert!(!levels.contains(&Level::EmbeddedFull), "1:1 à la demande");
            }
        }
    }

    #[test]
    fn on_demand_policy_keeps_raw_renders_out_of_the_background() {
        use RawPreviews::*;
        assert!(
            background_levels(MediaKind::Raw, Background, (ON, OFF)).contains(&Level::Standard)
        );
        assert!(!background_levels(MediaKind::Raw, OnDemand, (ON, OFF)).contains(&Level::Standard));
        assert!(
            background_levels(MediaKind::Flat, OnDemand, (ON, OFF)).contains(&Level::Standard),
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
            assert!(background_levels(kind, OnDemand, (OFF, ON)).contains(&Level::Sharpness));
            assert!(!background_levels(kind, OnDemand, (OFF, OFF)).contains(&Level::Sharpness));
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
                background_levels(MediaKind::Raw, policy, (ON, OFF)).contains(&Level::Exposure)
            );
            assert!(
                !background_levels(MediaKind::Raw, policy, (OFF, OFF)).contains(&Level::Exposure)
            );
        }
        assert!(background_levels(MediaKind::Flat, OnDemand, (ON, OFF)).contains(&Level::Exposure));
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
