//! L'application egui : grille de vignettes alimentée par le scheduler du moteur.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, ColorImage, Event as UiEvent, FontId, Rect, ScrollArea, Sense,
    TextureHandle, TextureOptions, Ui, ViewportCommand, pos2, vec2,
};
use tri::cache::{Cache, Level};
use tri::filter::{Filter, KindFilter, ungrouped};
use tri::media::{MediaFile, MediaKind};
use tri::preview::{self, Sizes};
use tri::scheduler::{CachedPreviews, Config, Event, Progress, Scheduler};

use crate::actions::{Action, Keymap};
use crate::grid::{self, Layout};
use crate::textures::Lru;

/// Côté d'une cellule de la grille (points).
const CELL: f32 = 200.0;
/// Espace entre les cellules (points).
const GAP: f32 = 8.0;
/// Hauteur réservée au nom du fichier sous la vignette.
const LABEL: f32 = 18.0;
/// Textures de vignettes gardées en mémoire au plus.
const TEXTURE_CAPACITY: usize = 600;
/// Temps maximal passé à charger des vignettes par image affichée : au-delà, on reprend à
/// l'image suivante pour ne pas figer l'interface.
const LOAD_BUDGET: Duration = Duration::from_millis(8);

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
}

/// Le moteur, démarré à la première image (il faut connaître la taille de l'écran).
struct Engine {
    scheduler: Scheduler,
    events: Receiver<Event>,
    generation: u64,
    cache: Cache,
    sizes: Sizes,
}

/// Capture d'écran automatique pour le développement (`TRI_GUI_SCREENSHOT=<fichier.png>`) :
/// prise une fois toutes les vignettes prêtes, puis l'application se ferme.
struct Screenshot {
    path: PathBuf,
    requested: bool,
    done: Arc<AtomicBool>,
    started: Instant,
}

/// État de l'application.
pub struct TriApp {
    dir: PathBuf,
    /// Dossier du cache ; `None` : emplacement par défaut du système.
    cache_dir: Option<PathBuf>,
    all: Vec<MediaFile>,
    filter: KindFilter,
    files: Vec<MediaFile>,
    status: Vec<Status>,
    progress: [Progress; 3],
    engine: Option<Engine>,
    error: Option<String>,
    keymap: Keymap,
    textures: Lru<PathBuf, TextureHandle>,
    selected: usize,
    layout: Layout,
    scroll_offset: f32,
    view_height: f32,
    reveal: Option<Reveal>,
    screenshot: Option<Screenshot>,
    help: bool,
    /// Actions à exécuter à la première image (`TRI_GUI_ACTIONS`, développement).
    startup_actions: Vec<Action>,
    /// Vignettes visibles prêtes mais pas encore chargées (budget de l'image dépassé).
    visible_waiting: usize,
}

