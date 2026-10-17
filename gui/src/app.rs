//! L'application egui : grille de vignettes et plein écran, alimentés par le scheduler du
//! moteur.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, ColorImage, Event as UiEvent, FontId, Rect, ScrollArea, Sense,
    TextureHandle, TextureOptions, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use tri::cache::{Cache, Level};
use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::{MediaFile, MediaKind};
use tri::preview;
use tri::scheduler::{CachedPreviews, Event, Progress, Scheduler};
use tri::settings::EngineSettings;

use crate::actions::{Action, Chord, Keymap, Sequencer};
use crate::grid::{self, Layout};
use crate::loader::Loader;
use crate::settings::{AppSettings, GuiSettings};
use crate::textures::Lru;
use crate::trace::trace;
use crate::view::{self, Source};

/// Ce qui occupe la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Grid,
    View,
}

/// Une grande texture, avec les dimensions d'origine de l'image (la texture a pu être
/// réduite pour tenir dans les limites du GPU).
#[derive(Clone)]
struct Big {
    texture: TextureHandle,
    original: Vec2,
    /// 1:1 du JPEG boîtier : part du capteur qu'il représente.
    sensor_ratio: Option<f32>,
}

/// Défilement demandé. Résolu au dessin de la grille, quand la largeur (donc le nombre de
/// colonnes) est connue : la calculer avant donnait une mauvaise ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reveal {
    /// Défiler juste assez pour que la sélection soit visible.
    Minimal,
    /// Mettre la ligne sélectionnée en haut de la vue (nouvelle liste).
    Top,
}

/// Disponibilité d'un niveau d'image pour un fichier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Avail {
    #[default]
    Pending,
    Ready,
    Failed(String),
}

#[derive(Clone, Debug, Default)]
struct Status {
    thumbnail: Avail,
    embedded: Avail,
    standard: Avail,
    embedded_full: Avail,
    full: Avail,
}

/// Le moteur, démarré à la première image (il faut connaître la taille de l'écran).
struct Engine {
    scheduler: Scheduler,
    events: Receiver<Event>,
    generation: u64,
    cache: Cache,
    preview: preview::Settings,
    loader: Loader,
}

/// Capture d'écran automatique pour le développement (`TRI_GUI_SCREENSHOT=<fichier.png>`) :
/// prise une fois toutes les vignettes prêtes, puis l'application se ferme.
struct Screenshot {
    path: PathBuf,
    /// Capture demandée par le script (`shot`) : tout de suite, sans attendre.
    forced: bool,
    requested: bool,
    done: Arc<AtomicBool>,
    started: Instant,
}

/// État de l'application.
pub struct TriApp {
    dir: PathBuf,
    /// Réglages du moteur (fichier de configuration).
    settings: EngineSettings,
    /// Réglages de l'interface (section `[gui]`).
    gui: GuiSettings,
    /// Problème de configuration à signaler (fichier invalide...), dans la barre d'état.
    warning: Option<String>,
    all: Vec<MediaFile>,
    filter: KindFilter,
    files: Vec<MediaFile>,
    status: Vec<Status>,
    progress: [Progress; 3],
    engine: Option<Engine>,
    error: Option<String>,
    keymap: Keymap,
    /// Séquences de touches en cours (`g g`).
    sequencer: Sequencer,
    /// Déplacements clavier d'une image zoomée pas encore appliqués, en fractions de la
    /// vue (appliqués au dessin, quand la taille de l'image est connue).
    pan: Vec2,
    textures: Lru<PathBuf, TextureHandle>,
    selected: usize,
    layout: Layout,
    scroll_offset: f32,
    view_height: f32,
    reveal: Option<Reveal>,
    screenshot: Option<Screenshot>,
    help: bool,
    /// Actions à exécuter à la première image (`TRI_GUI_ACTIONS`, développement).
    startup_actions: std::collections::VecDeque<Step>,
    /// Pause en cours dans le script de démarrage.
    script_wait: Option<Instant>,
    /// Vignettes visibles prêtes mais pas encore chargées (budget de l'image dépassé).
    visible_waiting: usize,
    mode: Mode,
    /// Source préférée en plein écran pour les RAW (JPEG boîtier par défaut).
    source: Source,
    /// Zoom 100 % demandé : point de l'image (coordonnées relatives) au centre de la vue.
    /// Conservé d'une image à l'autre pour comparer la netteté au même endroit.
    zoom: Option<Vec2>,
    /// Zoom par double-clic : point cliqué (relatif) et position du curseur, appliqués dès
    /// que le 100 % est affiché (sa taille n'est connue qu'à ce moment) pour garder ce point
    /// sous le curseur.
    zoom_anchor: Option<(Vec2, egui::Pos2)>,
    /// Grandes textures chargées, par fichier et niveau.
    big: Lru<(PathBuf, Level), Big>,
    /// Dernière liste envoyée au chargeur (pour ne pas la renvoyer à chaque image).
    wanted: Vec<(PathBuf, Level)>,
    /// Entrées annoncées prêtes mais introuvables dans le cache : ne pas les redemander.
    missing: HashSet<(PathBuf, Level)>,
    /// Plein écran : l'image voulue n'est pas encore affichée (pour la capture automatique).
    view_waiting: bool,
    /// Demandes en cours auprès du scheduler (1:1, rendus RAW à la demande) : ne pas les
    /// répéter à chaque image affichée.
    requested: HashSet<(PathBuf, Level)>,
    /// Taille (pixels) du dernier 1:1 affiché, par niveau : sert à afficher l'aperçu d'une
    /// image voisine déjà à l'échelle 1:1 en attendant son propre 1:1.
    last_zoom_size: std::collections::HashMap<Level, Vec2>,
    /// Moment où l'image sélectionnée l'a été (attente avant un 100 % RAW).
    focus_since: Instant,
    /// Dernière image affichée en plein écran (pour la trace).
    last_shown: (usize, Option<Level>),
}

impl TriApp {
    /// Application sur le dossier `dir` (parcouru tout de suite, calculs lancés à la première
    /// image), avec les réglages `settings`. `warning` : problème de configuration à
    /// afficher.
    pub fn new(dir: PathBuf, settings: AppSettings, warning: Option<String>) -> TriApp {
        let gui = settings.gui.clone();
        let (all, error) = match tri::media::scan(&dir, 1) {
            Ok(files) => (files, None),
            Err(e) => (Vec::new(), Some(format!("{} : {e}", dir.display()))),
        };
        let screenshot = std::env::var_os("TRI_GUI_SCREENSHOT").map(|p| Screenshot {
            path: p.into(),
            forced: false,
            requested: false,
            done: Arc::default(),
            started: Instant::now(),
        });
        let selected = std::env::var("TRI_GUI_SELECT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let mut app = TriApp {
            dir,
            settings: settings.engine(),
            warning,
            all,
            filter: KindFilter::All,
            files: Vec::new(),
            status: Vec::new(),
            progress: [Progress::default(); 3],
            engine: None,
            error,
            keymap: gui.keymap(),
            sequencer: Sequencer::new(gui.key_sequence_timeout()),
            pan: Vec2::ZERO,
            textures: Lru::new(gui.grid.textures),
            selected,
            layout: Layout::fit(0.0, gui.grid.cell, gui.grid.gap),
            scroll_offset: 0.0,
            view_height: 0.0,
            reveal: None,
            screenshot,
            help: false,
            visible_waiting: 0,
            startup_actions: startup_actions().into(),
            script_wait: None,
            mode: Mode::Grid,
            source: Source::default(),
            zoom: None,
            zoom_anchor: None,
            big: Lru::new(gui.view.textures),
            wanted: Vec::new(),
            missing: HashSet::new(),
            view_waiting: false,
            requested: HashSet::new(),
            last_shown: (usize::MAX, None),
            focus_since: Instant::now(),
            last_zoom_size: std::collections::HashMap::new(),
            gui,
        };
        app.apply_filter();
        app
    }

    /// Recalcule la liste affichée selon le filtre, et la confie au scheduler.
    fn apply_filter(&mut self) {
        let selected_path = self.files.get(self.selected).map(|f| f.path.clone());
        self.files = self
            .filter
            .apply(ungrouped(self.all.clone()))
            .into_iter()
            .flatten()
            .collect();
        self.status = vec![Status::default(); self.files.len()];
        self.requested.clear();
        self.progress = [Progress::default(); 3];
        // Garder la même image sélectionnée si elle reste visible.
        if let Some(path) = selected_path {
            self.selected = self.files.iter().position(|f| f.path == path).unwrap_or(0);
        }
        self.selected = self.selected.min(self.files.len().saturating_sub(1));
        // La liste a changé : remettre la ligne sélectionnée en haut de la vue.
        self.reveal = Some(Reveal::Top);
        if let Some(engine) = &mut self.engine {
            engine.generation = engine.scheduler.set_folder(self.files.clone());
            engine.scheduler.set_focus(self.selected);
        }
    }