impl TriApp {
    /// Application sur le dossier `dir` (parcouru tout de suite, calculs lancés à la première
    /// image), avec le cache dans `cache_dir` (`None` : emplacement par défaut).
    pub fn new(dir: PathBuf, cache_dir: Option<PathBuf>) -> TriApp {
        let (all, error) = match tri::media::scan(&dir, 1) {
            Ok(files) => (files, None),
            Err(e) => (Vec::new(), Some(format!("{} : {e}", dir.display()))),
        };
        let screenshot = std::env::var_os("TRI_GUI_SCREENSHOT").map(|p| Screenshot {
            path: p.into(),
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
            cache_dir,
            all,
            filter: KindFilter::All,
            files: Vec::new(),
            status: Vec::new(),
            progress: [Progress::default(); 3],
            engine: None,
            error,
            keymap: Keymap::default(),
            textures: Lru::new(TEXTURE_CAPACITY),
            selected,
            layout: Layout::fit(0.0, CELL, GAP),
            scroll_offset: 0.0,
            view_height: 0.0,
            reveal: None,
            screenshot,
            help: false,
            visible_waiting: 0,
            startup_actions: startup_actions(),
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

    /// Démarre le moteur : taille d'aperçu « Auto » = plus grand côté de l'écran.
    fn start_engine(&mut self, ctx: &egui::Context) {
        let monitor = ctx.input(|i| i.viewport().monitor_size);
        let ppp = ctx.pixels_per_point();
        let sizes = Sizes {
            standard: monitor.map_or(2560, |m| (m.x.max(m.y) * ppp).round() as u32),
            ..Sizes::default()
        };
        let cache = match self
            .cache_dir
            .as_ref()
            .map_or_else(Cache::open_default, Cache::open)
        {
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
            sizes,
        };
        let scheduler = Scheduler::new(producer, Config::default(), move |event| {
            let _ = tx.send(event);
            repaint.request_repaint();
        });
        let generation = scheduler.set_folder(self.files.clone());
        scheduler.set_focus(self.selected);
        self.engine = Some(Engine {
            scheduler,
            events: rx,
            generation,
            cache,
            sizes,
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
                } if g == generation => set(&mut self.status, index, level, Avail::Ready),
                Event::Failed {
                    generation: g,
                    index,
                    level,
                    error,
                    ..
                } if g == generation => set(&mut self.status, index, level, Avail::Failed(error)),
                Event::Progress {
                    generation: g,
                    level,
                    progress,
                } if g == generation => {
                    if let Some(i) = tracked(level) {
                        self.progress[i] = progress;
                    }
                }
                _ => {} // ancien dossier, ou niveau non suivi
            }
        }
    }

    /// Traduit les touches pressées en actions et les applique.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        let actions: Vec<Action> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    UiEvent::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => self.keymap.action(*key, *modifiers),
                    _ => None,
                })
                .collect()
        });
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
            Action::Back => self.help = false, // le plein écran s'y ajoutera
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
        if let Some(engine) = &self.engine {
            engine.scheduler.set_focus(index);
        }
        self.reveal = Some(Reveal::Minimal);
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
        let key = preview::key(file, Level::Thumbnail, &engine.sizes).ok()?;
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
                if p.total == 0 && name == "JPEG boîtier" {
                    continue; // aucun RAW affiché
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
        });
    }

    fn grid(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.layout = Layout::fit(ui.available_width(), CELL, GAP);
        let rows = self.layout.rows(self.files.len());
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
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
        let deadline = Instant::now() + LOAD_BUDGET;
        self.visible_waiting = 0;
        let output = area.show_rows(ui, CELL, rows, |ui, visible| {
            for row in visible {
                ui.horizontal(|ui| {
                    ui.add_space((margin - GAP).max(0.0));
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
        let (rect, response) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::click());
        if response.clicked() {
            self.select(index);
        }
        let selected = index == self.selected;
        let visuals = ui.visuals();
        let background = if selected {
            visuals.selection.bg_fill
        } else if response.hovered() {
            visuals.widgets.hovered.bg_fill
        } else {
            visuals.extreme_bg_color
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, background);

        let image_area = Rect::from_min_max(
            rect.min + vec2(6.0, 6.0),
            pos2(rect.max.x - 6.0, rect.max.y - LABEL - 4.0),
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
                    Color32::from_rgb(230, 160, 60),
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
            pos2(rect.min.x + 6.0, rect.max.y - 4.0),
            Align2::LEFT_BOTTOM,
            truncate(&file_name(&file.path), 24),
            FontId::proportional(12.0),
            text_color,
        );
        ui.painter().text(
            pos2(rect.max.x - 6.0, rect.max.y - 4.0),
            Align2::RIGHT_BOTTOM,
            badge,
            FontId::monospace(11.0),
            Color32::from_rgb(120, 170, 230),
        );
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
        let ready = self.progress[0].total > 0 && self.progress[0].done == self.progress[0].total;
        let timeout = shot.started.elapsed() > Duration::from_secs(60);
        // Au moins une seconde : laisser finir les animations (fondu d'ouverture des
        // fenêtres), sinon la capture montre un état intermédiaire.
        let settled = shot.started.elapsed() > Duration::from_secs(1);
        if !shot.requested && ((ready && self.visible_waiting == 0 && settled) || timeout) {
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
        for action in std::mem::take(&mut self.startup_actions) {
            self.perform(&ctx, action);
        }
        self.handle_keys(&ctx);

        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(Color32::from_rgb(230, 90, 90), error);
            } else if self.files.is_empty() {
                ui.label("Aucune image dans ce dossier (ou pour ce filtre : touche f).");
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
fn startup_actions() -> Vec<Action> {
    let Ok(list) = std::env::var("TRI_GUI_ACTIONS") else {
        return Vec::new();
    };
    parse_actions(&list)
}

fn parse_actions(list: &str) -> Vec<Action> {
    list.split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter_map(|id| {
            let action = Action::from_id(id);
            if action.is_none() {
                eprintln!("TRI_GUI_ACTIONS : action inconnue « {id} »");
            }
            action
        })
        .collect()
}

/// Position d'un niveau dans `TriApp::progress`, `None` pour le 1:1 (non suivi).
fn tracked(level: Level) -> Option<usize> {
    match level {
        Level::Thumbnail => Some(0),
        Level::Embedded => Some(1),
        Level::Standard => Some(2),
        Level::Full => None,
    }
}

fn set(status: &mut [Status], index: usize, level: Level, avail: Avail) {
    if let Some(s) = status.get_mut(index) {
        match level {
            Level::Thumbnail => s.thumbnail = avail,
            Level::Embedded => s.embedded = avail,
            Level::Standard => s.standard = avail,
            Level::Full => {}
        }
    }
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
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        for i in 0..3 {
            image::RgbImage::from_pixel(64, 48, image::Rgb([40 * i, 100, 160]))
                .save(photos.join(format!("img{i}.jpg")))
                .unwrap();
        }
        let cache = dir.path().join("cache");
        let harness = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(photos, Some(cache)));
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

    #[test]
    fn startup_actions_are_parsed_by_id() {
        assert_eq!(
            parse_actions(" core.nav.next, core.app.help ,,inconnue"),
            [Action::NavNext, Action::Help]
        );
        assert!(parse_actions("").is_empty());
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