    /// Démarre le moteur : taille d'aperçu « auto » = plus grand côté de l'écran.
    fn start_engine(&mut self, ctx: &egui::Context) {
        let monitor = ctx.input(|i| i.viewport().monitor_size);
        let ppp = ctx.pixels_per_point();
        let screen_side = monitor.map(|m| (m.x.max(m.y) * ppp).round() as u32);
        let preview = self.settings.preview(screen_side);
        let cache = match self.settings.open_cache() {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(format!("cache inaccessible : {e}"));
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let producer = CachedPreviews {
            cache: cache.clone(),
            settings: preview,
        };
        let config = self.settings.scheduler(self.gui.raw_previews);
        let scheduler = Scheduler::new(producer, config, move |event| {
            let _ = tx.send(event);
            repaint.request_repaint();
        });
        let generation = scheduler.set_folder(self.files.clone());
        scheduler.set_focus(self.selected);
        let max_texture = ctx.input(|i| i.max_texture_side);
        let loader = Loader::new(cache.clone(), preview, max_texture, ctx.clone());
        self.engine = Some(Engine {
            scheduler,
            events: rx,
            generation,
            cache,
            preview,
            loader,
        });
    }

    /// Intègre les événements du moteur reçus depuis la dernière image.
    fn drain_events(&mut self) {
        let Some(engine) = &self.engine else { return };
        let generation = engine.generation;
        while let Ok(event) = engine.events.try_recv() {
            match event {
                Event::Ready {
                    generation: g,
                    index,
                    level,
                    ..
                } if g == generation => {
                    if level != Level::Thumbnail {
                        trace(|| format!("prête dans le cache : image {index} {level:?}"));
                    }
                    // Prête à nouveau (recalculée) : on peut la redemander au chargeur.
                    if let Some(f) = self.files.get(index) {
                        self.missing.remove(&(f.path.clone(), level));
                        self.requested.remove(&(f.path.clone(), level));
                    }
                    set(&mut self.status, index, level, Avail::Ready)
                }
                Event::Failed {
                    generation: g,
                    index,
                    level,
                    error,
                    ..
                } if g == generation => {
                    if let Some(f) = self.files.get(index) {
                        self.requested.remove(&(f.path.clone(), level));
                    }
                    set(&mut self.status, index, level, Avail::Failed(error))
                }
                Event::Progress {
                    generation: g,
                    level,
                    progress,
                } if g == generation => {
                    if let Some(i) = tracked(level) {
                        self.progress[i] = progress;
                    }
                }
                Event::Dropped {
                    generation: g,
                    index,
                    level,
                    ..
                } if g == generation => {
                    // Abandonnée par le moteur (image éloignée) : à redemander si besoin.
                    if let Some(f) = self.files.get(index) {
                        trace(|| format!("abandonnée par le moteur : image {index} {level:?}"));
                        self.requested.remove(&(f.path.clone(), level));
                    }
                }
                _ => {} // ancien dossier, ou niveau non suivi
            }
        }
    }

    /// Traduit les touches pressées en actions et les applique.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let chords: Vec<Chord> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    UiEvent::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Chord::from_event(*key, *modifiers),
                    _ => None,
                })
                .collect()
        });
        let now = Instant::now();
        let actions: Vec<Action> = chords
            .into_iter()
            .filter_map(|c| self.sequencer.feed(&self.keymap, c, now))
            .collect();
        for action in actions {
            // L'aide est modale : derrière elle, la grille ne bouge pas.
            if self.help && !action.works_over_overlay() {
                continue;
            }
            self.perform(ctx, action);
        }
    }

    /// Exécute une action (clavier aujourd'hui, souris ou plugin demain).
    fn perform(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
            Action::Help => self.help = !self.help,
            Action::Back => {
                if self.help {
                    self.help = false;
                } else if self.mode == Mode::View {
                    self.mode = Mode::Grid;
                    self.zoom = None;
                    self.zoom_anchor = None;
                    self.reveal = Some(Reveal::Minimal);
                }
            }
            Action::Open => {
                if self.mode == Mode::Grid && !self.files.is_empty() {
                    self.mode = Mode::View;
                }
            }
            Action::ToggleSource => {
                // Zoomé, on reste au même endroit : seul le 1:1 demandé change.
                self.source = self.source.toggled();
                self.request_zoom();
            }
            Action::ToggleZoom => {
                if self.mode == Mode::View {
                    self.zoom = match self.zoom {
                        Some(_) => None,
                        None => Some(vec2(0.5, 0.5)),
                    };
                    self.request_zoom();
                }
            }
            Action::PanLeft | Action::PanRight | Action::PanUp | Action::PanDown => {
                // Seulement dans une image zoomée ; sans effet ailleurs.
                if self.mode == Mode::View && self.zoom.is_some() {
                    let step = self.gui.view.pan_step;
                    self.pan += match action {
                        Action::PanLeft => vec2(-step, 0.0),
                        Action::PanRight => vec2(step, 0.0),
                        Action::PanUp => vec2(0.0, -step),
                        _ => vec2(0.0, step),
                    };
                }
            }
            // En plein écran, seule la navigation image par image a un sens.
            Action::NavUp | Action::NavDown | Action::NavPageUp | Action::NavPageDown
                if self.mode == Mode::View => {}
            Action::FilterCycle => {
                self.filter = match self.filter {
                    KindFilter::All => KindFilter::RawOnly,
                    KindFilter::RawOnly => KindFilter::FlatOnly,
                    KindFilter::FlatOnly => KindFilter::All,
                };
                self.apply_filter();
            }
            nav => {
                let page = self.layout.rows_in(self.view_height);
                let new = grid::navigate(self.selected, self.files.len(), &self.layout, page, nav);
                self.select(new);
            }
        }
    }

    fn select(&mut self, index: usize) {
        if index == self.selected || index >= self.files.len() {
            return;
        }
        self.selected = index;
        self.focus_since = Instant::now();
        if let Some(engine) = &self.engine {
            engine.scheduler.set_focus(index);
        }
        self.reveal = Some(Reveal::Minimal);
        self.request_zoom();
    }

    /// Demande une image au scheduler, une seule fois tant qu'elle n'est pas arrivée.
    fn request(&mut self, index: usize, level: Level) {
        if self.ready(index, level) {
            return;
        }
        let key = (self.files[index].path.clone(), level);
        if let Some(engine) = &self.engine
            && self.requested.insert(key)
        {
            trace(|| format!("demande au moteur : image {index} {level:?}"));
            engine.scheduler.request(index, level);
        }
    }

    /// Plein écran : demande au moteur ce que l'affichage va réclamer, **dans l'ordre des
    /// priorités** : image affichée ajustée (rendu RAW en mode RAW), son 1:1 si zoomé, puis
    /// les voisines (préchargement) dans le même ordre. L'ordre d'envoi compte : un worker
    /// libre prend aussitôt la première demande reçue, et un rendu LibRaw ne s'interrompt
    /// pas ; envoyer d'abord les 1:1 faisait passer un 100 % (~4,5 s) devant le rendu
    /// ajusté (~0,5 s).
    fn request_zoom(&mut self) {
        if self.mode != Mode::View || self.files.is_empty() {
            return;
        }
        let n = self.files.len();
        let current = self.selected;
        let images: Vec<usize> = [current, current + 1, current.wrapping_sub(1)]
            .into_iter()
            .filter(|&i| i < n)
            .collect();
        let mut wanted = Vec::new();
        for (rank, &i) in images.iter().enumerate() {
            let kind = self.files[i].kind;
            // Ajusté : en mode RAW, le rendu RAW (le JPEG boîtier est calculé en fond).
            if self.source == Source::Raw && kind == MediaKind::Raw {
                wanted.push((rank == 0, i, Level::Standard));
            }
            if self.zoom.is_some() {
                let level = view::zoom_level(kind, self.source);
                // 100 % RAW : seulement une fois qu'on s'est arrêté sur l'image.
                let expensive = level == Level::Full && kind == MediaKind::Raw;
                if !expensive || self.focus_since.elapsed() >= self.gui.view.full_raw_dwell() {
                    wanted.push((rank == 0, i, level));
                }
            }
        }
        // Image affichée d'abord (ajusté puis 1:1), voisines ensuite.
        wanted.sort_by_key(|&(focus, _, _)| !focus);
        for (_, i, level) in wanted {
            self.request(i, level);
        }
    }

    /// Vrai si le niveau est calculé (dans le cache) pour l'image `index`.
    fn ready(&self, index: usize, level: Level) -> bool {
        let s = &self.status[index];
        let avail = match level {
            Level::Thumbnail => &s.thumbnail,
            Level::Embedded => &s.embedded,
            Level::Standard => &s.standard,
            Level::EmbeddedFull => &s.embedded_full,
            Level::Full => &s.full,
        };
        *avail == Avail::Ready
    }

    /// Vrai si l'image `index` au niveau `level` est affichable tout de suite : texture
    /// chargée, ou vignette calculée (elle se charge en 1–2 ms, au moment de dessiner). La
    /// vignette sert ainsi toujours de solution d'attente : jamais d'écran noir en changeant
    /// d'image.
    fn loaded(&self, index: usize, level: Level) -> bool {
        let path = &self.files[index].path;
        match level {
            Level::Thumbnail => self.textures.contains(path) || self.ready(index, level),
            _ => self.big.contains(&(path.clone(), level)),
        }
    }

    /// Plein écran : intègre les images chargées et indique au chargeur ce qu'il faut
    /// préparer (l'image affichée, puis ses voisines).
    fn update_loads(&mut self, ctx: &egui::Context) {
        let Some(engine) = &self.engine else { return };
        for loaded in engine.loader.drain() {
            trace(|| {
                format!(
                    "texture reçue : {} {:?} {:?}",
                    file_name(&loaded.path),
                    loaded.level,
                    loaded.original
                )
            });
            let key = (loaded.path.clone(), loaded.level);
            self.wanted.retain(|w| *w != key);
            match loaded.image {
                Some(image) => {
                    let texture = ctx.load_texture(
                        format!("{}#{:?}", loaded.path.display(), loaded.level),
                        image,
                        TextureOptions::LINEAR,
                    );
                    let original = vec2(loaded.original[0] as f32, loaded.original[1] as f32);
                    let sensor_ratio = loaded.sensor_ratio;
                    self.big.insert(
                        key,
                        Big {
                            texture,
                            original,
                            sensor_ratio,
                        },
                    );
                }
                None => {
                    self.missing.insert(key);
                }
            }
        }
        if self.mode != Mode::View || self.files.is_empty() {
            return;
        }
        let n = self.files.len();
        let current = self.selected;
        // Demandes au moteur (dédoublonnées : seules les nouvelles partent).
        self.request_zoom();
        // Repasser ici à la fin de l'attente, pour lancer le 100 % RAW.
        if self.zoom.is_some() && self.source == Source::Raw {
            let left = self
                .gui
                .view
                .full_raw_dwell()
                .saturating_sub(self.focus_since.elapsed());
            if !left.is_zero() {
                ctx.request_repaint_after(left);
            }
        }
        let Some(engine) = &self.engine else { return };
        let neighbours = [current + 1, current.wrapping_sub(1)]
            .into_iter()
            .filter(|&i| i < n);
        let mut wanted = Vec::new();
        for (index, zoomed) in
            // Zoomé, les voisines aussi en 1:1 (préchargement).
            std::iter::once((current, self.zoom.is_some()))
                .chain(neighbours.map(|i| (i, self.zoom.is_some())))
        {
            let kind = self.files[index].kind;
            let level = view::pick(kind, self.source, zoomed, |l| self.ready(index, l));
            if let Some(level) = level.filter(|&l| l != Level::Thumbnail) {
                let key = (self.files[index].path.clone(), level);
                if !self.loaded(index, level) && !self.missing.contains(&key) {
                    wanted.push((self.files[index].clone(), level));
                }
            }
        }
        let keys: Vec<(PathBuf, Level)> =
            wanted.iter().map(|(f, l)| (f.path.clone(), *l)).collect();
        if keys != self.wanted {
            trace(|| format!("chargeur : {keys:?}"));
            self.wanted = keys;
            engine.loader.want(wanted);
        }
    }

    /// Plein écran : l'image sélectionnée, ajustée ou à 100 %, avec ce qui est affiché.
    fn viewer(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let index = self.selected;
        let file = self.files[index].clone();
        let zoomed = self.zoom.is_some();
        let desired = view::pick(file.kind, self.source, zoomed, |l| self.ready(index, l));
        // En attendant que l'image voulue soit chargée, la meilleure déjà affichable.
        let shown = view::pick(file.kind, self.source, zoomed, |l| {
            self.ready(index, l) && self.loaded(index, l)
        });
        if (index, shown) != self.last_shown {
            trace(|| format!("affichée : image {index} {shown:?} (voulue {desired:?})"));
            self.last_shown = (index, shown);
        }
        // En attente tant que l'image voulue n'est pas affichée, ou que le 1:1 demandé
        // n'est pas encore calculé.
        let zoom_level = view::zoom_level(file.kind, self.source);
        self.view_waiting =
            shown != desired || shown.is_none() || (zoomed && shown != Some(zoom_level));

        let (view_rect, response) =
            ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(view_rect);
        painter.rect_filled(view_rect, 0.0, self.gui.colors.view_background);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

        // Texture à afficher, et dimensions d'origine (pixels) de l'image qu'elle représente.
        let deadline = Instant::now() + self.gui.load_budget();
        let drawable: Option<(TextureHandle, Vec2, Option<f32>)> = match shown {
            Some(Level::Thumbnail) => self.thumbnail_texture(&ctx, index, deadline).map(|t| {
                let size = t.size_vec2();
                (t, size, None)
            }),
            Some(level) => self.big.get(&(file.path.clone(), level)).cloned().map(|b| {
                let ratio = b.sensor_ratio.filter(|_| level == Level::EmbeddedFull);
                (b.texture, b.original, ratio)
            }),
            None => None,
        };
        let mut image_rect = None;
        let mut sensor_ratio = None;
        if let Some((texture, original, ratio)) = drawable {
            sensor_ratio = ratio;
            // Taille du 1:1 en pixels : connue si c'est lui qui est affiché ; sinon estimée
            // d'après le dernier 1:1 de même niveau (même boîtier, souvent même taille). En
            // attendant le 1:1, l'aperçu est ainsi déjà affiché à l'échelle 1:1 (flou un
            // instant), sans repasser par la vue ajustée.
            let zoom_px = match (self.zoom, shown) {
                (Some(_), Some(level)) if level == zoom_level => {
                    self.last_zoom_size.insert(zoom_level, original);
                    Some(original)
                }
                (Some(_), _) => self
                    .last_zoom_size
                    .get(&zoom_level)
                    .map(|&last| view::estimate_zoom_size(last, original.x / original.y)),
                (None, _) => None,
            };
            let rect = match (zoom_px, self.zoom) {
                (Some(px), Some(center)) => {
                    // 1:1 : un pixel de l'image = un pixel de l'écran.
                    let size = px / ctx.pixels_per_point();
                    let mut center = match self.zoom_anchor.take() {
                        Some((point, pointer)) => {
                            view::center_keeping(view_rect, size, point, pointer)
                        }
                        None => center,
                    };
                    if response.dragged() {
                        let d = response.drag_delta();
                        center -= vec2(d.x / size.x, d.y / size.y);
                    }
                    // Déplacements clavier (fractions de la vue) convertis en fractions de
                    // l'image.
                    let pan = std::mem::take(&mut self.pan);
                    center += vec2(
                        pan.x * view_rect.width() / size.x,
                        pan.y * view_rect.height() / size.y,
                    );
                    let center = view::clamp_center(view_rect.size(), size, center);
                    self.zoom = Some(center);
                    view::zoom_rect(view_rect, size, center)
                }
                _ => view::fit_rect(view_rect, original),
            };
            painter.image(texture.id(), rect, uv, Color32::WHITE);
            image_rect = Some(rect);
        }

        // Double-clic : 100 % centré sur le point cliqué, ou retour à l'ajusté.
        if response.double_clicked() {
            self.zoom_anchor = None;
            self.zoom = match (self.zoom, image_rect, response.interact_pointer_pos()) {
                (Some(_), _, _) => None,
                (None, Some(rect), Some(pos)) => {
                    let point = view::relative(rect, pos);
                    self.zoom_anchor = Some((point, pos));
                    Some(point)
                }
                (None, _, _) => Some(vec2(0.5, 0.5)),
            };
            self.request_zoom();
        }

        // Ce qui est affiché, toujours visible.
        let mut info = format!(
            "{} / {} · {}",
            index + 1,
            self.files.len(),
            file_name(&file.path)
        );
        match shown {
            Some(level) => {
                info.push_str(" · ");
                info.push_str(view::label(file.kind, level));
                if let Some(ratio) = sensor_ratio {
                    info.push_str(&format!(" ({:.0} % du capteur)", ratio * 100.0));
                }
            }
            None => info.push_str(" · aperçu en cours de calcul"),
        }
        // Mode RAW, mais faute de mieux c'est encore le JPEG qui est affiché : le dire.
        let raw_pending = self.source == Source::Raw
            && file.kind == MediaKind::Raw
            && matches!(
                shown,
                Some(Level::Thumbnail | Level::Embedded | Level::EmbeddedFull)
            );
        if raw_pending {
            info.push_str(" · rendu RAW en cours");
        } else if zoomed && shown != Some(zoom_level) {
            info.push_str(match zoom_level {
                Level::Full => " · calcul du 100 % en cours",
                _ => " · chargement du 1:1",
            });
        }
        overlay(
            &painter,
            view_rect.left_top() + vec2(10.0, 10.0),
            Align2::LEFT_TOP,
            &info,
        );
        let hint = match file.kind {
            MediaKind::Raw => "b : JPEG boîtier / RAW   z : 1:1   Échap : grille",
            MediaKind::Flat => "z : 1:1   Échap : grille",
        };
        overlay(
            &painter,
            view_rect.right_top() + vec2(-10.0, 10.0),
            Align2::RIGHT_TOP,
            hint,
        );
    }

    /// Texture de la vignette `index`, chargée depuis le cache si besoin et si le budget de
    /// l'image le permet.
    fn thumbnail_texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        deadline: Instant,
    ) -> Option<TextureHandle> {
        let file = &self.files[index];
        if let Some(t) = self.textures.get(&file.path) {
            return Some(t.clone());
        }
        if self.status[index].thumbnail != Avail::Ready {
            return None;
        }
        if Instant::now() > deadline {
            self.visible_waiting += 1;
            ctx.request_repaint(); // on continuera à l'image suivante
            return None;
        }
        let engine = self.engine.as_ref()?;
        let key = preview::key(file, Level::Thumbnail, &engine.preview).ok()?;
        let bitmap = engine.cache.get(&key)?;
        let image = ColorImage::from_rgb(
            [bitmap.width as usize, bitmap.height as usize],
            &bitmap.data,
        );
        let texture = ctx.load_texture(file.path.to_string_lossy(), image, TextureOptions::LINEAR);
        self.textures.insert(file.path.clone(), texture.clone());
        Some(texture)
    }

    fn status_bar(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let raws = self
                .files
                .iter()
                .filter(|f| f.kind == MediaKind::Raw)
                .count();
            ui.label(folder_name(&self.dir));
            ui.separator();
            ui.label(format!(
                "{} images ({raws} RAW) · filtre : {}",
                self.files.len(),
                filter_name(self.filter)
            ));
            ui.separator();
            for (name, p) in [
                ("vignettes", self.progress[0]),
                ("JPEG boîtier", self.progress[1]),
                ("aperçus", self.progress[2]),
            ] {
                if p.total == 0 {
                    continue; // rien à calculer en fond pour ce niveau
                }
                ui.add(
                    egui::ProgressBar::new(p.fraction())
                        .desired_width(160.0)
                        .text(format!("{name} {}/{}", p.done, p.total)),
                );
            }
            if let Some(file) = self.files.get(self.selected) {
                ui.separator();
                ui.label(file_name(&file.path));
            }
            if let Some(warning) = &self.warning {
                // Le message de toml tient sur plusieurs lignes (extrait du fichier) : résumé
                // ici, détail au survol, en chasse fixe pour que le « ^^^ » reste aligné.
                ui.separator();
                ui.colored_label(
                    self.gui.colors.error,
                    "configuration invalide : réglages par défaut",
                )
                .on_hover_text(egui::RichText::new(warning).monospace());
            }
        });
    }

    fn grid(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let g = self.gui.grid;
        self.layout = Layout::fit(ui.available_width(), g.cell, g.gap);
        let rows = self.layout.rows(self.files.len());
        ui.spacing_mut().item_spacing = vec2(g.gap, g.gap);
        let mut area = ScrollArea::vertical().auto_shrink(false);
        let row = self.selected / self.layout.columns;
        let offset = match self.reveal.take() {
            Some(Reveal::Top) => Some(row as f32 * self.layout.row_pitch()),
            Some(Reveal::Minimal) => {
                grid::scroll_to_show(&self.layout, row, self.scroll_offset, self.view_height)
            }
            None => None,
        };
        if let Some(offset) = offset {
            area = area.vertical_scroll_offset(offset);
        }
        // Grille centrée : la marge restante est répartie des deux côtés.
        let margin = (ui.available_width() - self.layout.width()) / 2.0;
        let deadline = Instant::now() + self.gui.load_budget();
        self.visible_waiting = 0;
        let output = area.show_rows(ui, g.cell, rows, |ui, visible| {
            for row in visible {
                ui.horizontal(|ui| {
                    ui.add_space((margin - g.gap).max(0.0));
                    let start = row * self.layout.columns;
                    let end = (start + self.layout.columns).min(self.files.len());
                    for index in start..end {
                        self.cell(ui, &ctx, index, deadline);
                    }
                });
            }
        });
        self.scroll_offset = output.state.clamped_offset().y;
        self.view_height = output.inner_rect.height();
    }

    fn cell(&mut self, ui: &mut Ui, ctx: &egui::Context, index: usize, deadline: Instant) {
        let (g, colors) = (self.gui.grid, self.gui.colors);
        let (rect, response) = ui.allocate_exact_size(vec2(g.cell, g.cell), Sense::click());
        if response.clicked() {
            self.select(index);
        }
        if response.double_clicked() {
            self.select(index);
            self.mode = Mode::View;
        }
        let selected = index == self.selected;
        let visuals = ui.visuals();
        let background = if selected {
            colors
                .selection
                .map_or(visuals.selection.bg_fill, Color32::from)
        } else if response.hovered() {
            colors
                .hover
                .map_or(visuals.widgets.hovered.bg_fill, Color32::from)
        } else {
            visuals.extreme_bg_color
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, background);

        let image_area = Rect::from_min_max(
            rect.min + vec2(g.padding, g.padding),
            pos2(rect.max.x - g.padding, rect.max.y - g.label - 4.0),
        );
        let file = self.files[index].clone();
        match (
            self.thumbnail_texture(ctx, index, deadline),
            &self.status[index].thumbnail,
        ) {
            (Some(texture), _) => {
                let size = texture.size_vec2();
                let scale = (image_area.width() / size.x).min(image_area.height() / size.y);
                let dest = Rect::from_center_size(image_area.center(), size * scale);
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                ui.painter().image(texture.id(), dest, uv, Color32::WHITE);
            }
            (None, Avail::Failed(error)) => {
                ui.painter().text(
                    image_area.center(),
                    Align2::CENTER_CENTER,
                    "⚠",
                    FontId::proportional(28.0),
                    colors.warning.into(),
                );
                response.clone().on_hover_text(error.clone());
            }
            (None, _) => {} // en attente : cellule vide
        }

        let text_color = if selected {
            visuals_text(ui, true)
        } else {
            visuals_text(ui, false)
        };
        let badge = match file.kind {
            MediaKind::Raw => "RAW",
            MediaKind::Flat => "",
        };
        ui.painter().text(
            pos2(rect.min.x + g.padding, rect.max.y - 4.0),
            Align2::LEFT_BOTTOM,
            truncate(&file_name(&file.path), g.name_chars),
            FontId::proportional(12.0),
            text_color,
        );
        ui.painter().text(
            pos2(rect.max.x - g.padding, rect.max.y - 4.0),
            Align2::RIGHT_BOTTOM,
            badge,
            FontId::monospace(11.0),
            colors.badge.into(),
        );
    }

    /// Script de démarrage (développement) : exécute les actions jusqu'à la prochaine pause.
    fn run_script(&mut self, ctx: &egui::Context) {
        if let Some(until) = self.script_wait {
            if Instant::now() < until {
                ctx.request_repaint_after(until - Instant::now());
                return;
            }
            self.script_wait = None;
        }
        while let Some(step) = self.startup_actions.pop_front() {
            match step {
                Step::Do(action) => {
                    trace(|| format!("script : {}", action.id()));
                    self.perform(ctx, action);
                }
                Step::Wait(d) => {
                    self.script_wait = Some(Instant::now() + d);
                    ctx.request_repaint_after(d);
                    return;
                }
                Step::Shot => {
                    if let Some(shot) = &mut self.screenshot {
                        shot.forced = true;
                    }
                    return;
                }
            }
        }
    }

    /// Liste des actions, de leurs identifiants et de leurs touches.
    fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = self.help;
        egui::Window::new("Raccourcis")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Grid::new("help").striped(true).show(ui, |ui| {
                    for action in Action::ALL {
                        // Identifiant technique au survol seulement (utile pour le futur
                        // fichier de raccourcis).
                        ui.label(action.label()).on_hover_text(action.id());
                        // Chasse fixe : « l » et « I » restent distincts.
                        ui.label(
                            egui::RichText::new(self.keymap.keys_for(action).join(", "))
                                .monospace()
                                .strong(),
                        );
                        ui.end_row();
                    }
                });
                ui.add_space(6.0);
                ui.weak("Échap pour fermer.");
            });
        self.help = open;
    }

    /// Capture automatique (développement) : une fois toutes les vignettes calculées et
    /// celles qui sont visibles affichées, ou après 60 s.
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else {
            return;
        };
        if shot.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let ready = self.progress[0].total > 0
            && self.progress[0].done == self.progress[0].total
            && !(self.mode == Mode::View && self.view_waiting);
        let timeout = shot.started.elapsed() > Duration::from_secs(60);
        // Au moins une seconde : laisser finir les animations (fondu d'ouverture des
        // fenêtres), sinon la capture montre un état intermédiaire.
        let settled = shot.started.elapsed() > Duration::from_secs(1);
        let script_done = self.startup_actions.is_empty() && self.script_wait.is_none();
        let automatic = ready && self.visible_waiting == 0 && settled && script_done;
        if !shot.requested && (automatic || shot.forced || timeout) {
            shot.requested = true;
            let (path, done) = (shot.path.clone(), shot.done.clone());
            let repaint = ctx.clone();
            ctx.request_screenshot(move |image| {
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|c| c.to_srgba_unmultiplied())
                    .collect();
                let [w, h] = image.size;
                if let Err(e) = image::save_buffer(
                    &path,
                    &rgba,
                    w as u32,
                    h as u32,
                    image::ExtendedColorType::Rgba8,
                ) {
                    eprintln!("capture impossible : {e}");
                }
                done.store(true, Ordering::SeqCst);
                repaint.request_repaint();
            });
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl eframe::App for TriApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.engine.is_none() && self.error.is_none() {
            self.start_engine(&ctx);
        }
        self.drain_events();
        self.run_script(&ctx);
        self.handle_keys(&ctx);
        self.update_loads(&ctx);

        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(self.gui.colors.error, error);
            } else if self.files.is_empty() {
                ui.label("Aucune image dans ce dossier (ou pour ce filtre : touche f).");
            } else if self.mode == Mode::View {
                self.viewer(ui);
            } else {
                self.grid(ui);
            }
        });
        if self.help {
            self.help_window(&ctx);
        }
        self.handle_screenshot(&ctx);
    }
}

/// Actions de `TRI_GUI_ACTIONS` (identifiants séparés par des virgules, ex.
/// `core.nav.next,core.app.help`) ; les identifiants inconnus sont signalés et ignorés.
/// Étape du script de démarrage (`TRI_GUI_ACTIONS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Exécuter une action.
    Do(Action),
    /// Attendre (`wait:<ms>`), pour laisser le moteur travailler entre deux actions.
    Wait(Duration),
    /// Capturer tout de suite (`shot`), sans attendre que tout soit prêt : pour voir un
    /// état intermédiaire (chargement en cours).
    Shot,
}

fn startup_actions() -> Vec<Step> {
    let Ok(list) = std::env::var("TRI_GUI_ACTIONS") else {
        return Vec::new();
    };
    parse_actions(&list)
}

fn parse_actions(list: &str) -> Vec<Step> {
    list.split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter_map(|id| {
            let step = match id.strip_prefix("wait:") {
                Some(ms) => ms
                    .parse()
                    .ok()
                    .map(|ms| Step::Wait(Duration::from_millis(ms))),
                None if id == "shot" => Some(Step::Shot),
                None => Action::from_id(id).map(Step::Do),
            };
            if step.is_none() {
                eprintln!("TRI_GUI_ACTIONS : étape inconnue « {id} »");
            }
            step
        })
        .collect()
}

/// Position d'un niveau dans `TriApp::progress`, `None` pour le 1:1 (non suivi).
fn tracked(level: Level) -> Option<usize> {
    match level {
        Level::Thumbnail => Some(0),
        Level::Embedded => Some(1),
        Level::Standard => Some(2),
        Level::EmbeddedFull | Level::Full => None,
    }
}

fn set(status: &mut [Status], index: usize, level: Level, avail: Avail) {
    if let Some(s) = status.get_mut(index) {
        match level {
            Level::Thumbnail => s.thumbnail = avail,
            Level::Embedded => s.embedded = avail,
            Level::Standard => s.standard = avail,
            Level::EmbeddedFull => s.embedded_full = avail,
            Level::Full => s.full = avail,
        }
    }
}

/// Texte lisible sur n'importe quelle image : fond sombre translucide.
fn overlay(painter: &egui::Painter, pos: egui::Pos2, anchor: Align2, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        FontId::proportional(13.0),
        Color32::from_gray(230),
    );
    let rect = anchor.anchor_size(pos, galley.size()).expand(5.0);
    painter.rect_filled(rect, 4.0, Color32::from_black_alpha(170));
    painter.galley(rect.min + vec2(5.0, 5.0), galley, Color32::WHITE);
}

fn visuals_text(ui: &Ui, strong: bool) -> Color32 {
    if strong {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn folder_name(path: &Path) -> String {
    file_name(path)
}

fn filter_name(filter: KindFilter) -> &'static str {
    match filter {
        KindFilter::All => "tout",
        KindFilter::RawOnly => "RAW",
        KindFilter::FlatOnly => "plates",
    }
}

/// Tronque `s` à `max` caractères (« … » en fin).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Application sur un dossier temporaire de 3 JPEG, avec un cache temporaire.
    fn harness() -> (Harness<'static, TriApp>, tempfile::TempDir) {
        harness_with(64, 48)
    }

    /// Application sur 3 JPEG de `w`×`h` pixels, avec un cache temporaire.
    fn harness_with(w: u32, h: u32) -> (Harness<'static, TriApp>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        for i in 0..3 {
            image::RgbImage::from_pixel(w, h, image::Rgb([40 * i as u8, 100, 160]))
                .save(photos.join(format!("img{i}.jpg")))
                .unwrap();
        }
        let mut settings = AppSettings::default();
        settings.cache.dir = Some(dir.path().join("cache"));
        let harness = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(photos, settings, None));
        (harness, dir)
    }

    fn press(h: &mut Harness<'_, TriApp>, key: Key, modifiers: Modifiers) {
        h.key_press_modifiers(modifiers, key);
        h.run_steps(3);
    }

    #[test]
    fn help_opens_and_closes_with_its_keys() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert!(!h.state().help);
        for (key, modifiers) in [
            (Key::F1, Modifiers::NONE),
            (Key::Questionmark, Modifiers::NONE),
            (Key::Questionmark, Modifiers::SHIFT),
        ] {
            press(&mut h, key, modifiers);
            assert!(h.state().help, "{key:?} {modifiers:?} doit ouvrir l'aide");
            press(&mut h, key, modifiers);
            assert!(
                !h.state().help,
                "{key:?} {modifiers:?} doit refermer l'aide"
            );
        }
    }

    #[test]
    fn help_is_modal() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert_eq!(h.state().selected, 0);
        press(&mut h, Key::F1, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(
            h.state().selected,
            0,
            "la grille ne doit pas bouger sous l'aide"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 1);
    }

    #[test]
    fn help_shows_labels_not_ids() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.query_by_label("Image suivante").is_some());
        assert!(h.query_by_label("Flèche droite, l").is_some());
        assert!(h.query_by_label("core.nav.next").is_none());
    }

    #[test]
    fn escape_closes_help() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.state().help);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().help, "Échap doit fermer l'aide");
    }

    /// Fait tourner l'interface jusqu'à ce que `done` soit vrai (le moteur travaille dans
    /// d'autres threads), au plus 20 s.
    fn run_until(h: &mut Harness<'_, TriApp>, done: impl Fn(&TriApp) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(h.state()) {
            assert!(Instant::now() < deadline, "délai dépassé");
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn enter_opens_fullscreen_and_escape_goes_back() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::Grid);
    }

    #[test]
    fn fullscreen_navigates_image_by_image() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Enter, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 1);
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.state().selected, 1, "pas de lignes en plein écran");
        press(&mut h, Key::H, Modifiers::NONE);
        assert_eq!(h.state().selected, 0);
    }

    #[test]
    fn fullscreen_shows_the_standard_preview_once_loaded() {
        let (mut h, _dir) = harness();
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| {
            let path = app.files[0].path.clone();
            app.big.contains(&(path, Level::Standard)) && !app.view_waiting
        });
    }

    #[test]
    fn source_toggles_between_embedded_and_raw() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert_eq!(
            h.state().source,
            Source::Embedded,
            "JPEG boîtier par défaut"
        );
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().source, Source::Raw);
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().source, Source::Embedded);
    }

    #[test]
    fn zoom_computes_and_displays_the_full_image() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Z, Modifiers::NONE);
        assert!(h.state().zoom.is_none(), "pas de zoom dans la grille");
        press(&mut h, Key::Enter, Modifiers::NONE);
        press(&mut h, Key::Z, Modifiers::NONE);
        assert!(h.state().zoom.is_some());
        run_until(&mut h, |app| {
            let path = app.files[0].path.clone();
            app.status[0].full == Avail::Ready
                && app.big.contains(&(path, Level::Full))
                && !app.view_waiting
        });
        // Image voisine : le zoom est conservé et son 100 % est demandé à son tour.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert!(h.state().zoom.is_some());
        run_until(&mut h, |app| app.status[1].full == Avail::Ready);
        press(&mut h, Key::Z, Modifiers::NONE);
        assert!(h.state().zoom.is_none());
    }

    #[test]
    fn switching_source_keeps_the_zoom_and_the_spot() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Enter, Modifiers::NONE);
        press(&mut h, Key::Z, Modifiers::NONE);
        h.state_mut().zoom = Some(vec2(0.3, 0.6));
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().source, Source::Raw);
        let zoom = h.state().zoom.expect("toujours zoomé");
        // Image plate de 64×48 plus petite que la vue : le centre est ramené au milieu,
        // mais le zoom reste actif.
        assert!(zoom.x >= 0.0 && zoom.y >= 0.0);
        press(&mut h, Key::B, Modifiers::NONE);
        assert!(h.state().zoom.is_some());
    }

    #[test]
    fn gg_goes_to_the_first_image_and_shift_g_to_the_last() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::G, Modifiers::SHIFT);
        assert_eq!(h.state().selected, 2);
        press(&mut h, Key::G, Modifiers::NONE);
        assert_eq!(h.state().selected, 2, "un seul g : on attend la suite");
        press(&mut h, Key::G, Modifiers::NONE);
        assert_eq!(h.state().selected, 0);
    }

    #[test]
    fn shift_arrows_and_shift_hjkl_move_inside_a_zoomed_image() {
        // Image bien plus grande que la vue (1000×700), pour pouvoir se déplacer.
        let (mut h, _dir) = harness_with(2000, 1400);
        h.run_steps(3);
        press(&mut h, Key::ArrowRight, Modifiers::SHIFT);
        assert!(h.state().zoom.is_none(), "sans zoom : sans effet");
        press(&mut h, Key::Enter, Modifiers::NONE);
        press(&mut h, Key::Z, Modifiers::NONE);
        run_until(&mut h, |app| {
            let path = app.files[0].path.clone();
            app.big.contains(&(path, Level::Full))
        });
        h.run_steps(2);
        let start = h.state().zoom.unwrap();
        press(&mut h, Key::ArrowRight, Modifiers::SHIFT);
        let right = h.state().zoom.unwrap();
        assert!(right.x > start.x, "vers la droite : {start:?} -> {right:?}");
        assert_eq!(right.y, start.y);
        press(&mut h, Key::J, Modifiers::SHIFT);
        let down = h.state().zoom.unwrap();
        assert!(down.y > right.y, "vers le bas : {right:?} -> {down:?}");
        press(&mut h, Key::H, Modifiers::SHIFT);
        press(&mut h, Key::K, Modifiers::SHIFT);
        let back = h.state().zoom.unwrap();
        assert!((back - start).length() < 1e-4, "retour au point de départ");
        assert_eq!(h.state().selected, 0, "se déplacer ne change pas d'image");
    }

    #[test]
    fn startup_actions_are_parsed_by_id() {
        assert_eq!(
            parse_actions(" core.nav.next, wait:250, core.app.help ,,inconnue,wait:x"),
            [
                Step::Do(Action::NavNext),
                Step::Wait(Duration::from_millis(250)),
                Step::Do(Action::Help)
            ]
        );
        assert!(parse_actions("").is_empty());
        assert_eq!(parse_actions("shot"), [Step::Shot]);
    }

    #[test]
    fn truncate_counts_characters_not_bytes() {
        assert_eq!(truncate("DSCF7171.RAF", 24), "DSCF7171.RAF");
        assert_eq!(truncate("éééééé", 4), "ééé…");
    }

    #[test]
    fn status_updates_by_level() {
        let mut s = vec![Status::default(); 2];
        set(&mut s, 1, Level::Thumbnail, Avail::Ready);
        set(&mut s, 1, Level::Standard, Avail::Failed("x".into()));
        set(&mut s, 9, Level::Thumbnail, Avail::Ready); // hors limites : ignoré
        assert_eq!(s[0].thumbnail, Avail::Pending);
        assert_eq!(s[1].thumbnail, Avail::Ready);
        assert_eq!(s[1].standard, Avail::Failed("x".into()));
    }
}
