//! L'application egui : grille de vignettes et plein écran, alimentés par le scheduler du
//! moteur.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Rect, ScrollArea, Sense, TextureHandle,
    TextureOptions, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use tri::cache::{Cache, Level};
use tri::exposure::Exposure;
use tri::filter::{Filter, KindFilter};
use tri::folder::Folder;
use tri::media::{MediaFile, MediaKind};
use tri::notes::{ColorRule, NotesRule, StarsRule};
use tri::places::{Monitor, Place, PlaceKind, PlaceSettings, Tree};
use tri::preview;
use tri::scheduler::{CachedPreviews, Event, Progress, Scheduler};
use tri::settings::EngineSettings;
use tri::xmp::{Color, Flag};

use crate::actions::{Action, FlagFilter, Group, Keymap, Sequencer};
use crate::grid::{self, Layout};
use crate::histogram;
use crate::loader::Loader;
use crate::marks;
use crate::meta::MetaLoader;
use crate::offload::{OffloadDialog, Request as OffloadRequest};
use crate::settings::{AppSettings, GuiSettings, HistogramScale};
use crate::textures::Lru;
use crate::trace::trace;
use crate::view::{self, Source};

/// Ce qui occupe la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Grid,
    View,
    /// Comparaison côte à côte des images de `TriApp::compare`.
    Compare,
}

/// État de la carte de netteté pour l'image affichée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Peaking {
    Shown,
    Computing,
    NotAtOneToOne,
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
    exposure: Avail,
    sharpness: Avail,
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
    /// Dossier ouvert : images et notes (moteur).
    folder: Folder,
    /// Arborescence du panneau des dossiers (moteur).
    tree: Tree,
    /// Panneau des dossiers affiché.
    folder_panel: bool,
    /// Faire défiler le panneau jusqu'au dossier ouvert (au démarrage, à l'ouverture).
    reveal_folder: bool,
    /// Réglages des emplacements, pour démarrer la surveillance des volumes.
    places: PlaceSettings,
    /// Surveillance des volumes (moteur), démarrée à la première image, et la liste
    /// qu'elle envoie quand un disque est branché ou débranché.
    volumes: Option<(Monitor, Receiver<Vec<Place>>)>,
    /// Boîte de déchargement d'une carte (éventuellement en arrière-plan).
    offload: Option<OffloadDialog>,
    /// Séance déchargée en arrière-plan, proposée à l'ouverture dans la barre d'état.
    offloaded: Option<PathBuf>,
    /// Volume dont l'éjection a été refusée : « Réessayer l'éjection ».
    eject_failed: Option<PathBuf>,
    /// Volume dont l'éjection attend confirmation.
    eject_dialog: Option<(PathBuf, egui::Pos2)>,
    /// Éjection en cours (dans un fil à part) : volume, et réponse du système à venir.
    ejecting: Option<(PathBuf, Receiver<Result<(), String>>)>,
    /// Message pour la barre d'état (texte, vrai si c'est une erreur).
    notice: Option<(String, bool)>,
    /// Réglages du moteur (fichier de configuration).
    settings: EngineSettings,
    /// Réglages de l'interface (section `[gui]`).
    gui: GuiSettings,
    /// Problème de configuration à signaler (fichier invalide...), dans la barre d'état.
    warning: Option<String>,
    /// Problèmes dans la section `[keys]` (touche inconnue, conflit...), dans la barre
    /// d'état.
    key_warnings: Vec<String>,
    /// Dernière note impossible à écrire (sidecar mal formé, disque en lecture seule...).
    note_error: Option<String>,
    filter: KindFilter,
    /// Filtres sur les notes (étoiles, drapeau, couleurs).
    notes_rule: NotesRule,
    /// Barre de filtres affichée (grille).
    filter_bar: bool,
    /// Bandeau de notation affiché.
    rating_bar: bool,
    /// Avance automatique après une note (`[gui] auto_advance`, Maj+A).
    auto_advance: bool,
    /// Boîte de notation ouverte par un clic droit : l'image cliquée. Modale : derrière
    /// elle, rien ne change de sélection.
    rating_dialog: Option<(PathBuf, egui::Pos2)>,
    /// Scores de netteté connus (cache d'analyse), par chemin.
    scores: HashMap<PathBuf, f32>,
    /// Jugement de chaque photo notée (dossier et voisines, voir `tri::sharpness::judge`).
    judgements: HashMap<PathBuf, tri::sharpness::Judgement>,
    /// Filtre « masquer les floues » (`f b`).
    blur_rule: Option<tri::sharpness::BlurRule>,
    /// Photos jugées floues au dernier filtrage (refiltrer seulement si ça change).
    blurry_applied: HashSet<PathBuf>,
    /// Carte de netteté affichée en 1:1 (`p`).
    peaking: bool,
    /// Collimateur de mise au point affiché (`m`).
    focus_point: bool,
    /// 1:1 demandé avant que les métadonnées de cette image soient lues : à recentrer sur
    /// son collimateur dès qu'elles arrivent.
    zoom_focus_pending: Option<PathBuf>,
    /// Dossier de données (dernier dossier, état d'affichage).
    data_dir: Option<PathBuf>,
    /// État d'affichage tel qu'écrit la dernière fois (`state.rs`).
    display_saved: crate::state::DisplayState,
    /// Couleur de la carte : indice dans `[gui.sharpness] colors` (`Ctrl+P`).
    peaking_color: usize,
    /// Carte de netteté chargée (une seule : celle de l'image affichée).
    peaking_texture: Option<((PathBuf, Level), TextureHandle)>,
    /// Carte demandée au chargeur, pas encore reçue.
    peaking_asked: Option<(PathBuf, Level)>,
    /// Images comparées côte à côte (`c`), dans l'ordre de la liste.
    compare: Vec<PathBuf>,
    /// Colonnes de la dernière disposition de la comparaison (navigation haut / bas).
    compare_cols: usize,
    /// Indication passagère dans la barre d'état (effacée à la touche suivante).
    hint: Option<String>,
    /// Sélection multiple (images marquées, mode visuel).
    selection: tri::selection::Selection,
    /// Boîte « Renommer » ou « Corbeille » ouverte.
    file_dialog: Option<crate::files::FileDialog>,
    /// Échec de la dernière action sur un fichier (barre d'état).
    file_error: Option<String>,
    files: Vec<MediaFile>,
    status: Vec<Status>,
    progress: [Progress; 5],
    engine: Option<Engine>,
    error: Option<String>,
    keymap: Keymap,
    /// Séquences de touches en cours (`g g`).
    sequencer: Sequencer,
    /// La souris est l'outil en cours : vrai dès qu'elle bouge, faux dès qu'on tape. Le
    /// pointeur est masqué quand c'est le clavier.
    mouse_active: bool,
    /// Vignette survolée par la souris (encadrée, pas sélectionnée).
    hovered: Option<usize>,
    /// Rectangles des cellules dessinées (pour les tests).
    #[cfg(test)]
    cell_rects: Vec<(usize, Rect)>,
    /// « Quitter » demandé (tests : le harnais ne ferme rien).
    #[cfg(test)]
    quit_requested: bool,
    /// Déplacements clavier d'une image zoomée pas encore appliqués, en fractions de la
    /// vue (appliqués au dessin, quand la taille de l'image est connue).
    pan: Vec2,
    textures: Lru<PathBuf, TextureHandle>,
    /// Panneau des métadonnées affiché.
    metadata_panel: bool,
    /// Lecture des métadonnées en fond (démarrée à la première image).
    meta: Option<MetaLoader>,
    /// Histogramme affiché (en haut du panneau des métadonnées).
    histogram: bool,
    /// Grand histogramme, en boîte modale au milieu de l'écran.
    histogram_large: bool,
    /// Échelle de l'histogramme.
    histogram_scale: HistogramScale,
    /// Grand histogramme : un seul canal (0 rouge, 1 vert, 2 bleu) ; `None` : tous.
    histogram_channel: Option<usize>,
    /// Image d'analyse du grand histogramme, pour (image, canal).
    analysis_texture: Option<((PathBuf, Option<usize>), TextureHandle)>,
    /// Zones saturées / bouchées superposées à l'image.
    clipping: bool,
    /// Analyses d'exposition lues dans le cache (images affichées récemment).
    exposures: Lru<PathBuf, Arc<Exposure>>,
    /// Masques des zones, en textures.
    mask_textures: Lru<PathBuf, TextureHandle>,
    /// Parts de pixels saturés / bouchés par image, pour les badges de la grille.
    clip_summary: std::collections::HashMap<PathBuf, (f64, f64)>,
    selected: usize,
    layout: Layout,
    scroll_offset: f32,
    view_height: f32,
    reveal: Option<Reveal>,
    screenshot: Option<Screenshot>,
    help: bool,
    /// Historique des modifications de la session (annuler, rétablir).
    history: tri::history::History,
    /// Liste de l'historique ouverte (`Ctrl+H`).
    history_open: bool,
    /// Fenêtre de réglages ouverte.
    settings_window: Option<crate::settings_window::SettingsWindow>,
    /// Fichier de configuration modifié par la fenêtre de réglages.
    config_path: Option<PathBuf>,
    /// Recherche dans l'aide (`None` : aide fermée, le champ prendra le focus à
    /// l'ouverture).
    help_search: Option<String>,
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
    /// Fichier de configuration que la fenêtre de réglages modifie.
    pub fn with_config(mut self, path: Option<PathBuf>) -> TriApp {
        self.config_path = path;
        self
    }

    /// Applique des réglages modifiés dans la fenêtre de réglages : interface, raccourcis,
    /// langue, fichiers, notes, déchargement. Le moteur (aperçus, cache, calculs) garde les
    /// siens jusqu'au prochain lancement.
    fn apply_settings(&mut self, ctx: &egui::Context, new: AppSettings) {
        if new.i18n.language != self.settings.i18n.language {
            crate::i18n::init(new.i18n.language);
        }
        if new.gui.theme != self.gui.theme {
            ctx.set_theme(new.gui.theme);
        }
        let (keymap, warnings) = new.keymap();
        self.keymap = keymap;
        self.key_warnings = warnings;
        // Textures colorées d'après les réglages : recalculées.
        self.mask_textures.clear();
        self.analysis_texture = None;
        self.gui = new.gui;
        self.settings.i18n = new.i18n;
        self.settings.files = new.files;
        self.settings.xmp = new.xmp;
        self.settings.offload = new.offload;
    }

    /// Application sur le dossier `dir` (parcouru tout de suite, calculs lancés à la première
    /// image), avec les réglages `settings`. `warning` : problème de configuration à
    /// afficher.
    pub fn new(dir: Option<PathBuf>, settings: AppSettings, warning: Option<String>) -> TriApp {
        let gui = settings.gui.clone();
        let data = settings.engine().data_dir();
        let dir = dir.or_else(|| {
            gui.reopen_last
                .then(|| tri::folder::last(data.as_deref()?))
                .flatten()
        });
        let (folder, error) = match &dir {
            Some(dir) => match Folder::open(dir, 1, settings.xmp.sidecar) {
                Ok(folder) => {
                    if let Some(data) = &data {
                        let _ = tri::folder::remember(data, &folder.dir);
                    }
                    (folder, None)
                }
                Err(e) => (Folder::default(), Some(format!("{} : {e}", dir.display()))),
            },
            None => (Folder::default(), None),
        };
        let mut tree = Tree::new(settings.places.clone(), gui.folders.hidden);
        if !folder.is_none() {
            tree.reveal(&folder.dir);
        }
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
        let (keymap, key_warnings) = settings.keymap();
        for warning in &key_warnings {
            eprintln!("{}", t!("warning-prefix", message = warning));
        }
        let mut app = TriApp {
            folder,
            tree,
            folder_panel: gui.folders.visible,
            reveal_folder: true,
            places: settings.places.clone(),
            volumes: None,
            offload: None,
            offloaded: None,
            eject_failed: None,
            eject_dialog: None,
            ejecting: None,
            notice: None,
            settings: settings.engine(),
            warning,
            key_warnings,
            note_error: None,
            filter: KindFilter::All,
            notes_rule: NotesRule::default(),
            filter_bar: gui.filter_bar,
            rating_bar: gui.rating_bar,
            auto_advance: gui.auto_advance,
            rating_dialog: None,
            file_dialog: None,
            file_error: None,
            selection: Default::default(),
            scores: HashMap::new(),
            judgements: HashMap::new(),
            blur_rule: None,
            blurry_applied: HashSet::new(),
            peaking: gui.sharpness.visible,
            peaking_color: 0,
            focus_point: gui.view.focus_point,
            zoom_focus_pending: None,
            data_dir: data.clone(),
            display_saved: Default::default(),
            peaking_texture: None,
            peaking_asked: None,
            compare: Vec::new(),
            compare_cols: 1,
            hint: None,
            files: Vec::new(),
            status: Vec::new(),
            progress: [Progress::default(); 5],
            engine: None,
            error,
            keymap,
            sequencer: Sequencer::new(gui.key_sequence_timeout()),
            mouse_active: false,
            hovered: None,
            #[cfg(test)]
            cell_rects: Vec::new(),
            #[cfg(test)]
            quit_requested: false,
            pan: Vec2::ZERO,
            textures: Lru::new(gui.grid.textures),
            metadata_panel: gui.metadata.visible,
            meta: None,
            histogram: gui.histogram.visible,
            histogram_large: false,
            histogram_scale: gui.histogram.scale,
            histogram_channel: None,
            analysis_texture: None,
            clipping: gui.histogram.clipping,
            exposures: Lru::new(gui.view.textures),
            mask_textures: Lru::new(gui.view.textures),
            clip_summary: std::collections::HashMap::new(),
            selected,
            layout: Layout::fit(0.0, gui.grid.cell, gui.grid.gap),
            scroll_offset: 0.0,
            view_height: 0.0,
            reveal: None,
            screenshot,
            help: false,
            settings_window: None,
            config_path: None,
            history: tri::history::History::new(gui.history),
            history_open: false,
            help_search: None,
            visible_waiting: 0,
            startup_actions: startup_actions().into(),
            script_wait: None,
            mode: Mode::Grid,
            source: Source::default(),
            zoom: None,
            zoom_anchor: None,
            // En comparaison, chaque image garde son aperçu et son 1:1.
            big: Lru::new(gui.view.textures.max(gui.compare.max * 2 + 2)),
            wanted: Vec::new(),
            missing: HashSet::new(),
            view_waiting: false,
            requested: HashSet::new(),
            last_shown: (usize::MAX, None),
            focus_since: Instant::now(),
            last_zoom_size: std::collections::HashMap::new(),
            gui,
        };
        app.restore_display();
        app.apply_filter();
        app
    }

    /// État d'affichage courant (`state.rs`).
    fn display(&self) -> crate::state::DisplayState {
        crate::state::DisplayState {
            focus_point: Some(self.focus_point),
            peaking: Some(self.peaking),
            clipping: Some(self.clipping),
            histogram: Some(self.histogram),
            folder_panel: Some(self.folder_panel),
            metadata_panel: Some(self.metadata_panel),
            filter_bar: Some(self.filter_bar),
            rating_bar: Some(self.rating_bar),
            auto_advance: Some(self.auto_advance),
        }
    }

    /// Reprend l'état d'affichage du lancement précédent (`[gui] remember_display`).
    fn restore_display(&mut self) {
        if !self.gui.remember_display {
            return;
        }
        let Some(data) = &self.data_dir else { return };
        let s = crate::state::load(data);
        let pairs = [
            (&mut self.focus_point, s.focus_point),
            (&mut self.peaking, s.peaking),
            (&mut self.clipping, s.clipping),
            (&mut self.histogram, s.histogram),
            (&mut self.folder_panel, s.folder_panel),
            (&mut self.metadata_panel, s.metadata_panel),
            (&mut self.filter_bar, s.filter_bar),
            (&mut self.rating_bar, s.rating_bar),
            (&mut self.auto_advance, s.auto_advance),
        ];
        for (field, saved) in pairs {
            if let Some(value) = saved {
                *field = value;
            }
        }
        self.display_saved = self.display();
    }

    /// Écrit l'état d'affichage s'il a changé depuis la dernière écriture.
    fn save_display(&mut self) {
        // Capture automatique (développement) : ses actions scriptées ne doivent pas
        // changer l'état de l'utilisateur.
        if !self.gui.remember_display || self.screenshot.is_some() {
            return;
        }
        let now = self.display();
        if now == self.display_saved {
            return;
        }
        if let Some(data) = &self.data_dir
            && crate::state::save(data, &now).is_ok()
        {
            self.display_saved = now;
        }
    }

    /// Ouvre le dossier `dir` à la place du dossier courant (panneau des dossiers). Les
    /// filtres sont gardés ; la sélection repart de la première image. En cas d'erreur, le
    /// message remplace la grille, le panneau reste là pour choisir un autre dossier.
    fn open_folder(&mut self, dir: &Path) {
        match Folder::open(dir, 1, self.settings.xmp.sidecar) {
            Ok(folder) => {
                trace(|| format!("dossier ouvert : {}", folder.dir.display()));
                // Mémorisé pour le prochain lancement (sans conséquence en cas d'échec).
                if let Some(data) = self.settings.data_dir() {
                    let _ = tri::folder::remember(&data, &folder.dir);
                }
                self.folder = folder;
                self.error = None;
            }
            Err(e) => {
                self.folder = Folder::default();
                self.error = Some(format!("{} : {e}", dir.display()));
            }
        }
        self.files.clear(); // la sélection ne survit pas au changement de dossier
        self.selection.clear();
        self.scores.clear();
        self.judgements.clear();
        self.blurry_applied.clear();
        self.selected = 0;
        self.mode = Mode::Grid;
        self.zoom = None;
        self.zoom_anchor = None;
        self.note_error = None;
        self.rating_dialog = None;
        self.wanted.clear();
        self.missing.clear();
        if let Some(meta) = &mut self.meta {
            meta.clear();
        }
        self.apply_filter();
    }

    /// Panneau des métadonnées : celles de l'image sélectionnée (lignes du moteur), et en
    /// grille son histogramme.
    fn metadata_panel(&mut self, ui: &mut Ui) {
        let Some(file) = self.files.get(self.selected).cloned() else {
            ui.weak(t!("panel-no-image"));
            return;
        };
        ui.strong(file_name(&file.path));
        ui.add_space(4.0);
        // Histogramme en haut du panneau, en grille comme en plein écran (retour du
        // 17/10/2026) ; panneau masqué : en plein écran, il est sur l'image.
        let settings = self.gui.metadata;
        if settings.histogram && self.histogram {
            match self.exposure_of(self.selected) {
                Some(e) => {
                    let (rect, response) = ui.allocate_exact_size(
                        vec2(ui.available_width(), settings.histogram_height),
                        Sense::click(),
                    );
                    let tip = t!(
                        "histogram-enlarge",
                        keys = self.keymap.keys_for(Action::HistogramLarge).join(" / ")
                    );
                    if response
                        .on_hover_cursor(egui::CursorIcon::ZoomIn)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        self.histogram_large = true;
                    }
                    let look = histogram::Look {
                        scale: self.histogram_scale,
                        details: false,
                        channel: None,
                    };
                    histogram::draw(
                        ui.painter(),
                        rect,
                        &e,
                        look,
                        &self.gui.histogram,
                        &self.gui.colors,
                    );
                }
                None => {
                    ui.weak(t!("histogram-computing"));
                }
            }
            ui.add_space(8.0);
        }
        let read = self
            .meta
            .get_or_insert_with(|| MetaLoader::new(ui.ctx().clone()))
            .get(&file)
            .cloned();
        match read {
            None => {
                ui.weak(t!("panel-reading"));
            }
            Some(Err(e)) => {
                ui.colored_label(self.gui.colors.error, e);
            }
            Some(Ok(m)) => {
                egui::Grid::new("metadata")
                    .num_columns(2)
                    .spacing(vec2(10.0, 4.0))
                    .show(ui, |ui| {
                        for (label, value) in m.rows() {
                            ui.weak(label);
                            // À la ligne plutôt qu'élargir le panneau (noms d'objectif).
                            ui.add(egui::Label::new(value).wrap());
                            ui.end_row();
                        }
                        if let Some(j) = self.judgements.get(&file.path) {
                            ui.weak(t!("panel-sharpness"));
                            let ratio = |r: Option<f32>| {
                                r.map_or("-".to_string(), |r| tri::i18n::decimal(r as f64, 2))
                            };
                            let mut text = t!(
                                "panel-sharpness-value",
                                score = tri::i18n::decimal(j.score as f64, 1),
                                folder = ratio(j.folder_ratio),
                                neighbours = ratio(j.neighbour_ratio)
                            );
                            if j.blurry {
                                text.push_str(&t!("panel-sharpness-blurry"));
                            }
                            ui.add(egui::Label::new(text).wrap());
                            ui.end_row();
                        }
                    });
            }
        }
    }

    /// Surveillance des volumes : démarrée au premier appel (il faut le contexte pour
    /// redessiner), puis intègre la nouvelle liste quand un disque est branché ou
    /// débranché.
    fn watch_volumes(&mut self, ctx: &egui::Context) {
        if self.volumes.is_none() {
            let (tx, rx) = mpsc::channel();
            let repaint = ctx.clone();
            let monitor = Monitor::start(self.places.clone(), move |places| {
                let _ = tx.send(places);
                repaint.request_repaint();
            });
            self.volumes = Some((monitor, rx));
        }
        if let Some((_, rx)) = &self.volumes
            && let Some(places) = rx.try_iter().last()
        {
            trace(|| format!("volumes : {}", places.len()));
            self.tree.set_places(places);
        }
    }

    /// Panneau des dossiers : l'arborescence du moteur ([`Tree`]), dessinée ligne par
    /// ligne. Le triangle déplie ou replie ; un clic sur le nom ouvre le dossier.
    fn folder_panel(&mut self, ui: &mut Ui) {
        let f = self.gui.folders;
        let mut toggle = None;
        let mut open = None;
        let mut eject = None;
        let mut offload = None;
        ui.horizontal(|ui| {
            ui.strong(t!("folders-title"));
            // Icône (retour du 17/10/2026), libellé, raccourci et explication au survol.
            let size = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
            let tip = format!(
                "{}\n{}",
                self.action_tip(Action::RefreshFolders),
                t!("folders-refresh-tip")
            );
            let (rect, response) = marks::icon_button(ui, size, &t!("folders-refresh"), &tip);
            let color = ui.style().interact(&response).fg_stroke.color;
            marks::refresh(ui.painter(), rect.shrink(size * 0.2), color);
            if response.clicked() {
                self.tree.refresh();
            }
        });
        ui.separator();
        let reveal = std::mem::take(&mut self.reveal_folder);
        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            for row in self.tree.rows() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_space(row.depth as f32 * f.indent);
                    let tip = if row.expanded {
                        t!("folders-collapse")
                    } else {
                        t!("folders-expand")
                    };
                    let size = ui.text_style_height(&egui::TextStyle::Body);
                    let (rect, response) = marks::clickable(ui, size, &tip);
                    marks::disclosure(ui.painter(), rect, row.expanded, ui.visuals().text_color());
                    if response.clicked() {
                        toggle = Some(row.path.clone());
                    }
                    let current = row.path == self.folder.dir;
                    let label = ui.selectable_label(current, &row.name);
                    if label.clicked() && !current {
                        open = Some(row.path.clone());
                    }
                    if label.double_clicked() {
                        toggle = Some(row.path.clone());
                    }
                    if current && reveal {
                        label.scroll_to_me(Some(egui::Align::Center));
                    }
                    if let Some(error) = &row.error {
                        ui.colored_label(self.gui.colors.error, t!("folders-unreadable"))
                            .on_hover_text(error);
                    }
                    if row.kind == Some(PlaceKind::Volume) {
                        let color = ui.visuals().text_color();
                        if !row.ejectable {
                            // Volume fixé ([places] fixed) : pas d'éjection, une icône
                            // de disque à la place.
                            let (rect, response) =
                                ui.allocate_exact_size(vec2(size, size), Sense::hover());
                            marks::drive(ui.painter(), rect, ui.visuals().weak_text_color());
                            response.on_hover_text(t!("folders-fixed-drive"));
                        } else if self.ejecting.is_none() {
                            // Un seul déchargement à la fois.
                            if self.offload.is_none() {
                                let (rect, response) =
                                    marks::clickable(ui, size, &t!("folders-offload"));
                                marks::offload(ui.painter(), rect, color);
                                if response.clicked() {
                                    offload = Some((row.path.clone(), rect.right_bottom()));
                                }
                            }
                            let (rect, response) = marks::clickable(ui, size, &t!("eject"));
                            marks::eject(ui.painter(), rect, color);
                            if response.clicked() {
                                eject = Some((row.path.clone(), rect.right_bottom()));
                            }
                        }
                    }
                });
            }
        });
        if let Some(path) = toggle {
            self.tree.toggle(&path);
        }
        if let Some(path) = open {
            self.open_folder(&path);
        }
        if eject.is_some() {
            self.eject_dialog = eject;
        }
        if let Some((source, at)) = offload {
            self.offload = Some(OffloadDialog::new(
                source,
                at,
                self.settings.offload.clone(),
                self.settings.data_dir(),
                ui.ctx(),
            ));
        }
    }

    /// Confirmation d'éjection (modale), puis éjection dans un fil à part : l'outil du
    /// système prend une à quelques secondes.
    fn eject_dialog(&mut self, ctx: &egui::Context) {
        let Some((path, at)) = self.eject_dialog.clone() else {
            return;
        };
        let name = folder_name(&path);
        let open_inside = !self.folder.is_none() && self.folder.dir.starts_with(&path);
        let (mut confirm, mut cancel) = (false, false);
        let title = t!("eject-question", name = &name);
        let modal = modal_at("eject_dialog", at).title(title).show(ctx, |ui| {
            if open_inside {
                ui.label(t!("eject-open-folder"));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                confirm = ui.button(t!("eject")).clicked();
                cancel = ui.button(t!("cancel")).clicked();
            });
        });
        if confirm && !self.tree.can_eject(&path) {
            // Fixé entre-temps (configuration relue) : on ne touche à rien.
            self.eject_dialog = None;
        } else if confirm {
            self.eject_dialog = None;
            self.start_eject(ctx, path);
        } else if cancel || modal.should_close() {
            self.eject_dialog = None;
        }
    }

    /// Éjecte `volume` dans un fil à part (l'outil du système prend une à quelques
    /// secondes) ; ferme d'abord le dossier ouvert s'il est dessus (sinon le système
    /// refuse : fichiers ouverts). Le résultat s'affiche dans la barre d'état.
    fn start_eject(&mut self, ctx: &egui::Context, volume: PathBuf) {
        if !self.tree.can_eject(&volume) {
            return; // volume fixé (configuration) : jamais
        }
        if !self.folder.is_none() && self.folder.dir.starts_with(&volume) {
            self.close_folder();
        }
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let target = volume.clone();
        std::thread::spawn(move || {
            let _ = tx.send(tri::places::eject(&target));
            repaint.request_repaint();
        });
        self.notice = Some((t!("ejecting", name = folder_name(&volume)), false));
        self.ejecting = Some((volume, rx));
    }

    /// Ouvre la séance déchargée dans la grille, et la montre dans l'arborescence.
    fn open_offloaded(&mut self, dir: &Path) {
        self.tree.reload();
        self.tree.reveal(dir);
        self.reveal_folder = true;
        self.open_folder(dir);
    }

    /// Boîte de déchargement : affichage, et suites (ouvrir la séance, éjecter la carte),
    /// choisies avant la copie et enchaînées à la fin.
    fn offload_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.offload else {
            return;
        };
        let hidden = dialog.is_hidden();
        let request = if hidden {
            dialog.tick()
        } else {
            dialog.show(ctx)
        };
        match request {
            None => {}
            Some(OffloadRequest::Close) => self.offload = None,
            // Fin de la copie : la boîte reste ouverte sur le bilan.
            Some(OffloadRequest::Finished {
                open,
                offer,
                eject,
                summary,
            }) => {
                if let Some(dir) = open {
                    self.open_offloaded(&dir);
                }
                self.offloaded = offer;
                if hidden {
                    // En arrière-plan : plus de boîte, le bilan va dans la barre d'état.
                    self.offload = None;
                    self.notice = Some((summary, false));
                }
                if let Some(volume) = eject {
                    self.start_eject(ctx, volume);
                }
            }
            Some(OffloadRequest::Open(dir)) => {
                self.offload = None;
                self.open_offloaded(&dir);
            }
            Some(OffloadRequest::Eject(volume)) => {
                self.offload = None;
                self.start_eject(ctx, volume);
            }
        }
    }

    /// Fin d'une éjection : message, et liste des volumes relue.
    fn poll_eject(&mut self) {
        let Some((path, rx)) = &self.ejecting else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        let name = folder_name(path);
        // Refus (volume utilisé par une autre application, ex. un gestionnaire de
        // fichiers ouvert dessus) : proposer de réessayer, une fois l'application fermée.
        self.eject_failed = result.is_err().then(|| path.clone());
        self.notice = Some(match result {
            Ok(()) => (t!("ejected", name = &name), false),
            Err(e) => (
                t!("eject-failed", name = &name, error = e.to_string()),
                true,
            ),
        });
        self.ejecting = None;
        self.tree.refresh();
    }

    /// Ferme le dossier ouvert (avant d'éjecter son volume).
    fn close_folder(&mut self) {
        self.folder = Folder::default();
        self.files.clear();
        self.selected = 0;
        self.mode = Mode::Grid;
        self.rating_dialog = None;
        self.apply_filter();
    }

    /// Recalcule la liste affichée selon les filtres, et la confie au scheduler. La ligne
    /// sélectionnée est remise en haut de la vue.
    fn apply_filter(&mut self) {
        self.refilter(Reveal::Top);
    }

    /// Recalcule la liste affichée selon les filtres (type, puis notes). L'image
    /// sélectionnée reste sélectionnée si elle reste visible ; sinon, la sélection garde sa
    /// place dans la liste (l'image suivante : on vient de rejeter une photo avec « masquer
    /// les rejetées », on passe à la suite).
    fn refilter(&mut self, reveal: Reveal) {
        let selected_path = self.files.get(self.selected).map(|f| f.path.clone());
        let groups = self.folder.filtered(self.filter, &self.notes_rule);
        // Photos floues : jugées par le moteur, d'après les scores connus.
        let groups = match self.blur_rule {
            Some(rule) => tri::sharpness::BlurFilter {
                judgements: &self.judgements,
                rule,
            }
            .apply(groups),
            None => groups,
        };
        self.blurry_applied = self.blurry_now();
        self.files = groups.into_iter().flatten().collect();
        self.status = vec![Status::default(); self.files.len()];
        self.requested.clear();
        self.progress = [Progress::default(); 5];
        // Garder la même image sélectionnée si elle reste visible.
        if let Some(path) = selected_path
            && let Some(index) = self.files.iter().position(|f| f.path == path)
        {
            self.selected = index;
        }
        self.selected = self.selected.min(self.files.len().saturating_sub(1));
        self.reveal = Some(reveal);
        if let Some(engine) = &mut self.engine {
            engine.generation = engine.scheduler.set_folder(self.files.clone());
        }
        self.tell_shown();
    }

    /// Indique au moteur les images affichées (priorité de calcul) : l'image courante et
    /// ses voisines, ou en comparaison toutes les images comparées.
    fn tell_shown(&self) {
        let Some(engine) = &self.engine else { return };
        if self.mode == Mode::Compare {
            engine.scheduler.set_shown(&self.displayed());
        } else {
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
                self.error = Some(t!("cache-unavailable", error = e.to_string()));
                return;
            }
        };
        // Limite des 1:1 en cache, dans un fil à part (parcours des dossiers du cache).
        {
            let (settings, cache) = (self.settings.clone(), cache.clone());
            std::thread::spawn(move || {
                if let Ok(freed) = settings.trim_cache(&cache)
                    && freed.entries > 0
                {
                    trace(|| format!("cache : {} 1:1 anciens supprimés", freed.entries));
                }
            });
        }
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

    /// Photos jugées floues d'après les scores connus.
    fn blurry_now(&self) -> HashSet<PathBuf> {
        self.judgements
            .iter()
            .filter(|(_, j)| j.blurry)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Vrai si l'image `path` est jugée floue (score connu).
    fn is_blurry(&self, path: &Path) -> bool {
        self.judgements.get(path).is_some_and(|j| j.blurry)
    }

    /// Lit les scores de netteté arrivés dans le cache ; filtre « masquer les floues »
    /// actif : refiltre une fois tous les scores connus, si le jugement a changé (la
    /// médiane bouge tant que des scores arrivent).
    fn take_scores(&mut self, ready: Vec<usize>) {
        let Some(engine) = &self.engine else { return };
        let mut changed = false;
        for index in ready {
            if let Some(file) = self.files.get(index)
                && let Some(score) = preview::sharpness(&engine.cache, file, &engine.preview)
            {
                self.scores.insert(file.path.clone(), score.value);
                changed = true;
            }
        }
        if changed {
            // Dans l'ordre du dossier, quels que soient les filtres : les voisines d'une
            // photo sont celles prises juste avant et juste après.
            let ordered: Vec<(PathBuf, Option<f32>)> = self
                .folder
                .files
                .iter()
                .map(|f| (f.path.clone(), self.scores.get(&f.path).copied()))
                .collect();
            self.judgements = tri::sharpness::judge(&ordered, &self.settings.sharpness);
        }
        let p = self.progress[4];
        if self.blur_rule.is_some()
            && p.total > 0
            && p.done == p.total
            && self.blurry_now() != self.blurry_applied
        {
            self.refilter(Reveal::Minimal);
        }
    }

    /// Intègre les événements du moteur reçus depuis la dernière image.
    fn drain_events(&mut self) {
        let mut scored = Vec::new();
        self.drain_engine(&mut scored);
        self.take_scores(scored);
    }

    fn drain_engine(&mut self, scored: &mut Vec<usize>) {
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
                    if level == Level::Sharpness {
                        scored.push(index);
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
        let mut chords = ctx.input(|i| self.keymap.chords(&i.events));
        // Saisie dans un champ de texte (nom de dossier, recherche) : les touches sont du
        // texte, pas des raccourcis ; seules les touches qui ne tapent rien (Échap, F1…)
        // agissent. Sans cela, taper « q » dans le nom d'un dossier quittait l'application.
        if ctx.egui_wants_keyboard_input() {
            chords.retain(|c| c.trigger.types_no_text());
        }
        if !chords.is_empty() {
            self.mouse_active = false; // on tape : pointeur masqué
        }
        let now = Instant::now();
        let actions: Vec<Action> = chords
            .into_iter()
            .filter_map(|c| self.sequencer.feed(&self.keymap, c, now))
            .collect();
        for action in actions {
            // Historique : annuler / rétablir pas à pas restent possibles, Échap ferme.
            if self.history_open {
                match action {
                    Action::Back | Action::History => self.history_open = false,
                    Action::Undo | Action::Redo => self.perform(ctx, action),
                    _ => {}
                }
                continue;
            }
            // Réglages : seule la fermeture.
            if self.settings_window.is_some() {
                if matches!(action, Action::Back | Action::Settings) {
                    self.settings_window = None;
                }
                continue;
            }
            // L'aide est modale : derrière elle, la grille ne bouge pas.
            if self.help && !action.works_over_overlay() {
                continue;
            }
            // Boîte de déchargement : seule la fermeture (pas pendant la copie).
            if let Some(dialog) = self.offload.as_ref().filter(|d| !d.is_hidden()) {
                if action == Action::Back && !dialog.is_running() {
                    self.offload = None;
                } else if action.works_over_overlay() && action != Action::Back {
                    self.perform(ctx, action);
                }
                continue;
            }
            // Confirmation d'éjection : seule la fermeture.
            if self.eject_dialog.is_some() {
                if action == Action::Back {
                    self.eject_dialog = None;
                } else if action.works_over_overlay() {
                    self.perform(ctx, action);
                }
                continue;
            }
            // Grand histogramme : échelle, fermeture (Échap, même touche).
            if self.histogram_large {
                match action {
                    Action::Back | Action::HistogramLarge => self.histogram_large = false,
                    Action::HistogramScale | Action::HistogramChannel => self.perform(ctx, action),
                    _ if action.works_over_overlay() => self.perform(ctx, action),
                    _ => {}
                }
                continue;
            }
            // Renommer / corbeille / copier : seule la fermeture (la boîte gère Entrée),
            // pas pendant une copie.
            if let Some(dialog) = &self.file_dialog {
                if action == Action::Back && !dialog.busy() {
                    self.file_dialog = None;
                } else if action.works_over_overlay() {
                    self.perform(ctx, action);
                }
                continue;
            }
            // Boîte de notation : seules les notes (pour son image) et la fermeture.
            if let Some((path, _)) = &self.rating_dialog {
                match action {
                    Action::Stars(_) | Action::Color(_) | Action::Flag(_) => {
                        self.rate_path(path.clone(), action)
                    }
                    _ if action.works_over_overlay() => self.perform(ctx, action),
                    _ => {}
                }
                continue;
            }
            self.perform(ctx, action);
        }
    }

    /// Exécute une action (clavier aujourd'hui, souris ou plugin demain).
    fn perform(&mut self, ctx: &egui::Context, action: Action) {
        self.hint = None;
        if self.mode == Mode::Compare && self.compare_perform(action) {
            return;
        }
        match action {
            Action::Quit => {
                #[cfg(test)]
                {
                    self.quit_requested = true;
                }
                ctx.send_viewport_cmd(ViewportCommand::Close)
            }
            Action::Help => self.help = !self.help,
            Action::Settings => self.open_settings(),
            Action::Undo => {
                let result = self.history.undo(self.settings.xmp.sidecar);
                self.after_history(result.map(|a| vec![a]).map_err(Some));
            }
            Action::Redo => {
                let result = self.history.redo(self.settings.xmp.sidecar);
                self.after_history(result.map(|a| vec![a]).map_err(Some));
            }
            Action::History => self.history_open = !self.history_open,
            Action::Back => {
                if self.help {
                    self.help = false;
                } else if self.rating_dialog.is_some() {
                    self.rating_dialog = None;
                } else if self.selection.is_visual() {
                    self.selection.end_visual(false);
                } else if self.mode == Mode::Compare {
                    self.leave_compare();
                } else if self.selection.visible(&self.files) > 0 {
                    self.selection.clear();
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
            Action::Compare => self.start_compare(),
            Action::TogglePeaking => self.peaking = !self.peaking,
            Action::ToggleFocusPoint => self.focus_point = !self.focus_point,
            Action::PeakingColor => {
                // Changer de couleur montre la carte si elle était masquée.
                let n = self.gui.sharpness.colors.len().max(1);
                self.peaking_color = (self.peaking_color + 1) % n;
                self.peaking = true;
            }
            Action::ToggleZoom => {
                // Depuis la grille : plein écran directement à 1:1 (vérifier vite la netteté
                // d'une vignette ; Échap ramène à la grille).
                if self.mode == Mode::Grid && !self.files.is_empty() {
                    self.mode = Mode::View;
                    self.zoom = None;
                    self.tell_shown();
                }
                if matches!(self.mode, Mode::View | Mode::Compare) {
                    self.zoom = match self.zoom {
                        Some(_) => None,
                        None => Some(self.zoom_center(ctx)),
                    };
                    self.request_zoom();
                }
            }
            Action::PanLeft | Action::PanRight | Action::PanUp | Action::PanDown => {
                // Seulement dans une image zoomée ; sans effet ailleurs.
                if matches!(self.mode, Mode::View | Mode::Compare) && self.zoom.is_some() {
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
            Action::Stars(_) | Action::Color(_) | Action::Flag(_) => self.rate_selected(action),
            Action::ToggleAutoAdvance => self.auto_advance = !self.auto_advance,
            Action::FileRename
            | Action::FileDelete
            | Action::FileReveal
            | Action::FileEdit
            | Action::FileCopy
            | Action::FileMove => {
                // Renommer, afficher : l'image courante ; les autres : la sélection.
                let paths = match action {
                    Action::FileDelete | Action::FileEdit | Action::FileCopy | Action::FileMove => {
                        self.selection.targets(&self.files, self.selected)
                    }
                    _ => self
                        .files
                        .get(self.selected)
                        .map(|f| vec![f.path.clone()])
                        .unwrap_or_default(),
                };
                // Au clavier : boîte vers le centre de la fenêtre.
                let at = ctx.content_rect().center() - vec2(160.0, 80.0);
                self.file_action(action, paths, at);
            }
            Action::SelectToggle => {
                if let Some(file) = self.files.get(self.selected) {
                    self.selection.toggle(&file.path);
                }
            }
            Action::SelectVisual => {
                if self.selection.is_visual() {
                    self.selection.end_visual(true);
                } else {
                    self.selection.start_visual(&self.files, self.selected);
                }
            }
            Action::SelectAll => self.selection.select_all(&self.files),
            Action::SelectClear => self.selection.clear(),
            Action::FilterCycle => {
                self.filter = match self.filter {
                    KindFilter::All => KindFilter::RawOnly,
                    KindFilter::RawOnly => KindFilter::FlatOnly,
                    KindFilter::FlatOnly => KindFilter::All,
                };
                self.apply_filter();
            }
            Action::FilterStars(stars) => {
                self.notes_rule.stars = (stars > 0).then_some(StarsRule::AtLeast(stars));
                self.apply_filter();
            }
            Action::FilterFlag(choice) => {
                // La même touche désactive le filtre.
                let flags = choice.flags();
                self.notes_rule.flags = match &self.notes_rule.flags {
                    Some(current) if *current == flags => None,
                    _ => Some(flags),
                };
                self.apply_filter();
            }
            Action::FilterColor(color) => {
                let mut rule = self.notes_rule.colors.take().unwrap_or_default();
                match rule.any_of.iter().position(|&c| c == color) {
                    Some(i) => {
                        rule.any_of.remove(i);
                    }
                    None => {
                        rule.any_of.push(color);
                        rule.any_of.sort();
                    }
                }
                self.notes_rule.colors = (rule != ColorRule::default()).then_some(rule);
                self.apply_filter();
            }
            Action::FilterClear => {
                self.filter = KindFilter::All;
                self.notes_rule = NotesRule::default();
                self.blur_rule = None;
                self.apply_filter();
            }
            Action::FilterBlurry | Action::FilterOnlyBlurry => {
                use tri::sharpness::BlurRule;
                let rule = match action {
                    Action::FilterBlurry => BlurRule::HideBlurry,
                    _ => BlurRule::OnlyBlurry,
                };
                // Même touche : on retire le filtre ; l'autre : on bascule.
                self.blur_rule = (self.blur_rule != Some(rule)).then_some(rule);
                self.apply_filter();
            }
            Action::ToggleFilterBar => self.filter_bar = !self.filter_bar,
            Action::ToggleRatingBar => self.rating_bar = !self.rating_bar,
            Action::RefreshFolders => self.tree.refresh(),
            Action::ToggleHistogram => self.histogram = !self.histogram,
            Action::HistogramScale => self.histogram_scale = self.histogram_scale.toggled(),
            Action::HistogramLarge => self.histogram_large = !self.histogram_large,
            // Le grand histogramme s'ouvre au besoin : c'est là que le canal se voit.
            Action::HistogramChannel => {
                self.histogram_channel = match self.histogram_channel {
                    None => Some(0),
                    Some(c) if c + 1 < tri::exposure::CHANNELS => Some(c + 1),
                    Some(_) => None,
                };
                self.histogram_large = true;
            }
            Action::ToggleClipping => self.clipping = !self.clipping,
            Action::ToggleMetadata => self.metadata_panel = !self.metadata_panel,
            Action::ToggleFolderPanel => {
                self.folder_panel = !self.folder_panel;
                self.reveal_folder = self.folder_panel;
            }
            nav => {
                let page = self.layout.rows_in(self.view_height);
                let new = grid::navigate(self.selected, self.files.len(), &self.layout, page, nav);
                self.select(new);
            }
        }
    }

    /// Bulle d'un contrôle : libellé de l'action et ses raccourcis.
    fn action_tip(&self, action: Action) -> String {
        format!(
            "{} ({})",
            action.label(),
            self.keymap.keys_for(action).join(" / ")
        )
    }

    /// Note l'image sélectionnée ; avec l'avance automatique, passe ensuite à la suivante.
    /// Si la note a fait sortir l'image du filtre, la sélection est déjà sur la suivante
    /// ([`TriApp::refilter`]) : on ne bouge pas, sinon on sauterait une image. L'autre image
    /// d'une paire RAW+JPEG, notée en même temps, est sautée aussi.
    fn rate_selected(&mut self, action: Action) {
        // Comparaison : l'image active seule ; avance automatique vers l'image comparée
        // suivante.
        if self.mode == Mode::Compare {
            if let Some(file) = self.files.get(self.selected) {
                self.rate_path(file.path.clone(), action);
            }
            let panes = self.compare_indices();
            if self.auto_advance
                && self.note_error.is_none()
                && let Some(pos) = panes.iter().position(|&i| i == self.selected)
                && let Some(&next) = panes.get(pos + 1)
            {
                self.select(next);
            }
            return;
        }
        let targets = self.selection.targets(&self.files, self.selected);
        if targets.len() > 1 {
            self.rate_many(&targets, action);
            return;
        }
        let Some(path) = self.files.get(self.selected).map(|f| f.path.clone()) else {
            return;
        };
        self.rate_path(path.clone(), action);
        if !self.auto_advance
            || self.note_error.is_some()
            || self.files.get(self.selected).is_none_or(|f| f.path != path)
        {
            return;
        }
        let siblings = self.folder.notes.siblings(&path);
        let next = (self.selected + 1..self.files.len())
            .find(|&i| !siblings.contains(&self.files[i].path));
        if let Some(next) = next {
            self.select(next);
        }
    }

    /// Note plusieurs images (sélection multiple), sans avance automatique. Une paire
    /// RAW+JPEG n'est notée qu'une fois (noter une image note aussi l'autre : une couleur
    /// basculerait deux fois). Couleur : même effet sur toutes, d'après la première
    /// (ajoutée si elle ne l'a pas, retirée sinon), au lieu d'une bascule image par image
    /// qui mélangerait.
    fn rate_many(&mut self, targets: &[PathBuf], action: Action) {
        let before = tri::history::snapshot(&self.folder.notes, targets);
        let multiple = self.settings.xmp.multiple_colors;
        let add = match action {
            Action::Color(c) => !self.folder.notes.notes(&targets[0]).has_color(c),
            _ => true,
        };
        let mut done: HashSet<PathBuf> = HashSet::new();
        let mut error = None;
        for path in targets {
            if !done.insert(path.clone()) {
                continue;
            }
            done.extend(self.folder.notes.siblings(path));
            let result = self.folder.notes.edit(path, |n| match action {
                Action::Stars(stars) => n.set_stars(stars),
                Action::Color(color) if n.has_color(color) != add => {
                    n.toggle_color(color, multiple)
                }
                Action::Flag(flag) => n.flag = flag,
                _ => {}
            });
            if let Err(e) = result {
                error.get_or_insert(t!("rating-not-saved", error = e.to_string()));
            }
        }
        self.note_error = error;
        // Un seul pas pour toute la sélection.
        if let Some(step) = tri::history::notes_step(&self.folder.notes, before) {
            self.history.push(step);
        }
        if !self.notes_rule.is_empty() {
            self.refilter(Reveal::Minimal);
        }
    }

    /// Note l'image `path` et écrit son sidecar. En cas d'échec, l'erreur est affichée
    /// dans la barre d'état et la note affichée reste celle du disque.
    fn rate_path(&mut self, path: PathBuf, action: Action) {
        let before = tri::history::snapshot(&self.folder.notes, std::slice::from_ref(&path));
        let multiple = self.settings.xmp.multiple_colors;
        let result = self.folder.notes.edit(&path, |n| match action {
            Action::Stars(stars) => n.set_stars(stars),
            Action::Color(color) => n.toggle_color(color, multiple),
            Action::Flag(flag) => n.flag = flag,
            _ => {}
        });
        self.note_error = match result {
            Ok(changed) => {
                trace(|| format!("notes écrites : {changed:?}"));
                None
            }
            Err(e) => Some(t!("rating-not-saved", error = e.to_string())),
        };
        if let Some(step) = tri::history::notes_step(&self.folder.notes, before) {
            self.history.push(step);
        }
        // Filtre sur les notes : l'image peut ne plus y correspondre.
        if !self.notes_rule.is_empty() {
            self.refilter(Reveal::Minimal);
        }
    }

    fn select(&mut self, index: usize) {
        self.select_with(index, true);
    }

    /// Sélectionne `index` ; `reveal` : faire défiler la grille pour qu'il soit visible
    /// (pas pour le survol de la souris : la grille bougerait sous le pointeur, et la
    /// sélection avec elle).
    fn select_with(&mut self, index: usize, reveal: bool) {
        if index == self.selected || index >= self.files.len() {
            return;
        }
        self.selected = index;
        self.selection.update_visual(&self.files, index);
        self.focus_since = Instant::now();
        self.tell_shown();
        if reveal {
            self.reveal = Some(Reveal::Minimal);
        }
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

    /// Images affichées, la principale d'abord : en plein écran l'image et ses voisines
    /// (préchargement), en comparaison les images comparées (l'active d'abord).
    fn displayed(&self) -> Vec<usize> {
        let n = self.files.len();
        let current = self.selected;
        match self.mode {
            Mode::Grid => Vec::new(),
            Mode::View => [current, current + 1, current.wrapping_sub(1)]
                .into_iter()
                .filter(|&i| i < n)
                .collect(),
            Mode::Compare => {
                let mut images = self.compare_indices();
                if let Some(pos) = images.iter().position(|&i| i == current) {
                    images[..=pos].rotate_right(1);
                }
                images
            }
        }
    }

    /// Indices (dans la liste affichée) des images comparées encore visibles.
    fn compare_indices(&self) -> Vec<usize> {
        self.compare
            .iter()
            .filter_map(|p| self.files.iter().position(|f| &f.path == p))
            .collect()
    }

    /// Entre en comparaison avec les images sélectionnées (au moins deux).
    fn start_compare(&mut self) {
        let mut targets = self.selection.targets(&self.files, self.selected);
        if targets.len() < 2 {
            self.hint = Some(t!("compare-needs-two"));
            return;
        }
        let max = self.gui.compare.max.max(2);
        if targets.len() > max {
            self.hint = Some(t!("compare-limited", max = max));
            targets.truncate(max);
        }
        // Image active : l'image courante si elle est comparée, sinon la première.
        let current = self.files.get(self.selected).map(|f| f.path.clone());
        let active = match current {
            Some(p) if targets.contains(&p) => p,
            _ => targets[0].clone(),
        };
        self.compare = targets;
        self.mode = Mode::Compare;
        self.zoom = None;
        self.zoom_anchor = None;
        if let Some(index) = self.files.iter().position(|f| f.path == active) {
            self.select(index);
        }
        self.tell_shown();
        self.request_zoom();
    }

    /// Quitte la comparaison vers la grille, sur l'image active.
    fn leave_compare(&mut self) {
        self.mode = Mode::Grid;
        self.compare.clear();
        self.zoom = None;
        self.zoom_anchor = None;
        self.reveal = Some(Reveal::Minimal);
        self.tell_shown();
    }

    /// Actions propres à la comparaison ; `false` : action ordinaire (zoom, source,
    /// déplacement, panneaux…).
    fn compare_perform(&mut self, action: Action) -> bool {
        let panes = self.compare_indices();
        let Some(active) = panes.iter().position(|&i| i == self.selected) else {
            // Image active disparue (filtre) : la première.
            match panes.first() {
                Some(&first) => self.select(first),
                None => self.leave_compare(),
            }
            return false;
        };
        let (n, cols) = (panes.len(), self.compare_cols);
        let target = match action {
            Action::NavPrev => Some(crate::compare::neighbour(active, n, cols, -1, 0)),
            Action::NavNext => Some(crate::compare::neighbour(active, n, cols, 1, 0)),
            Action::NavUp => Some(crate::compare::neighbour(active, n, cols, 0, -1)),
            Action::NavDown => Some(crate::compare::neighbour(active, n, cols, 0, 1)),
            Action::NavFirst | Action::NavPageUp => Some(0),
            Action::NavLast | Action::NavPageDown => Some(n - 1),
            _ => None,
        };
        if let Some(t) = target {
            self.select(panes[t]);
            return true;
        }
        let path = self.files[self.selected].path.clone();
        match action {
            Action::Open => {
                self.compare.clear();
                self.mode = Mode::View;
                self.tell_shown();
                self.request_zoom();
            }
            // Éliminer : l'image active quitte la comparaison (et la sélection).
            Action::SelectToggle => {
                self.compare.retain(|p| *p != path);
                self.tell_shown();
                if self.selection.contains(&path) {
                    self.selection.toggle(&path);
                }
                let left = self.compare_indices();
                match left.get(active.min(left.len().saturating_sub(1))) {
                    Some(&next) => self.select(next),
                    None => self.leave_compare(),
                }
            }
            Action::Compare => self.leave_compare(),
            Action::Stars(_) | Action::Color(_) | Action::Flag(_) => self.rate_selected(action),
            Action::FileRename
            | Action::FileDelete
            | Action::FileReveal
            | Action::FileEdit
            | Action::FileCopy
            | Action::FileMove => {
                let at = pos2(200.0, 200.0);
                self.file_action(action, vec![path], at);
            }
            _ => return false,
        }
        true
    }

    /// Plein écran : demande au moteur ce que l'affichage va réclamer, **dans l'ordre des
    /// priorités** : image affichée ajustée (rendu RAW en mode RAW), son 1:1 si zoomé, puis
    /// les voisines (préchargement) dans le même ordre. L'ordre d'envoi compte : un worker
    /// libre prend aussitôt la première demande reçue, et un rendu LibRaw ne s'interrompt
    /// pas ; envoyer d'abord les 1:1 faisait passer un 100 % (~4,5 s) devant le rendu
    /// ajusté (~0,5 s).
    fn request_zoom(&mut self) {
        let images = self.displayed();
        if images.is_empty() {
            return;
        }
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
            Level::Exposure => &s.exposure,
            Level::Sharpness => &s.sharpness,
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
            if loaded.peaking {
                if self.peaking_asked.as_ref() == Some(&key) {
                    self.peaking_asked = None;
                }
                self.peaking_texture = loaded.image.map(|image| {
                    let name = format!("{}#netteté", loaded.path.display());
                    (key, ctx.load_texture(name, image, TextureOptions::NEAREST))
                });
                continue;
            }
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
        let displayed = self.displayed();
        if displayed.is_empty() {
            return;
        }
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
        let mut wanted = Vec::new();
        // Zoomé, les voisines aussi en 1:1 (préchargement).
        let zoomed = self.zoom.is_some();
        for index in displayed {
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

    /// Centre du 1:1 au clavier ou au bouton : le collimateur de l'image courante quand il
    /// est connu (c'est là qu'on juge la netteté, `[gui.view] zoom_on_focus_point`), sinon
    /// le milieu de l'image.
    fn zoom_center(&mut self, ctx: &egui::Context) -> Vec2 {
        let middle = vec2(0.5, 0.5);
        if !self.gui.view.zoom_on_focus_point {
            return middle;
        }
        let Some(file) = self.files.get(self.selected).cloned() else {
            return middle;
        };
        let read = self
            .meta
            .get_or_insert_with(|| MetaLoader::new(ctx.clone()))
            .get(&file)
            .cloned();
        match read {
            // Pas encore lues : milieu pour l'instant, recentré à leur arrivée.
            None => {
                self.zoom_focus_pending = Some(file.path);
                middle
            }
            Some(read) => read
                .ok()
                .and_then(|m| m.focus_point)
                .map_or(middle, |(x, y)| vec2(x, y)),
        }
    }

    /// 1:1 en attente du collimateur ([`TriApp::zoom_center`]) : recentré dès que les
    /// métadonnées de l'image arrivent, si l'on est toujours sur elle à 1:1.
    fn apply_pending_focus_zoom(&mut self) {
        let Some(path) = self.zoom_focus_pending.clone() else {
            return;
        };
        let current = self.files.get(self.selected).cloned();
        let (Some(file), Some(_), Some(meta)) = (current, self.zoom, self.meta.as_mut()) else {
            self.zoom_focus_pending = None;
            return;
        };
        if file.path != path {
            self.zoom_focus_pending = None;
            return;
        }
        if let Some(read) = meta.get(&file) {
            if let Some((x, y)) = read.as_ref().ok().and_then(|m| m.focus_point) {
                self.zoom = Some(vec2(x, y));
            }
            self.zoom_focus_pending = None;
        }
    }

    /// Repère du collimateur de mise au point de `file` sur l'image affichée en `rect`
    /// (métadonnées lues en fond ; rien tant qu'elles ne sont pas là, ni pour un boîtier
    /// dont le collimateur n'est pas lu).
    fn draw_focus_point(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        file: &MediaFile,
        rect: Option<Rect>,
    ) {
        let Some(rect) = rect.filter(|_| self.focus_point) else {
            return;
        };
        let point = self
            .meta
            .get_or_insert_with(|| MetaLoader::new(ctx.clone()))
            .get(file)
            .and_then(|read| read.as_ref().ok())
            .and_then(|m| m.focus_point);
        if let Some((x, y)) = point {
            let v = &self.gui.view;
            let center = rect.min + vec2(x * rect.width(), y * rect.height());
            let color = Color32::from(self.gui.colors.focus);
            // Liseré sombre sous le trait : visible sur un fond clair comme foncé.
            let shadow =
                egui::Stroke::new(v.focus_marker_width + 2.0, Color32::from_black_alpha(140));
            marks::focus_marker(painter, center, v.focus_marker_size, v.focus_marker, shadow);
            let stroke = egui::Stroke::new(v.focus_marker_width, color);
            marks::focus_marker(painter, center, v.focus_marker_size, v.focus_marker, stroke);
        }
    }

    /// Carte de netteté de `file` sur son 1:1 affiché en `rect` (`at_one_to_one` : c'est
    /// le 1:1 qui est affiché) : dessinée si elle est chargée, demandée au chargeur sinon ;
    /// sans objet, la demande en cours est retirée.
    fn peaking_overlay(
        &mut self,
        painter: &egui::Painter,
        file: &MediaFile,
        at_one_to_one: bool,
        rect: Option<Rect>,
    ) -> Peaking {
        let wanted = (self.peaking && at_one_to_one)
            .then(|| (file.path.clone(), view::zoom_level(file.kind, self.source)));
        let Some(key) = wanted else {
            if self.peaking_asked.take().is_some()
                && let Some(engine) = &self.engine
            {
                engine.loader.want_peaking(None);
            }
            return Peaking::NotAtOneToOne;
        };
        if let (Some((loaded, texture)), Some(rect)) = (&self.peaking_texture, rect)
            && *loaded == key
        {
            let colors = &self.gui.sharpness.colors;
            let color = colors
                .get(self.peaking_color % colors.len().max(1))
                .map_or(Color32::RED, |&c| Color32::from(c))
                .gamma_multiply(self.gui.sharpness.opacity);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(texture.id(), rect, uv, color);
            return Peaking::Shown;
        }
        if self.peaking_asked.as_ref() != Some(&key)
            && let Some(engine) = &self.engine
        {
            engine
                .loader
                .want_peaking(Some((file.clone(), key.1, self.settings.sharpness)));
            self.peaking_asked = Some(key);
        }
        self.view_waiting = true;
        Peaking::Computing
    }

    /// Comparaison : les images comparées côte à côte, ajustées ou à 1:1 au même endroit
    /// (zoom et déplacement liés), l'image active encadrée.
    fn compare_view(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let panes = self.compare_indices();
        if panes.is_empty() {
            self.leave_compare();
            return;
        }
        let area = ui.available_rect_before_wrap();
        ui.allocate_rect(area, Sense::hover());
        let settings = self.gui.compare;
        // Proportions : celles de la première image dont un aperçu est chargé.
        let aspect = panes
            .iter()
            .find_map(|&i| {
                [Level::Embedded, Level::Standard].iter().find_map(|&l| {
                    self.big
                        .get(&(self.files[i].path.clone(), l))
                        .map(|b| b.original.x / b.original.y)
                })
            })
            .unwrap_or(1.5);
        let (cols, rects) = crate::compare::layout(panes.len(), area, aspect, settings.gap);
        self.compare_cols = cols;
        let deadline = Instant::now() + self.gui.load_budget();
        let zoomed = self.zoom.is_some();
        let pan = std::mem::take(&mut self.pan);
        let mut waiting = false;
        let mut new_center = None;
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        for (&index, &rect) in panes.iter().zip(&rects) {
            let file = self.files[index].clone();
            let id = ui.scope_id().with(("compare", &file.path));
            let response = ui.interact(rect, id, Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, self.gui.colors.view_background);
            let zoom_level = view::zoom_level(file.kind, self.source);
            let desired = view::pick(file.kind, self.source, zoomed, |l| self.ready(index, l));
            let shown = view::pick(file.kind, self.source, zoomed, |l| {
                self.ready(index, l) && self.loaded(index, l)
            });
            waiting |= shown != desired || shown.is_none() || (zoomed && shown != Some(zoom_level));
            let drawable: Option<(TextureHandle, Vec2)> = match shown {
                Some(Level::Thumbnail) => self
                    .thumbnail_texture(&ctx, index, deadline)
                    .map(|t| (t.clone(), t.size_vec2())),
                Some(level) => self
                    .big
                    .get(&(file.path.clone(), level))
                    .map(|b| (b.texture.clone(), b.original)),
                None => None,
            };
            if let Some((texture, original)) = drawable {
                let image_rect = match (self.zoom, shown == Some(zoom_level)) {
                    // 1:1 au même point relatif de chaque image.
                    (Some(center), true) => {
                        let size = original / ctx.pixels_per_point();
                        let mut c = new_center.unwrap_or(center);
                        if new_center.is_none() {
                            if response.dragged() {
                                let d = response.drag_delta();
                                c -= vec2(d.x / size.x, d.y / size.y);
                            }
                            c += vec2(
                                pan.x * rect.width() / size.x,
                                pan.y * rect.height() / size.y,
                            );
                            if c != center {
                                new_center = Some(c);
                            }
                        }
                        let c = view::clamp_center(rect.size(), size, c);
                        view::zoom_rect(rect, size, c)
                    }
                    _ => view::fit_rect(rect, original),
                };
                painter.image(texture.id(), image_rect, uv, Color32::WHITE);
                self.draw_focus_point(&ctx, &painter, &file, Some(image_rect));
                if self.clipping
                    && let Some(mask) = self.mask_texture(&ctx, index)
                {
                    let opacity = self.gui.histogram.clipping_opacity;
                    painter.image(
                        mask.id(),
                        image_rect,
                        uv,
                        Color32::WHITE.gamma_multiply(opacity),
                    );
                }
            }
            // Nom, ce qui est affiché, notes.
            let mut label = file_name(&file.path);
            if let Some(level) = shown {
                label.push_str(" · ");
                label.push_str(&view::label(file.kind, level));
            }
            let font = egui::FontId::proportional(13.0);
            let galley = painter.layout_no_wrap(label, font, Color32::WHITE);
            let text_pos = rect.left_bottom() + vec2(8.0, -8.0 - galley.size().y);
            painter.rect_filled(
                Rect::from_min_size(text_pos - vec2(4.0, 2.0), galley.size() + vec2(8.0, 4.0)),
                3.0,
                Color32::from_black_alpha(160),
            );
            painter.galley(text_pos, galley, Color32::WHITE);
            let notes = self.folder.notes.notes(&file.path).clone();
            let size = self.gui.view.marks;
            marks::row(
                &painter,
                rect.left_top() + vec2(10.0, 10.0 + size / 2.0),
                &notes,
                size,
                &self.gui.colors,
            );
            // Image active : cadre de la couleur de sélection.
            if index == self.selected {
                let color = self
                    .gui
                    .colors
                    .selection
                    .map_or(ui.visuals().selection.bg_fill, Color32::from);
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(settings.frame, color),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() || response.drag_started() {
                self.select(index);
            }
            if response.double_clicked() {
                self.select(index);
                self.compare.clear();
                self.mode = Mode::View;
                self.tell_shown();
            }
            self.open_rating_dialog(&response, index);
        }
        if let Some(c) = new_center {
            self.zoom = Some(c);
        }
        self.view_waiting = waiting;
    }

    /// Plein écran : l'image sélectionnée, ajustée ou à 100 %, avec ce qui est affiché.
    fn viewer(&mut self, ui: &mut Ui) {
        self.apply_pending_focus_zoom();
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
            // Zones saturées / bouchées, étirées sur l'image (aussi zoomée).
            if self.clipping
                && let Some(texture) = self.mask_texture(&ctx, index)
            {
                let opacity = self.gui.histogram.clipping_opacity;
                painter.image(
                    texture.id(),
                    rect,
                    uv,
                    Color32::WHITE.gamma_multiply(opacity),
                );
            }
        }
        self.draw_focus_point(&ctx, &painter, &file, image_rect);
        // Carte de netteté : en 1:1 seulement, sur le 1:1 affiché.
        let peaking = self.peaking_overlay(
            &painter,
            &file,
            shown == Some(zoom_level) && zoomed,
            image_rect,
        );

        self.open_rating_dialog(&response, index);
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
        if self.selection.contains(&file.path) {
            info.push_str(&t!("view-info-selected"));
        }
        if self.peaking {
            info.push_str(&match peaking {
                Peaking::Shown => t!("view-info-peaking"),
                Peaking::Computing => t!("view-info-peaking-computing"),
                Peaking::NotAtOneToOne => t!("view-info-peaking-one-to-one"),
            });
        }
        match shown {
            Some(level) => {
                info.push_str(" · ");
                info.push_str(&view::label(file.kind, level));
                if let Some(ratio) = sensor_ratio {
                    info.push_str(&t!(
                        "view-info-sensor",
                        percent = (ratio * 100.0).round() as i64
                    ));
                }
            }
            None => info.push_str(&t!("view-info-preview-computing")),
        }
        // Mode RAW, mais faute de mieux c'est encore le JPEG qui est affiché : le dire.
        let raw_pending = self.source == Source::Raw
            && file.kind == MediaKind::Raw
            && matches!(
                shown,
                Some(Level::Thumbnail | Level::Embedded | Level::EmbeddedFull)
            );
        if raw_pending {
            info.push_str(&t!("view-info-raw-pending"));
        } else if zoomed && shown != Some(zoom_level) {
            info.push_str(&match zoom_level {
                Level::Full => t!("view-info-full-computing"),
                _ => t!("view-info-one-to-one-loading"),
            });
        }
        overlay(
            &painter,
            view_rect.left_top() + vec2(10.0, 10.0),
            Align2::LEFT_TOP,
            &info,
        );
        let notes = self.folder.notes.notes(&file.path);
        let size = self.gui.view.marks;
        if *notes != tri::xmp::Notes::EMPTY {
            let width = marks::row_width(notes, size);
            let left_center = view_rect.left_bottom() + vec2(15.0, -15.0 - size / 2.0);
            let back = Rect::from_min_size(left_center - vec2(0.0, size / 2.0), vec2(width, size))
                .expand(size * 0.35);
            painter.rect_filled(back, 4.0, Color32::from_black_alpha(170));
            marks::row(&painter, left_center, notes, size, &self.gui.colors);
        }
        // Aide-mémoire des touches, en haut à droite : masqué par défaut (retour du
        // 17/10/2026 : il mordait sur l'image, F1 ouvre l'aide).
        if self.gui.view.key_hints {
            let hint = match file.kind {
                MediaKind::Raw => t!("view-hint-raw"),
                MediaKind::Flat => t!("view-hint-flat"),
            };
            overlay(
                &painter,
                view_rect.right_top() + vec2(-10.0, 10.0),
                Align2::RIGHT_TOP,
                &hint,
            );
        }
    }

    /// Analyse d'exposition de l'image `index`, lue dans le cache ; demandée au moteur si
    /// elle n'y est pas encore (`None` en attendant).
    fn exposure_of(&mut self, index: usize) -> Option<Arc<Exposure>> {
        let path = self.files.get(index)?.path.clone();
        if let Some(e) = self.exposures.get(&path) {
            return Some(e.clone());
        }
        if !self.ready(index, Level::Exposure) {
            self.request(index, Level::Exposure);
            return None;
        }
        let engine = self.engine.as_ref()?;
        let e = Arc::new(preview::exposure(
            &engine.cache,
            &self.files[index],
            &engine.preview,
        )?);
        self.clip_summary
            .insert(path.clone(), (e.over_fraction(), e.under_fraction()));
        self.exposures.insert(path, e.clone());
        Some(e)
    }

    /// Masque des zones de l'image `index`, en texture (créée à la première demande).
    fn mask_texture(&mut self, ctx: &egui::Context, index: usize) -> Option<TextureHandle> {
        let path = self.files.get(index)?.path.clone();
        if let Some(t) = self.mask_textures.get(&path) {
            return Some(t.clone());
        }
        let e = self.exposure_of(index)?;
        let c = self.gui.colors;
        let zones = histogram::ZoneColors {
            channels: [c.clip_red.into(), c.clip_green.into(), c.clip_blue.into()],
            all: c.clip_over.into(),
            under: c.clip_under.into(),
            per_channel: self.gui.histogram.clipping_channels,
        };
        let image = histogram::mask_image(&e.mask, &zones);
        // Au plus proche : des cases nettes, pas un dégradé flou.
        let texture = ctx.load_texture(
            format!("{}#zones", path.display()),
            image,
            TextureOptions::NEAREST,
        );
        self.mask_textures.insert(path, texture.clone());
        Some(texture)
    }

    /// Parts de pixels saturés / bouchés de l'image `index` (badges de la grille), lues
    /// dans le cache si l'analyse est prête et que le budget de l'image le permet.
    fn clip_fractions(&mut self, index: usize, deadline: Instant) -> Option<(f64, f64)> {
        let path = &self.files.get(index)?.path;
        if let Some(&fractions) = self.clip_summary.get(path) {
            return Some(fractions);
        }
        if !self.ready(index, Level::Exposure) || Instant::now() > deadline {
            return None;
        }
        let engine = self.engine.as_ref()?;
        let e = preview::exposure(&engine.cache, &self.files[index], &engine.preview)?;
        let fractions = (e.over_fraction(), e.under_fraction());
        self.clip_summary.insert(path.clone(), fractions);
        Some(fractions)
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

    /// Barre d'état ; renvoie l'action d'une icône cliquée (afficher / masquer un
    /// panneau).
    fn status_bar(&self, ui: &mut Ui) -> Option<StatusClick> {
        let mut clicked = None;
        let size = ui.text_style_height(&egui::TextStyle::Body) + 2.0;
        let color = ui.visuals().text_color();
        // Icône d'un panneau : cadre, bande du panneau pleine quand il est affiché.
        let toggle = |ui: &mut Ui, side: marks::Side, shown: bool, action: Action| {
            let (rect, response) = marks::clickable(ui, size, &self.action_tip(action));
            marks::panel(ui.painter(), rect, side, shown, color);
            response.clicked().then_some(StatusClick::Action(action))
        };
        ui.horizontal(|ui| {
            if let Some(c) = toggle(
                ui,
                marks::Side::Left,
                self.folder_panel,
                Action::ToggleFolderPanel,
            ) {
                clicked = Some(c);
            }
            if let Some(c) = toggle(
                ui,
                marks::Side::Top,
                self.filter_bar,
                Action::ToggleFilterBar,
            ) {
                clicked = Some(c);
            }
            if let Some(c) = toggle(
                ui,
                marks::Side::Bottom,
                self.rating_bar,
                Action::ToggleRatingBar,
            ) {
                clicked = Some(c);
            }
            ui.separator();
            let raws = self
                .files
                .iter()
                .filter(|f| f.kind == MediaKind::Raw)
                .count();
            ui.label(if self.folder.is_none() {
                t!("status-no-folder")
            } else {
                folder_name(&self.folder.dir)
            });
            ui.separator();
            ui.label(t!(
                "status-images",
                n = self.files.len(),
                raws = raws,
                filter = self.filter_summary()
            ));
            let marked = self.selection.visible(&self.files);
            if marked > 0 || self.selection.is_visual() {
                ui.separator();
                let mut text = t!("status-selected", n = marked);
                if self.selection.is_visual() {
                    text.push_str(&t!("status-visual-mode"));
                }
                ui.colored_label(Color32::from(self.gui.colors.marked), text);
            }
            ui.separator();
            for (name, p) in [
                (t!("progress-thumbnails"), self.progress[0]),
                (t!("progress-embedded"), self.progress[1]),
                (t!("progress-previews"), self.progress[2]),
                (t!("progress-histograms"), self.progress[3]),
                (t!("progress-sharpness"), self.progress[4]),
            ] {
                // Rien à calculer en fond pour ce niveau, ou déjà fini.
                if p.total == 0 || (self.gui.progress_hide_done && p.done >= p.total) {
                    continue;
                }
                // Gris discret, pas la couleur d'accent : une information de fond, qui ne
                // doit pas attirer le regard (retour du 17/10/2026).
                let fill = self
                    .gui
                    .colors
                    .progress
                    .map_or(ui.visuals().widgets.inactive.bg_fill, Color32::from);
                ui.add(
                    egui::ProgressBar::new(p.fraction())
                        .desired_width(self.gui.progress_width)
                        .fill(fill)
                        .text(format!("{name} {}/{}", p.done, p.total)),
                );
            }
            if let Some(file) = self.files.get(self.selected) {
                ui.separator();
                ui.label(file_name(&file.path));
            }
            if let Some((text, error)) = &self.notice {
                ui.separator();
                if *error {
                    ui.colored_label(self.gui.colors.error, text);
                } else {
                    ui.label(text);
                }
            }
            for error in [&self.note_error, &self.file_error].into_iter().flatten() {
                ui.separator();
                ui.colored_label(self.gui.colors.error, error);
            }
            if let Some(hint) = &self.hint {
                ui.separator();
                ui.label(hint);
            }
            if !self.key_warnings.is_empty() {
                ui.separator();
                ui.colored_label(
                    self.gui.colors.error,
                    t!("status-key-problems", n = self.key_warnings.len()),
                )
                .on_hover_text(self.key_warnings.join("\n"));
            }
            if let Some(warning) = &self.warning {
                // Le message de toml tient sur plusieurs lignes (extrait du fichier) : résumé
                // ici, détail au survol, en chasse fixe pour que le « ^^^ » reste aligné.
                ui.separator();
                ui.colored_label(self.gui.colors.error, t!("status-invalid-config"))
                    .on_hover_text(egui::RichText::new(warning).monospace());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // De droite à gauche : icône du panneau des métadonnées, puis déchargement.
                if let Some(c) = toggle(
                    ui,
                    marks::Side::Right,
                    self.metadata_panel,
                    Action::ToggleMetadata,
                ) {
                    clicked = Some(c);
                }
                // Déchargement en arrière-plan : progression (clic : réafficher la boîte).
                if let Some((text, fraction)) = self
                    .offload
                    .as_ref()
                    .filter(|d| d.is_hidden())
                    .and_then(|d| d.status())
                {
                    let fill = self
                        .gui
                        .colors
                        .progress
                        .map_or(ui.visuals().widgets.inactive.bg_fill, Color32::from);
                    let bar = ui
                        .add(
                            egui::ProgressBar::new(fraction)
                                .desired_width(260.0)
                                .fill(fill)
                                .text(text),
                        )
                        .interact(Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(t!("status-show-offload"));
                    if bar.clicked() {
                        clicked = Some(StatusClick::RevealOffload);
                    }
                }
                if self.eject_failed.is_some()
                    && self.ejecting.is_none()
                    && ui.button(t!("status-retry-eject")).clicked()
                {
                    clicked = Some(StatusClick::RetryEject);
                }
                // Séance déchargée en arrière-plan : proposée, pas ouverte d'office.
                if let Some(dir) = &self.offloaded {
                    if ui.small_button(t!("status-dismiss")).clicked() {
                        clicked = Some(StatusClick::DismissOffloaded);
                    }
                    if ui
                        .button(t!("status-open-session", name = folder_name(dir)))
                        .clicked()
                    {
                        clicked = Some(StatusClick::OpenOffloaded);
                    }
                }
            });
        });
        clicked
    }

    /// Clic droit sur `response` (image `index`) : la sélectionne et ouvre la boîte de
    /// notation.
    fn open_rating_dialog(&mut self, response: &egui::Response, index: usize) {
        if response.secondary_clicked()
            && let Some(file) = self.files.get(index)
        {
            let path = file.path.clone();
            // Près du clic : pas de long trajet jusqu'au centre d'un grand écran.
            let at = response
                .interact_pointer_pos()
                .unwrap_or(response.rect.center());
            self.select(index);
            self.rating_dialog = Some((path, at));
        }
    }

    /// Grand histogramme de l'image sélectionnée, en boîte modale centrée (`Maj+E`, clic
    /// sur l'histogramme du panneau) ; se ferme par Échap, la même touche ou un clic à
    /// côté.
    /// Statistiques par canal du grand histogramme (`tri::exposure::ChannelStats`), en IL
    /// sous la saturation.
    /// Un seul canal choisi : sa ligne seulement.
    fn histogram_stats(&self, ui: &mut Ui, e: &Exposure) {
        let tail = self.gui.histogram.stats_tail;
        let ev = |v: f32| t!("stats-ev", value = tri::i18n::decimal(v as f64, 1));
        let percent = |v: f64| t!("stats-percent", value = tri::i18n::decimal(v * 100.0, 2));
        let c = &self.gui.colors;
        let channels = [
            (t!("stats-red"), c.hist_red),
            (t!("stats-green"), c.hist_green),
            (t!("stats-blue"), c.hist_blue),
        ];
        let range_title = t!(
            "stats-range",
            low = tri::i18n::decimal(tail as f64 * 100.0, 0),
            high = tri::i18n::decimal(100.0 - tail as f64 * 100.0, 0)
        );
        egui::Grid::new("histogram_stats")
            .num_columns(7)
            .spacing(vec2(18.0, 4.0))
            .show(ui, |ui| {
                for title in [
                    String::new(),
                    t!("stats-median"),
                    t!("stats-mean"),
                    range_title.clone(),
                    t!("stats-spread"),
                    t!("stats-over"),
                    t!("stats-under"),
                ] {
                    ui.weak(title);
                }
                ui.end_row();
                for (channel, (name, color)) in channels.into_iter().enumerate() {
                    if self.histogram_channel.is_some_and(|c| c != channel) {
                        continue;
                    }
                    let Some(s) = e.channel_stats(channel, tail) else {
                        continue;
                    };
                    ui.colored_label(Color32::from(color), name);
                    ui.label(ev(s.median));
                    ui.label(ev(s.mean));
                    ui.label(t!("stats-range-value", low = ev(s.low), high = ev(s.high)));
                    ui.label(ev(s.std_dev));
                    ui.label(percent(s.over));
                    ui.label(percent(s.under));
                    ui.end_row();
                }
            });
    }

    /// Image d'analyse du grand histogramme pour l'image `index` et le canal choisi
    /// ([`histogram::analysis_image`]) : vignette en gris, zones du masque d'exposition.
    fn analysis_texture(&mut self, ctx: &egui::Context, index: usize) -> Option<TextureHandle> {
        let file = self.files.get(index)?;
        let key = (file.path.clone(), self.histogram_channel);
        if let Some((k, t)) = &self.analysis_texture
            && *k == key
        {
            return Some(t.clone());
        }
        if self.status.get(index)?.thumbnail != Avail::Ready {
            return None;
        }
        let e = self.exposure_of(index)?;
        let engine = self.engine.as_ref()?;
        let file = &self.files[index];
        let thumb = engine
            .cache
            .get(&preview::key(file, Level::Thumbnail, &engine.preview).ok()?)?;
        let c = &self.gui.colors;
        let image = histogram::analysis_image(
            &thumb,
            &e.mask,
            self.histogram_channel,
            c.analysis_over.into(),
            c.analysis_under.into(),
        );
        let texture = ctx.load_texture(
            format!("{}#analysis", file.path.display()),
            image,
            TextureOptions::LINEAR,
        );
        self.analysis_texture = Some((key, texture.clone()));
        Some(texture)
    }

    /// Bloc de l'image d'analyse, à droite des statistiques : fond un peu plus clair que
    /// la boîte, image centrée, légende empilée en bas à droite (retours du 18/10/2026).
    fn analysis_block(&self, ui: &mut Ui, texture: &TextureHandle) {
        let settings = &self.gui.histogram;
        let size = vec2(ui.available_width().max(80.0), settings.analysis_height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter();
        let fill = ui.visuals().window_fill;
        let k = settings.analysis_lighten.clamp(0.0, 1.0);
        let lighten = |v: u8| (v as f32 + (255.0 - v as f32) * k) as u8;
        painter.rect_filled(
            rect,
            4.0,
            Color32::from_rgb(lighten(fill.r()), lighten(fill.g()), lighten(fill.b())),
        );
        let margin = self.gui.dialogs.spacing.max(4.0);
        let inner = rect.shrink(margin);
        // Légende : les deux mots l'un au-dessus de l'autre, en bas à droite du bloc,
        // alignés à droite ; l'image prend toute la hauteur à leur gauche (retour du
        // 18/10/2026 : la photo est petite, chaque ligne gagnée compte).
        let c = &self.gui.colors;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let galleys = [
            (t!("analysis-over"), Color32::from(c.analysis_over)),
            (t!("analysis-under"), Color32::from(c.analysis_under)),
        ]
        .map(|(text, color)| painter.layout_no_wrap(text, font.clone(), color));
        let legend_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
        let mut y = inner.max.y;
        for g in galleys.into_iter().rev() {
            y -= g.size().y;
            painter.galley(pos2(inner.max.x - g.size().x, y), g, Color32::WHITE);
        }
        let room = Rect::from_min_max(
            inner.min,
            pos2(inner.max.x - legend_w - margin, inner.max.y),
        );
        let image = texture.size_vec2();
        let fit = (room.width() / image.x)
            .min(room.height() / image.y)
            .max(0.0);
        painter.image(
            texture.id(),
            Rect::from_center_size(room.center(), image * fit),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    fn histogram_dialog(&mut self, ctx: &egui::Context) {
        if !self.histogram_large {
            return;
        }
        let exposure = self.exposure_of(self.selected);
        let analysis = self.analysis_texture(ctx, self.selected);
        let name = self
            .files
            .get(self.selected)
            .map(|f| file_name(&f.path))
            .unwrap_or_default();
        let screen = ctx.content_rect().size();
        let fraction = self.gui.histogram.large.clamp(0.2, 1.0);
        let size = vec2(screen.x * fraction, screen.y * fraction * 0.6);
        let (mut scale, mut close, mut channel) = (false, false, None);
        // Comme les autres boîtes (titre, déplaçable : retour du 17/10/2026), ouverte au
        // centre et à sa propre largeur.
        let at = ctx.content_rect().center() - size / 2.0 - vec2(20.0, 40.0);
        let dialog = modal_at("histogram_dialog", at).title(name).free_width();
        let modal = dialog.show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            match &exposure {
                Some(e) => {
                    let look = histogram::Look {
                        scale: self.histogram_scale,
                        details: true,
                        channel: self.histogram_channel,
                    };
                    histogram::draw(
                        ui.painter(),
                        rect,
                        e,
                        look,
                        &self.gui.histogram,
                        &self.gui.colors,
                    );
                    // Pastilles : rouge, vert, bleu, blanc (tous les canaux).
                    let names = [
                        t!("stats-red"),
                        t!("stats-green"),
                        t!("stats-blue"),
                        t!("stats-all"),
                    ];
                    let dots = histogram::channel_dots(rect, self.gui.histogram.channel_dot);
                    for (i, (dot, name)) in dots.into_iter().zip(names).enumerate() {
                        let tip = t!(
                            "histogram-channel-tip",
                            channel = name,
                            keys = self.keymap.keys_for(Action::HistogramChannel).join(" / ")
                        );
                        if ui
                            .interact(
                                dot.expand(3.0),
                                egui::Id::unique("histogram_channel").with(i),
                                Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            channel = Some((i < 3).then_some(i));
                        }
                    }
                }
                None => {
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        t!("histogram-computing"),
                        FontId::proportional(14.0),
                        ui.visuals().weak_text_color(),
                    );
                }
            }
            if let Some(e) = &exposure {
                // Statistiques, et à leur droite l'image d'analyse (zones du canal choisi
                // sur l'image en gris).
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| self.histogram_stats(ui, e));
                    if let Some(texture) = &analysis {
                        ui.add_space(self.gui.dialogs.spacing);
                        self.analysis_block(ui, texture);
                    }
                });
                // Sur des données RAW, pas de luminance : chaque photosite ne mesure
                // qu'une couleur ; le vert, qui en porte l'essentiel, en tient lieu. Sous
                // le tableau et l'image : sur la ligne du tableau, elle l'élargissait et
                // ne laissait presque plus de place à l'image.
                if matches!(e.source, tri::exposure::Source::Raw { .. }) {
                    ui.weak(t!("stats-raw-luminance"));
                }
            }
            ui.horizontal(|ui| {
                let label = match self.histogram_scale {
                    HistogramScale::Ev => t!("histogram-linear-scale"),
                    HistogramScale::Linear => t!("histogram-ev-scale"),
                };
                scale = ui.button(label).clicked();
                close = ui.button(t!("close")).clicked();
                ui.weak(t!("or-escape"));
            });
        });
        if scale {
            self.histogram_scale = self.histogram_scale.toggled();
        }
        if let Some(channel) = channel {
            self.histogram_channel = channel;
        }
        if close || modal.should_close() {
            self.histogram_large = false;
        }
    }

    /// Action sur les fichiers `paths` (renommer, afficher : le premier) ; `at` : position
    /// d'une boîte éventuelle.
    fn file_action(&mut self, action: Action, paths: Vec<PathBuf>, at: egui::Pos2) {
        let settings = &self.settings.files;
        self.file_error = None;
        let Some(first) = paths.first().cloned() else {
            return;
        };
        let result = match action {
            Action::FileRename => {
                self.file_dialog = Some(crate::files::FileDialog::rename(first, at));
                Ok(())
            }
            Action::FileDelete => {
                self.file_dialog = Some(crate::files::FileDialog::delete(&paths, settings, at));
                Ok(())
            }
            Action::FileReveal => tri::files::reveal(&first),
            Action::FileEdit => tri::files::open_with(&settings.editor, &paths),
            Action::FileCopy | Action::FileMove => {
                let mode = if action == Action::FileCopy {
                    tri::files::TransferMode::Copy
                } else {
                    tri::files::TransferMode::Move
                };
                self.file_dialog = Some(crate::files::FileDialog::transfer(
                    paths,
                    mode,
                    self.data_dir.as_deref(),
                    at,
                ));
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.file_error = Some(e.to_string());
        }
    }

    /// Boîte « Renommer » ou « Corbeille » ; une fois l'action faite, le dossier est relu.
    fn file_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.file_dialog else {
            return;
        };
        match dialog.show(ctx, &self.settings.files, self.data_dir.as_deref()) {
            crate::files::Outcome::Open => {}
            crate::files::Outcome::Cancelled => self.file_dialog = None,
            crate::files::Outcome::Failed(e) => {
                self.file_dialog = None;
                self.file_error = Some(e);
                // Un renommage annulé à mi-chemin est remis en place, mais une corbeille
                // partielle non : relire le dossier.
                self.reload_folder(None);
            }
            crate::files::Outcome::Done { select, step } => {
                if let Some(step) = step {
                    self.history.push(step);
                }
                // Image renommée sélectionnée : elle le reste sous son nouveau nom.
                if let (Some(crate::files::FileDialog::Rename { image, .. }), Some(to)) =
                    (&self.file_dialog, &select)
                {
                    self.selection.renamed(image, to);
                }
                self.file_dialog = None;
                self.reload_folder(select);
            }
        }
    }

    /// Relit le dossier ouvert après une action sur ses fichiers, en gardant la vue (grille
    /// ou plein écran, zoom) ; `select` : image à sélectionner (nouveau nom), sinon la
    /// sélection garde sa place (l'image suivante, après une suppression).
    fn reload_folder(&mut self, select: Option<PathBuf>) {
        match Folder::open(&self.folder.dir, 1, self.settings.xmp.sidecar) {
            Ok(folder) => self.folder = folder,
            Err(e) => {
                self.file_error = Some(format!("{} : {e}", self.folder.dir.display()));
                return;
            }
        }
        self.wanted.clear();
        self.missing.clear();
        self.selection.retain_existing();
        if let Some(meta) = &mut self.meta {
            meta.clear();
        }
        self.refilter(Reveal::Minimal);
        if let Some(index) = select.and_then(|p| self.files.iter().position(|f| f.path == p)) {
            self.selected = index;
            self.tell_shown();
        }
        if self.files.is_empty() {
            self.mode = Mode::Grid;
        }
    }

    /// Boîte de notation (clic droit) : modale, pour l'image cliquée. Se ferme par Échap,
    /// un clic à côté ou « Fermer » ; reste ouverte pour poser plusieurs notes.
    fn rating_dialog(&mut self, ctx: &egui::Context) {
        let Some((path, at)) = self.rating_dialog.clone() else {
            return;
        };
        let notes = self.folder.notes.notes(&path).clone();
        let (colors, size) = (self.gui.colors, self.gui.grid.marks + 8.0);
        let mut clicked = None;
        let mut file_action = None;
        let mut close = false;
        let modal = modal_at("rating_dialog", at)
            .title(file_name(&path))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| clicked = marks::controls(ui, &notes, &colors, size));
                ui.add_space(6.0);
                ui.separator();
                // Actions sur le fichier.
                // Icônes carrées, libellé et raccourci au survol (retour du 17/10/2026).
                let icon = self.gui.dialogs.icon_size;
                ui.horizontal(|ui| {
                    for action in [
                        Action::FileRename,
                        Action::FileCopy,
                        Action::FileMove,
                        Action::FileDelete,
                        Action::FileReveal,
                        Action::FileEdit,
                    ] {
                        let (rect, response) =
                            marks::icon_button(ui, icon, &action.label(), &self.action_tip(action));
                        let color = ui.style().interact(&response).fg_stroke.color;
                        marks::file_action(ui.painter(), rect.shrink(icon * 0.22), action, color);
                        if response.clicked() {
                            file_action = Some(action);
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    close = ui.button(t!("close")).clicked();
                    ui.weak(t!("or-escape"));
                });
            });
        if let Some(action) = clicked {
            self.rate_path(path.clone(), action);
        }
        if let Some(action) = file_action {
            self.rating_dialog = None;
            self.file_action(action, vec![path], at);
            return;
        }
        if close || modal.should_close() {
            self.rating_dialog = None;
        }
    }

    /// Bandeau de notation : notes de l'image sélectionnée, cliquables.
    fn rating_bar(&mut self, ui: &mut Ui) {
        let Some(file) = self.files.get(self.selected) else {
            return;
        };
        let notes = self.folder.notes.notes(&file.path).clone();
        let name = file_name(&file.path);
        let (colors, size) = (self.gui.colors, self.gui.grid.marks + 4.0);
        let marked = self.selection.visible(&self.files);
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.label(t!("rating-bar-title"));
            clicked = marks::controls(ui, &notes, &colors, size);
            ui.weak(name);
            // En comparaison, seule l'image active est notée.
            if marked > 1 && self.mode != Mode::Compare {
                ui.colored_label(
                    Color32::from(self.gui.colors.marked),
                    t!("rating-bar-applies", n = marked),
                );
            }
            // À droite (posés de droite à gauche) : avance automatique, puis image suivante
            // et précédente, des icônes (retour du 17/10/2026).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let icon = self.gui.bar_icon_size;
                let zoomed = self.mode != Mode::Grid && self.zoom.is_some();
                for action in [
                    Action::ToggleAutoAdvance,
                    Action::NavNext,
                    Action::NavPrev,
                    Action::ToggleZoom,
                ] {
                    let (rect, response) =
                        marks::icon_button(ui, icon, &action.label(), &self.action_tip(action));
                    let active = match action {
                        Action::ToggleAutoAdvance => self.auto_advance,
                        Action::ToggleZoom => zoomed,
                        _ => false,
                    };
                    if active {
                        ui.painter()
                            .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                    }
                    let color = ui.style().interact(&response).fg_stroke.color;
                    marks::bar_icon(
                        ui.painter(),
                        rect.shrink(icon * 0.2),
                        action,
                        color,
                        &colors,
                    );
                    if response.clicked() {
                        clicked = Some(action);
                    }
                }
            });
        });
        match clicked {
            Some(Action::ToggleAutoAdvance) => self.auto_advance = !self.auto_advance,
            Some(action @ (Action::NavPrev | Action::NavNext | Action::ToggleZoom)) => {
                let ctx = ui.ctx().clone();
                self.perform(&ctx, action);
            }
            Some(action) => self.rate_selected(action),
            None => {}
        }
    }

    /// Filtres actifs en clair (« RAW · 3 étoiles et plus · sans les rejetées »).
    fn filter_summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.filter != KindFilter::All {
            parts.push(filter_name(self.filter));
        }
        if let Some(StarsRule::AtLeast(n)) = self.notes_rule.stars {
            parts.push(t!("filter-summary-stars", n = n));
        }
        if let Some(flags) = &self.notes_rule.flags
            && let Some(choice) = FlagFilter::ALL.into_iter().find(|c| c.flags() == *flags)
        {
            parts.push(choice.label());
        }
        if let Some(colors) = &self.notes_rule.colors {
            let names: Vec<String> = colors.any_of.iter().map(|&c| color_name(c)).collect();
            parts.push(names.join(&t!("filter-summary-or")));
        }
        match self.blur_rule {
            Some(tri::sharpness::BlurRule::HideBlurry) => parts.push(t!("filter-bar-no-blurry")),
            Some(tri::sharpness::BlurRule::OnlyBlurry) => parts.push(t!("filter-bar-only-blurry")),
            None => {}
        }
        if parts.is_empty() {
            t!("filter-all")
        } else {
            parts.join(" · ")
        }
    }

    /// Barre de filtres, à la souris : chaque clic déclenche la même action que le clavier.
    fn filter_bar(&mut self, ui: &mut Ui) {
        let mut clicked: Vec<Action> = Vec::new();
        let colors = self.gui.colors;
        // Passe à la ligne si la fenêtre est étroite (panneau des dossiers ouvert).
        // Icônes et textes courts, détail au survol (retour du 17/10/2026 : sur un écran
        // de portable, la barre débordait).
        ui.horizontal_wrapped(|ui| {
            let type_tip = self.action_tip(Action::FilterCycle);
            for (kind, label) in [
                (KindFilter::All, t!("filter-bar-all")),
                (KindFilter::RawOnly, "RAW".to_string()),
                (KindFilter::FlatOnly, t!("filter-bar-flat")),
            ] {
                if ui
                    .selectable_label(self.filter == kind, label)
                    .on_hover_text(&type_tip)
                    .clicked()
                {
                    // Le type tourne en boucle : autant de crans que nécessaire.
                    let order = [KindFilter::All, KindFilter::RawOnly, KindFilter::FlatOnly];
                    let from = order.iter().position(|&k| k == self.filter).unwrap_or(0);
                    let to = order.iter().position(|&k| k == kind).unwrap_or(0);
                    for _ in 0..(to + 3 - from) % 3 {
                        clicked.push(Action::FilterCycle);
                    }
                }
            }
            ui.separator();
            let current = match self.notes_rule.stars {
                Some(StarsRule::AtLeast(n)) => n,
                _ => 0,
            };
            let size = self.gui.grid.marks + 4.0;
            for n in 1..=5u8 {
                // Recliquer l'étoile du filtre en cours l'efface.
                let tip = Action::FilterStars(if n == current { 0 } else { n }).label();
                let (rect, response) = marks::clickable(ui, size, &tip);
                let color = if n <= current {
                    Color32::from(colors.stars)
                } else {
                    ui.visuals().weak_text_color()
                };
                marks::star(ui.painter(), rect.center(), size / 2.0, color);
                if response.clicked() {
                    clicked.push(Action::FilterStars(if n == current { 0 } else { n }));
                }
            }
            ui.separator();
            for choice in FlagFilter::ALL {
                let active = self.notes_rule.flags.as_ref() == Some(&choice.flags());
                let tip = self.action_tip(Action::FilterFlag(choice));
                let (rect, response) = marks::clickable(ui, size, &tip);
                if active {
                    ui.painter()
                        .rect_filled(rect.expand(2.0), 4.0, ui.visuals().selection.bg_fill);
                }
                marks::flag_filter(ui.painter(), rect, choice, &colors);
                if response.clicked() {
                    clicked.push(Action::FilterFlag(choice));
                }
            }
            ui.separator();
            // Goutte barrée : masquer les floues ; goutte : seulement les floues.
            for (rule, action, barred) in [
                (
                    tri::sharpness::BlurRule::HideBlurry,
                    Action::FilterBlurry,
                    true,
                ),
                (
                    tri::sharpness::BlurRule::OnlyBlurry,
                    Action::FilterOnlyBlurry,
                    false,
                ),
            ] {
                let tip = self.action_tip(action);
                let (rect, response) = marks::clickable(ui, size, &tip);
                if self.blur_rule == Some(rule) {
                    ui.painter()
                        .rect_filled(rect.expand(2.0), 4.0, ui.visuals().selection.bg_fill);
                }
                let color = Color32::from(colors.blur_drop);
                marks::blur_drop(ui.painter(), rect, color, barred);
                if response.clicked() {
                    clicked.push(action);
                }
            }
            ui.separator();
            for color in Color::ALL {
                let tip = self.action_tip(Action::FilterColor(color));
                let (rect, response) = marks::clickable(ui, size, &tip);
                let active = self
                    .notes_rule
                    .colors
                    .as_ref()
                    .is_some_and(|r| r.any_of.contains(&color));
                let painter = ui.painter();
                painter.circle_filled(rect.center(), size * 0.35, marks::color_of(&colors, color));
                if active {
                    painter.circle_stroke(
                        rect.center(),
                        size * 0.48,
                        egui::Stroke::new(2.0, ui.visuals().strong_text_color()),
                    );
                }
                if response.clicked() {
                    clicked.push(Action::FilterColor(color));
                }
            }
            ui.separator();
            let any = self.filter != KindFilter::All
                || !self.notes_rule.is_empty()
                || self.blur_rule.is_some();
            if ui
                .add_enabled(any, egui::Button::new(t!("filter-bar-clear")))
                .on_hover_text(self.action_tip(Action::FilterClear))
                .clicked()
            {
                clicked.push(Action::FilterClear);
            }
            ui.weak(t!(
                "filter-bar-count",
                shown = self.files.len(),
                n = self.folder.files.len()
            ))
            .on_hover_text(t!("filter-bar-count-tip"));
        });
        let ctx = ui.ctx().clone();
        for action in clicked {
            self.perform(&ctx, action);
        }
    }

    fn grid(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        #[cfg(test)]
        self.cell_rects.clear();
        self.hovered = None;
        let g = self.gui.grid;
        self.layout = Layout::fit(ui.available_width(), g.cell, g.gap);
        let rows = self.layout.rows(self.files.len());
        ui.spacing_mut().item_spacing = vec2(g.gap, g.gap);
        let mut area = ScrollArea::vertical().auto_shrink(false);
        let row = self.selected / self.layout.columns;
        let offset = match self.reveal.take() {
            Some(Reveal::Top) => Some(row as f32 * self.layout.row_pitch()),
            // Grille pas encore mise en page (hauteur visible inconnue, 0) : on attend
            // l'image suivante, sinon la rangée de l'image passait sous le haut de la vue
            // (actions dès le démarrage, `TRI_GUI_ACTIONS`).
            Some(Reveal::Minimal) if self.view_height <= 0.0 => {
                self.reveal = Some(Reveal::Minimal);
                None
            }
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
        #[cfg(test)]
        self.cell_rects.push((index, rect));
        // Le clic sélectionne ; le survol ne fait qu'encadrer (retour du 17/10/2026 : quand le
        // survol sélectionnait, aller vers le bandeau de notation changeait d'image).
        if response.clicked() {
            // Maj+clic : plage depuis l'image courante ; Cmd / Ctrl+clic : ajouter ou retirer
            // une image ; clic simple : sélection effacée.
            let modifiers = ctx.input(|i| i.modifiers);
            let path = self.files[index].path.clone();
            if modifiers.shift {
                self.selection.add_range(&self.files, self.selected, index);
            } else if modifiers.command || modifiers.ctrl {
                self.selection.toggle(&path);
            } else if !self.selection.is_visual() {
                self.selection.clear();
            }
            self.select(index);
        }
        self.open_rating_dialog(&response, index);
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
        } else {
            visuals.extreme_bg_color
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, background);
        let marked = self.selection.contains(&self.files[index].path);
        if marked {
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(g.marked_frame, Color32::from(colors.marked)),
                egui::StrokeKind::Inside,
            );
        }
        // Survol : cadre gris clair, bien distinct de la sélection (fond plein). Pas quand
        // on tape : le pointeur est masqué, le cadre l'est aussi.
        if response.hovered() && self.mouse_active && !selected && !marked {
            self.hovered = Some(index);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(g.hover_frame, Color32::from(colors.hover)),
                egui::StrokeKind::Inside,
            );
        }

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
                // Photo rejetée : estompée, comme dans Lightroom.
                let tint = match self.folder.notes.notes(&file.path).flag {
                    Flag::Rejected => Color32::WHITE.gamma_multiply(g.rejected_opacity),
                    _ => Color32::WHITE,
                };
                ui.painter().image(texture.id(), dest, uv, tint);
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
        let badge_rect = ui.painter().text(
            pos2(rect.max.x - g.padding, rect.max.y - 4.0),
            Align2::RIGHT_BOTTOM,
            badge,
            FontId::monospace(11.0),
            colors.badge.into(),
        );
        // Photo jugée floue (score de netteté sous la médiane du dossier).
        if g.blur_badge && self.is_blurry(&file.path) {
            ui.painter().text(
                pos2(badge_rect.min.x - 6.0, rect.max.y - 4.0),
                Align2::RIGHT_BOTTOM,
                t!("badge-blurry"),
                FontId::proportional(11.0),
                colors.warning.into(),
            );
        }
        // Notes en bas de la vignette, sur un fond sombre pour rester lisibles.
        let notes = self.folder.notes.notes(&file.path);
        if *notes != tri::xmp::Notes::EMPTY {
            let size = g.marks;
            let left_center = pos2(image_area.min.x + 3.0, image_area.max.y - 3.0 - size / 2.0);
            let width = marks::row_width(notes, size);
            let back = Rect::from_min_size(left_center - vec2(0.0, size / 2.0), vec2(width, size))
                .expand(2.0);
            let painter = ui.painter();
            painter.rect_filled(back, 3.0, Color32::from_black_alpha(160));
            marks::row(painter, left_center, notes, size, &colors);
        }
        // Badges d'exposition en haut à droite : brûlée (rouge), bouchée (bleu).
        if let Some((over, under)) = self.clip_fractions(index, deadline) {
            let h = self.gui.histogram;
            let radius = g.marks * 0.4;
            let mut center = pos2(
                image_area.max.x - radius - 2.0,
                image_area.min.y + radius + 2.0,
            );
            let mut tips = Vec::new();
            for (fraction, threshold, color, what) in [
                (over, h.badge_over, colors.clip_over, "badge-over"),
                (under, h.badge_under, colors.clip_under, "badge-under"),
            ] {
                if fraction > threshold {
                    let painter = ui.painter();
                    painter.circle_filled(center, radius + 1.5, Color32::from_black_alpha(160));
                    painter.circle_filled(center, radius, color);
                    center.x -= radius * 2.0 + 4.0;
                    let percent = tri::i18n::decimal((fraction * 1000.0).round() / 10.0, 1);
                    tips.push(t!(what, percent = percent));
                }
            }
            if !tips.is_empty() {
                response.on_hover_text(tips.join("\n"));
            }
        }
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
                Step::RatingDialog => {
                    if let Some(file) = self.files.get(self.selected) {
                        let at = ctx.content_rect().center() - vec2(160.0, 80.0);
                        self.rating_dialog = Some((file.path.clone(), at));
                    }
                }
            }
        }
    }

    /// Après une annulation ou un rétablissement : dossier relu si des fichiers ont bougé
    /// (sinon seulement les notes), image concernée montrée, et message dans la barre
    /// d'état ; refus (fichier modifié ailleurs…) : la raison.
    fn after_history(
        &mut self,
        result: Result<Vec<tri::history::Applied>, Option<tri::history::HistoryError>>,
    ) {
        let (applied, error) = match result {
            Ok(applied) => (applied, None),
            Err(e) => (Vec::new(), e),
        };
        self.after_applied(applied, error);
    }

    fn after_applied(
        &mut self,
        applied: Vec<tri::history::Applied>,
        error: Option<tri::history::HistoryError>,
    ) {
        if let Some(last) = applied.last() {
            let show = last
                .images
                .iter()
                .find(|p| p.parent() == Some(&self.folder.dir))
                .cloned();
            if applied.iter().any(|a| a.files_changed) {
                self.reload_folder(show.clone());
            } else {
                self.folder.notes.reload();
                self.refilter(Reveal::Minimal);
            }
            if let Some(path) = show
                && let Some(i) = self.files.iter().position(|f| f.path == path)
            {
                self.select(i);
            }
            let text = if last.undone {
                t!("history-undone", label = last.label.clone())
            } else {
                t!("history-redone", label = last.label.clone())
            };
            self.notice = Some((text, false));
        }
        self.file_error = error.map(|e| t!("history-refused", error = e.to_string()));
    }

    /// Liste de l'historique (`Ctrl+H`) : les pas de la session, les plus récents en haut ;
    /// cliquer un pas y revient (en défaisant ou refaisant ce qui le suit ou le précède).
    fn history_dialog(&mut self, ctx: &egui::Context) {
        if !self.history_open {
            return;
        }
        let mut target = None;
        let (mut undo, mut redo, mut close) = (false, false, false);
        let at = ctx.content_rect().center() - vec2(220.0, 200.0);
        let position = self.history.position();
        let modal = modal_at("history_dialog", at)
            .title(t!("history-title"))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    // Les plus récents en haut ; l'état de départ en bas.
                    for (i, entry) in self.history.entries().iter().enumerate().rev() {
                        let done = i < position;
                        let current = i + 1 == position;
                        let mut text = egui::RichText::new(&entry.label);
                        if !done {
                            text = text.weak().strikethrough();
                        }
                        if current {
                            text = text.strong();
                        }
                        if ui.selectable_label(current, text).clicked() {
                            target = Some(i + 1);
                        }
                    }
                    let start = egui::RichText::new(t!("history-start"));
                    if ui
                        .selectable_label(position == 0, start.italics())
                        .clicked()
                    {
                        target = Some(0);
                    }
                });
                if self.history.entries().is_empty() {
                    ui.weak(t!("history-empty"));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let tip = |a: Action| self.keymap.keys_for(a).join(" / ");
                    undo = ui
                        .add_enabled(
                            self.history.can_undo(),
                            egui::Button::new(t!("history-undo")),
                        )
                        .on_hover_text(tip(Action::Undo))
                        .clicked();
                    redo = ui
                        .add_enabled(
                            self.history.can_redo(),
                            egui::Button::new(t!("history-redo")),
                        )
                        .on_hover_text(tip(Action::Redo))
                        .clicked();
                    close = ui.button(t!("close")).clicked();
                    ui.weak(t!("or-escape"));
                });
            });
        let convention = self.settings.xmp.sidecar;
        if let Some(target) = target {
            let (applied, error) = self.history.go_to(target, convention);
            self.after_applied(applied, error);
        }
        if undo {
            let result = self.history.undo(convention);
            self.after_history(result.map(|a| vec![a]).map_err(Some));
        }
        if redo {
            let result = self.history.redo(convention);
            self.after_history(result.map(|a| vec![a]).map_err(Some));
        }
        if close || modal.should_close() {
            self.history_open = false;
        }
    }

    /// Ouvre la fenêtre de réglages (ou dit pourquoi c'est impossible).
    fn open_settings(&mut self) {
        let Some(path) = self
            .config_path
            .clone()
            .or_else(tri::settings::default_path)
        else {
            self.file_error = Some(t!("settings-no-path"));
            return;
        };
        match crate::settings_window::SettingsWindow::open(&path) {
            Ok(w) => {
                self.help = false;
                self.settings_window = Some(w);
            }
            Err(e) => self.file_error = Some(e),
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let Some(window) = &mut self.settings_window else {
            return;
        };
        match window.show(ctx) {
            crate::settings_window::Outcome::Open => {}
            crate::settings_window::Outcome::Closed => self.settings_window = None,
            crate::settings_window::Outcome::Changed(new) => self.apply_settings(ctx, *new),
        }
    }

    /// Liste des actions, de leurs identifiants et de leurs touches.
    fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = self.help;
        let opening = self.help_search.is_none();
        let mut query = self.help_search.take().unwrap_or_default();
        egui::Window::new(t!("help-title"))
            .open(&mut open)
            .min_width(self.gui.dialogs.help_width)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                // Recherche floue à la fzf : on tape dès l'ouverture.
                let search = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .id(help_search_id())
                        .hint_text(t!("help-search-hint"))
                        .desired_width(f32::INFINITY),
                );
                if opening {
                    search.request_focus();
                }
                ui.add_space(4.0);
                let rows = help_rows(&self.keymap, &query);
                // Défilante : la liste des actions dépasse la hauteur d'un petit écran.
                let max_height = ui.ctx().content_rect().height() * 0.75;
                // Toute la largeur de la boîte : l'ascenseur au bord, les bandes du tableau
                // jusqu'à lui (retour du 18/10/2026 : la boîte élargie gardait une zone de
                // texte étroite).
                ScrollArea::vertical()
                    .max_height(max_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            ui.weak(t!("help-no-match"));
                        }
                        let column = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                        egui::Grid::new("help")
                            .striped(true)
                            .min_col_width(column)
                            .show(ui, |ui| {
                                let mut group = None;
                                for (action, keys) in &rows {
                                    // Sans recherche : rangées par thème, avec un titre.
                                    if query.trim().is_empty() && group != Some(action.group()) {
                                        group = Some(action.group());
                                        ui.label(
                                            egui::RichText::new(action.group().label()).strong(),
                                        );
                                        ui.end_row();
                                    }
                                    // Identifiant technique au survol seulement (utile pour la
                                    // section `[keys]` de la configuration).
                                    ui.label(action.label()).on_hover_text(action.id());
                                    // Chasse fixe : « l » et « I » restent distincts.
                                    ui.label(egui::RichText::new(keys).monospace().strong());
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(6.0);
                ui.weak(t!("help-close-hint"));
            });
        self.help_search = Some(query);
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
            && !(matches!(self.mode, Mode::View | Mode::Compare) && self.view_waiting);
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
        ctx.data_mut(|d| d.insert_temp(dialog_settings_id(), self.gui.dialogs));
        if self.engine.is_none() && self.error.is_none() {
            self.start_engine(&ctx);
        }
        self.drain_events();
        self.watch_volumes(&ctx);
        self.poll_eject();
        self.run_script(&ctx);
        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.mouse_active = true;
        }
        self.handle_keys(&ctx);
        self.update_loads(&ctx);

        let status_click = egui::Panel::bottom("status")
            .show(ui, |ui| self.status_bar(ui))
            .inner;
        match status_click {
            Some(StatusClick::Action(action)) => self.perform(&ctx, action),
            Some(StatusClick::RevealOffload) => {
                if let Some(dialog) = &mut self.offload {
                    dialog.reveal();
                }
            }
            Some(StatusClick::OpenOffloaded) => {
                if let Some(dir) = self.offloaded.take() {
                    self.open_offloaded(&dir);
                }
            }
            Some(StatusClick::DismissOffloaded) => self.offloaded = None,
            Some(StatusClick::RetryEject) => {
                if let Some(volume) = self.eject_failed.take() {
                    self.start_eject(&ctx, volume);
                }
            }
            None => {}
        }
        if self.metadata_panel && self.error.is_none() && !self.folder.is_none() {
            egui::Panel::right("metadata")
                .resizable(true)
                .default_size(self.gui.metadata.width)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| self.metadata_panel(ui));
                });
        }
        if self.folder_panel && self.mode == Mode::Grid {
            egui::Panel::left("folders")
                .resizable(true)
                .default_size(self.gui.folders.width)
                .show(ui, |ui| self.folder_panel(ui));
        }
        if self.rating_bar && !self.files.is_empty() && self.error.is_none() {
            egui::Panel::bottom("rating").show(ui, |ui| self.rating_bar(ui));
        }
        if self.filter_bar && self.mode == Mode::Grid && self.error.is_none() {
            egui::Panel::top("filters").show(ui, |ui| self.filter_bar(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(self.gui.colors.error, error);
            } else if self.files.is_empty() {
                ui.label(if self.folder.is_none() {
                    t!("empty-no-folder")
                } else if self.folder.files.is_empty() {
                    t!("empty-folder")
                } else {
                    t!("empty-filtered")
                });
            } else if self.mode == Mode::View {
                self.viewer(ui);
            } else if self.mode == Mode::Compare {
                self.compare_view(ui);
            } else {
                self.grid(ui);
            }
        });
        if self.help {
            self.help_window(&ctx);
        } else {
            self.help_search = None;
        }
        self.settings_window(&ctx);
        self.history_dialog(&ctx);
        self.rating_dialog(&ctx);
        self.file_dialog(&ctx);
        self.eject_dialog(&ctx);
        self.histogram_dialog(&ctx);
        self.offload_dialog(&ctx);
        // Clavier en cours d'usage : pointeur masqué (dernier mot sur l'icône de l'image).
        if !self.mouse_active {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        self.save_display();
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
    /// Ouvrir la boîte du clic droit pour l'image sélectionnée (`rating_dialog`), qu'aucune
    /// action clavier n'ouvre.
    RatingDialog,
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
                None if id == "rating_dialog" => Some(Step::RatingDialog),
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
        Level::Exposure => Some(3),
        Level::Sharpness => Some(4),
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
            Level::Exposure => s.exposure = avail,
            Level::Sharpness => s.sharpness = avail,
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

/// Clic dans la barre d'état.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusClick {
    /// Icône d'un panneau.
    Action(Action),
    /// Progression d'un déchargement en arrière-plan : réafficher sa boîte.
    RevealOffload,
    /// Ouvrir la séance déchargée en arrière-plan.
    OpenOffloaded,
    /// Ne plus la proposer.
    DismissOffloaded,
    /// Réessayer une éjection refusée.
    RetryEject,
}

/// Boîte modale ouverte près de `at` (le clic qui l'ouvre), plutôt qu'au centre de la
/// fenêtre : sur un grand écran, le trajet jusqu'au centre est long (retour du
/// 17/10/2026). Gardée dans la fenêtre, **déplaçable** en glissant son fond, aérée
/// (`[gui.dialogs]`) ; derrière, le fond est grisé et bloqué comme pour toute modale.
pub(crate) fn modal_at(id: &str, at: egui::Pos2) -> Dialog {
    // Une position par ouverture : la boîte rouverte ailleurs repart de son nouveau clic.
    let id = egui::Id::unique(id).with((at.x as i32, at.y as i32));
    Dialog {
        id,
        start: at + vec2(4.0, 4.0),
        title: None,
        free_width: false,
    }
}

/// Boîte de [`modal_at`].
pub(crate) struct Dialog {
    id: egui::Id,
    /// Position initiale (coin haut gauche).
    start: egui::Pos2,
    /// Titre, dessiné en couleur d'accent au-dessus d'une ligne de séparation.
    title: Option<String>,
    /// Sans largeurs minimale et maximale (`[gui.dialogs]`) : la boîte prend la taille de
    /// son contenu (grand histogramme).
    free_width: bool,
}

impl Dialog {
    /// Titre de la boîte.
    pub(crate) fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Largeur libre (voir [`Dialog::free_width`]).
    pub(crate) fn free_width(mut self) -> Self {
        self.free_width = true;
        self
    }

    /// Affiche la boîte, mise en forme selon `[gui.dialogs]` (réglages déposés dans le
    /// contexte à chaque image, voir [`dialog_settings_id`]). La `Modal` d'egui n'est pas
    /// déplaçable (sa zone couvre aussi le fond grisé) : la position est gardée ici et
    /// décalée quand on glisse le fond de la boîte (les boutons et champs, créés après,
    /// restent prioritaires).
    pub(crate) fn show<R>(
        self,
        ctx: &egui::Context,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> egui::ModalResponse<R> {
        let d: crate::settings::DialogSettings = ctx
            .data(|data| data.get_temp(dialog_settings_id()))
            .unwrap_or_default();
        let pos_id = self.id.with("position");
        let pos = ctx
            .data(|data| data.get_temp::<egui::Pos2>(pos_id))
            .unwrap_or(self.start);
        let area = egui::Area::new(self.id)
            .kind(egui::UiKind::Modal)
            .sense(Sense::hover())
            .order(egui::Order::Foreground)
            .interactable(true)
            .fixed_pos(pos)
            .constrain(true);
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(d.margin);
        let mut moved = egui::Vec2::ZERO;
        // Le cadre est dessiné dans la zone de glissement : marges comprises, toute la boîte
        // se saisit.
        let modal = egui::Modal::new(self.id)
            .area(area)
            .frame(egui::Frame::NONE);
        let response = modal.show(ctx, |ui| {
            let scope = ui.scope_builder(egui::UiBuilder::new().sense(Sense::drag()), |ui| {
                frame
                    .show(ui, |ui| {
                        let style = ui.style_mut();
                        // Textes non sélectionnables : glisser le titre déplace la boîte.
                        style.interaction.selectable_labels = false;
                        // Main sur ce qui se clique (boutons, sections, cases) : sans
                        // elle, le pointeur « saisir » de la boîte déplaçable restait
                        // partout (retour du 18/10/2026).
                        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
                        style
                            .text_styles
                            .insert(egui::TextStyle::Heading, FontId::proportional(d.title_size));
                        let spacing = ui.spacing_mut();
                        spacing.item_spacing = vec2(d.spacing, d.spacing);
                        spacing.button_padding = vec2(d.button_padding, d.button_padding / 2.0);
                        if !self.free_width {
                            ui.set_min_width(d.min_width);
                            ui.set_max_width(d.max_width.max(d.min_width));
                        }
                        if let Some(title) = &self.title {
                            let color = Color32::from(d.title_color);
                            ui.label(egui::RichText::new(title).size(d.title_size).color(color));
                            ui.separator();
                        }
                        add(ui)
                    })
                    .inner
            });
            if scope.response.dragged() {
                moved = scope.response.drag_delta();
            }
            // Sans écraser le pointeur choisi par un élément de la boîte (main sur les
            // pastilles du grand histogramme : retour du 18/10/2026).
            // Sous le pointeur, un élément qui se clique (case, radio, icône dessinée :
            // `interact_cursor` ne vaut que pour les boutons d'egui) : la main ; ailleurs,
            // « saisir » pour déplacer la boîte (retours du 18/10/2026).
            if scope.response.hovered()
                && ctx.output(|o| o.cursor_icon) == egui::CursorIcon::Default
            {
                let clickable =
                    ctx.viewport(|v| v.hits.click.is_some_and(|w| w.sense.senses_click()));
                ctx.set_cursor_icon(if clickable {
                    egui::CursorIcon::PointingHand
                } else {
                    egui::CursorIcon::Grab
                });
            }
            scope.inner
        });
        if moved != egui::Vec2::ZERO {
            ctx.data_mut(|data| data.insert_temp(pos_id, pos + moved));
            ctx.request_repaint();
        }
        response
    }
}

/// Emplacement des réglages des boîtes dans les données du contexte egui.
pub(crate) fn dialog_settings_id() -> egui::Id {
    egui::Id::unique("tri-dialog-settings")
}

/// Nom d'une couleur de notation.
fn color_name(color: Color) -> String {
    match color {
        Color::Red => t!("color-red"),
        Color::Yellow => t!("color-yellow"),
        Color::Green => t!("color-green"),
        Color::Blue => t!("color-blue"),
        Color::Purple => t!("color-purple"),
    }
}

fn filter_name(filter: KindFilter) -> String {
    match filter {
        KindFilter::All => t!("filter-all"),
        KindFilter::RawOnly => "RAW".into(),
        KindFilter::FlatOnly => t!("filter-flat"),
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

/// Identifiant du champ de recherche de l'aide.
fn help_search_id() -> egui::Id {
    egui::Id::unique("help_search")
}

/// Lignes de l'aide : (action, raccourcis). Sans recherche, toutes les actions rangées par
/// thème ; avec une recherche, celles qui y correspondent (nom, raccourci ou identifiant),
/// les meilleures d'abord.
fn help_rows(keymap: &Keymap, query: &str) -> Vec<(Action, String)> {
    let mut rows: Vec<(Action, String, i32)> = Action::ALL
        .iter()
        .filter_map(|&action| {
            let keys = keymap.keys_for(action).join(" / ");
            let label = action.label();
            let score = [label.as_str(), keys.as_str(), action.id()]
                .iter()
                .filter_map(|text| crate::actions::fuzzy_score(query, text))
                .max()?;
            Some((action, keys, score))
        })
        .collect();
    if query.trim().is_empty() {
        let rank = |a: Action| Group::ALL.iter().position(|g| *g == a.group());
        rows.sort_by_key(|(a, _, _)| rank(*a)); // stable : ordre d'origine dans un thème
    } else {
        // À score égal, le nom le plus court (le plus proche de la requête) d'abord.
        rows.sort_by_key(|(a, _, score)| (std::cmp::Reverse(*score), a.label().len()));
    }
    rows.into_iter().map(|(a, k, _)| (a, k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event as UiEvent, Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Application sur un dossier temporaire de 3 JPEG, avec un cache temporaire.
    fn harness() -> (Harness<'static, TriApp>, tempfile::TempDir) {
        harness_with(64, 48)
    }

    /// Application sur 3 JPEG de `w`×`h` pixels, avec un cache temporaire.
    fn harness_with(w: u32, h: u32) -> (Harness<'static, TriApp>, tempfile::TempDir) {
        harness_files(&["img0.jpg", "img1.jpg", "img2.jpg"], w, h)
    }

    /// Application sur les images `names` (JPEG ou PNG selon l'extension) de `w`×`h`
    /// pixels, avec un cache temporaire.
    fn harness_files(
        names: &[&str],
        w: u32,
        h: u32,
    ) -> (Harness<'static, TriApp>, tempfile::TempDir) {
        let images = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let pixel = image::Rgb([40 * i as u8, 100, 160]);
                (*name, image::RgbImage::from_pixel(w, h, pixel))
            })
            .collect();
        harness_images(images)
    }

    /// Application sur les images `images` (nom, pixels), avec un cache temporaire.
    fn harness_images(
        images: Vec<(&str, image::RgbImage)>,
    ) -> (Harness<'static, TriApp>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        for (name, img) in &images {
            img.save(photos.join(name)).unwrap();
        }
        let mut settings = AppSettings::default();
        settings.cache.dir = Some(dir.path().join("cache"));
        // Pas de surveillance des volumes : les tests fixent leur propre arborescence.
        settings.places.watch_ms = 0;
        // Déchargements : historique et racine dans le dossier temporaire.
        settings.offload.history = dir.path().join("history.tsv").display().to_string();
        settings.offload.destination = dir.path().join("photos-local").display().to_string();
        // Jamais d'éjection réelle dans les tests : la « carte » est un dossier temporaire,
        // sur le disque du système.
        settings.offload.eject_after = false;
        // Données (dernier dossier…) dans le dossier temporaire : jamais celles de
        // l'utilisateur.
        settings.data.dir = Some(dir.path().join("data"));
        // Textes en français (les tests cherchent les libellés), quelle que soit la langue
        // du système.
        settings.i18n.language = tri::i18n::LanguageSetting::Fr;
        crate::i18n::init(settings.i18n.language);
        // Jamais la corbeille de l'utilisateur : suppression définitive, dans le dossier
        // temporaire.
        settings.files.trash = false;
        let harness = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(Some(photos), settings, None));
        (harness, dir)
    }

    fn press(h: &mut Harness<'_, TriApp>, key: Key, modifiers: Modifiers) {
        h.key_press_modifiers(modifiers, key);
        h.run_steps(3);
    }

    #[test]
    fn undo_and_redo_ratings_and_refuse_after_an_outside_change() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let image = dir.path().join("photos/img0.jpg");
        press(&mut h, Key::Num3, Modifiers::SHIFT);
        press(&mut h, Key::X, Modifiers::SHIFT);
        assert_eq!(h.state().history.entries().len(), 2, "un pas par note");
        // u : la dernière note (rejet) défaite, puis Ctrl+Z : les étoiles.
        press(&mut h, Key::U, Modifiers::NONE);
        assert_eq!(tri::xmp::read(&image).unwrap().flag, Flag::None);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 3);
        assert!(h.state().notice.as_ref().unwrap().0.starts_with("Annulé"));
        press(&mut h, Key::Z, Modifiers::CTRL);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 0);
        assert_eq!(
            h.state().folder.notes.notes(&image).stars,
            0,
            "affichage relu"
        );
        // Rétablir : Ctrl+R puis Ctrl+Maj+Z.
        press(&mut h, Key::R, Modifiers::CTRL);
        press(&mut h, Key::Z, Modifiers::CTRL | Modifiers::SHIFT);
        let notes = tri::xmp::read(&image).unwrap();
        assert_eq!((notes.stars, notes.flag), (3, Flag::Rejected));
        // Modifié ailleurs : refus, rien ne bouge.
        let mut changed = notes.clone();
        changed.stars = 5;
        tri::xmp::write_sidecar(&dir.path().join("photos/img0.jpg.xmp"), &changed).unwrap();
        press(&mut h, Key::U, Modifiers::NONE);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 5);
        assert!(
            h.state()
                .file_error
                .as_ref()
                .unwrap()
                .contains("modifiées ailleurs"),
            "{:?}",
            h.state().file_error
        );
        assert_eq!(h.state().history.position(), 2);
    }

    #[test]
    fn undo_a_rename_and_a_move_and_walk_the_history_list() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        // Renommer img0 en quai.
        press(&mut h, Key::F2, Modifiers::NONE);
        h.event(UiEvent::Text("quai".into()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(photos.join("quai.jpg").exists());
        // Déplacer l'image courante vers un dossier.
        let dest = dir.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        press(&mut h, Key::X, Modifiers::NONE);
        h.event(UiEvent::Text(dest.display().to_string()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.file_dialog.is_none());
        assert_eq!(h.state().history.entries().len(), 2);
        let moved = std::fs::read_dir(&dest).unwrap().count();
        assert_eq!(moved, 1);
        // Ctrl+H : la liste ; « État au lancement » défait tout.
        press(&mut h, Key::H, Modifiers::CTRL);
        assert!(h.state().history_open);
        h.get_by_label("État au lancement").click();
        h.run_steps(3);
        assert_eq!(h.state().history.position(), 0);
        assert!(photos.join("img0.jpg").exists() && !photos.join("quai.jpg").exists());
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "revenue");
        assert_eq!(h.state().files.len(), 3, "dossier relu");
        // Le pas le plus récent : tout refait.
        let label = h.state().history.entries()[1].label.clone();
        h.get_by_label(&label).click();
        h.run_steps(3);
        assert_eq!(h.state().history.position(), 2);
        assert!(photos.join("quai.jpg").exists() || dest.join("quai.jpg").exists());
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().history_open);
    }

    #[test]
    fn rating_keys_write_the_sidecar_of_the_selected_image() {
        let (mut h, dir) = harness();
        let photos = dir.path().join("photos");
        let image = photos.join("img0.jpg");
        press(&mut h, Key::Num3, Modifiers::SHIFT);
        press(&mut h, Key::X, Modifiers::SHIFT);
        press(&mut h, Key::Num8, Modifiers::NONE);
        h.event(UiEvent::Text("°".into()));
        h.run_steps(3);
        let on_disk = tri::xmp::read(&image).unwrap();
        assert_eq!(on_disk.stars, 3);
        assert_eq!(on_disk.flag, Flag::Rejected);
        assert_eq!(
            on_disk.colors,
            [tri::xmp::Color::Purple],
            "une seule couleur par défaut : le violet remplace le vert"
        );
        assert!(photos.join("img0.jpg.xmp").exists(), "convention darktable");
        assert_eq!(h.state().folder.notes.notes(&image), &on_disk);
        assert!(h.state().note_error.is_none());
        // L'image suivante n'est pas touchée.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::Num1, Modifiers::NONE);
        assert_eq!(tri::xmp::read(&photos.join("img1.jpg")).unwrap().stars, 1);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 3);
    }

    fn names(h: &Harness<'_, TriApp>) -> Vec<String> {
        h.state().files.iter().map(|f| file_name(&f.path)).collect()
    }

    #[test]
    fn f_prefixed_keys_filter_on_notes() {
        let (mut h, _dir) = harness();
        press(&mut h, Key::Num3, Modifiers::NONE); // img0 : 3 étoiles
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::X, Modifiers::SHIFT); // img1 : rejetée
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::Num9, Modifiers::NONE); // img2 : bleue

        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::Num2, Modifiers::SHIFT);
        assert_eq!(names(&h), ["img0.jpg"], "f puis Maj+2 : au moins 2 étoiles");
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::Num0, Modifiers::NONE);
        assert_eq!(names(&h).len(), 3, "f 0 : étoiles indifférentes");

        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::U, Modifiers::NONE);
        assert_eq!(
            names(&h),
            ["img0.jpg", "img2.jpg"],
            "f u : sans les rejetées"
        );
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::U, Modifiers::NONE);
        assert_eq!(names(&h).len(), 3, "f u à nouveau : filtre retiré");

        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::Num9, Modifiers::SHIFT);
        assert_eq!(names(&h), ["img2.jpg"], "f 9 : bleues");
        // « ° » sur un AZERTY : touche physique Maj + Minus, puis le caractère (kittest les
        // envoie dans deux images, comme cela arrive en vrai).
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::Minus, Modifiers::SHIFT);
        h.event(UiEvent::Text("°".into()));
        h.run_steps(3);
        assert_eq!(h.state().filter_summary(), "bleu ou violet");
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(names(&h).len(), 3, "f Échap : tout effacé");
        assert_eq!(h.state().filter_summary(), "tout");
        assert_eq!(
            h.state().mode,
            Mode::Grid,
            "Échap de la séquence ne fait rien d'autre"
        );
    }

    #[test]
    fn rejecting_under_hide_rejected_moves_on_to_the_next_image() {
        let (mut h, _dir) = harness();
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::U, Modifiers::NONE);
        press(&mut h, Key::X, Modifiers::SHIFT);
        assert_eq!(names(&h), ["img1.jpg", "img2.jpg"]);
        assert_eq!(
            h.state().selected,
            0,
            "img1, désormais première, est sélectionnée"
        );
        press(&mut h, Key::X, Modifiers::SHIFT);
        assert_eq!(names(&h), ["img2.jpg"]);
    }

    fn selected_name(h: &Harness<'_, TriApp>) -> String {
        file_name(&h.state().files[h.state().selected].path)
    }

    #[test]
    fn auto_advance_skips_the_pair_and_is_off_by_default() {
        let names = ["img0.jpg", "img0.png", "img1.jpg", "img2.jpg"];
        let (mut h, dir) = harness_files(&names, 64, 48);
        h.run_steps(3);
        press(&mut h, Key::Num3, Modifiers::NONE);
        assert_eq!(selected_name(&h), "img0.jpg", "désactivée par défaut");
        press(&mut h, Key::A, Modifiers::SHIFT);
        assert!(h.state().auto_advance);
        press(&mut h, Key::Num2, Modifiers::NONE);
        // La paire img0.jpg + img0.png est notée d'un coup : on passe à img1.jpg.
        assert_eq!(selected_name(&h), "img1.jpg");
        let photos = dir.path().join("photos");
        assert_eq!(tri::xmp::read(&photos.join("img0.png")).unwrap().stars, 2);
        press(&mut h, Key::Num8, Modifiers::NONE);
        assert_eq!(
            selected_name(&h),
            "img2.jpg",
            "une couleur fait avancer aussi"
        );
        press(&mut h, Key::Num1, Modifiers::NONE);
        assert_eq!(selected_name(&h), "img2.jpg", "dernière image : on y reste");
        assert_eq!(tri::xmp::read(&photos.join("img2.jpg")).unwrap().stars, 1);
    }

    #[test]
    fn auto_advance_does_not_skip_an_image_when_the_rated_one_leaves_the_filter() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::A, Modifiers::SHIFT);
        // Filtre « sans les rejetées », puis rejet de img0 : elle disparaît, img1 prend sa
        // place sous la sélection ; avancer encore sauterait img1.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::U, Modifiers::NONE);
        assert_eq!(h.state().files.len(), 3);
        press(&mut h, Key::X, Modifiers::SHIFT);
        assert_eq!(h.state().files.len(), 2);
        assert_eq!(selected_name(&h), "img1.jpg");
        // Image toujours dans le filtre : on avance normalement.
        press(&mut h, Key::Num4, Modifiers::NONE);
        assert_eq!(selected_name(&h), "img2.jpg");
    }

    #[test]
    fn the_rating_bar_rates_with_the_mouse() {
        let (mut h, dir) = harness();
        let image = dir.path().join("photos").join("img0.jpg");
        h.get_by_label("3 étoiles").click();
        h.run_steps(3);
        h.get_by_label("Rejeter").click();
        h.run_steps(3);
        h.get_by_label("Couleur verte (ajouter / retirer)").click();
        h.run_steps(3);
        let on_disk = tri::xmp::read(&image).unwrap();
        assert_eq!(on_disk.stars, 3);
        assert_eq!(on_disk.flag, Flag::Rejected);
        assert_eq!(on_disk.colors, [Color::Green]);
        // Recliquer la note posée ne l'efface pas (un double clic involontaire effaçait
        // une note) : des contrôles à part le font, comme les touches 0 et Maj+U.
        h.get_by_label("3 étoiles").click();
        h.run_steps(3);
        h.get_by_label("Rejeter").click();
        h.run_steps(3);
        let on_disk = tri::xmp::read(&image).unwrap();
        assert_eq!(
            (on_disk.stars, on_disk.flag),
            (3, Flag::Rejected),
            "inchangé"
        );
        h.get_by_label("Retirer les étoiles").click();
        h.run_steps(3);
        h.get_by_label("Ni acceptée ni rejetée").click();
        h.run_steps(3);
        let on_disk = tri::xmp::read(&image).unwrap();
        assert_eq!((on_disk.stars, on_disk.flag), (0, Flag::None));
        press(&mut h, Key::N, Modifiers::SHIFT);
        assert!(
            h.query_by_label("3 étoiles").is_none(),
            "Maj+N masque le bandeau"
        );
    }

    #[test]
    fn space_marks_and_ratings_apply_to_the_marked_images() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Space, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::Space, Modifiers::NONE);
        press(&mut h, Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(selected_name(&h), "img1.jpg");
        assert!(h.query_by_label("2 sélectionnées").is_some());
        press(&mut h, Key::Num3, Modifiers::NONE);
        let photos = dir.path().join("photos");
        let stars = |n: &str| tri::xmp::read(&photos.join(n)).unwrap().stars;
        assert_eq!(
            (stars("img0.jpg"), stars("img1.jpg"), stars("img2.jpg")),
            (3, 0, 3),
            "les sélectionnées, pas l'image courante"
        );
        // Couleur : même effet sur toutes (ajoutée, puis retirée), pas une bascule chacune.
        press(&mut h, Key::Num8, Modifiers::NONE);
        let green = |n: &str| tri::xmp::read(&photos.join(n)).unwrap().colors;
        assert_eq!(green("img0.jpg"), [Color::Green]);
        assert_eq!(green("img2.jpg"), [Color::Green]);
        press(&mut h, Key::Num8, Modifiers::NONE);
        assert!(green("img0.jpg").is_empty() && green("img2.jpg").is_empty());
        // Échap désélectionne (on reste dans la grille).
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().selection.is_empty());
        press(&mut h, Key::Num1, Modifiers::NONE);
        assert_eq!(stars("img1.jpg"), 1, "de nouveau l'image courante");
    }

    #[test]
    fn visual_mode_selects_a_range_and_delete_takes_it() {
        let names = ["a.jpg", "b.jpg", "c.jpg", "d.jpg"];
        let (mut h, dir) = harness_files(&names, 64, 48);
        h.run_steps(3);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::V, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert!(h.query_by_label("3 sélectionnées · mode visuel").is_some());
        // Échap annule la plage ; on recommence et on valide par v.
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().selection.is_empty());
        press(&mut h, Key::V, Modifiers::NONE);
        press(&mut h, Key::ArrowLeft, Modifiers::NONE);
        press(&mut h, Key::V, Modifiers::NONE);
        assert!(!h.state().selection.is_visual());
        assert_eq!(h.state().selection.len(), 2, "c et d");
        press(&mut h, Key::Delete, Modifiers::NONE);
        assert!(
            h.query_by_label("Supprimer définitivement 2 fichiers ?")
                .is_some()
        );
        press(&mut h, Key::Enter, Modifiers::NONE);
        let photos = dir.path().join("photos");
        assert!(photos.join("a.jpg").exists() && photos.join("b.jpg").exists());
        assert!(!photos.join("c.jpg").exists() && !photos.join("d.jpg").exists());
        assert!(h.state().selection.is_empty(), "images supprimées oubliées");
    }

    #[test]
    fn ctrl_a_selects_everything_and_ctrl_shift_a_nothing() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::A, Modifiers::CTRL);
        assert_eq!(h.state().selection.len(), 3);
        press(&mut h, Key::A, Modifiers::CTRL | Modifiers::SHIFT);
        assert!(h.state().selection.is_empty());
    }

    #[test]
    fn shift_click_adds_a_range_and_command_click_toggles() {
        let names = ["a.jpg", "b.jpg", "c.jpg", "d.jpg"];
        let (mut h, _dir) = harness_files(&names, 64, 48);
        h.run_steps(5);
        let center = |h: &Harness<'_, TriApp>, i: usize| {
            h.state()
                .cell_rects
                .iter()
                .find(|(j, _)| *j == i)
                .unwrap()
                .1
                .center()
        };
        let click = |h: &mut Harness<'_, TriApp>, i: usize, modifiers: Modifiers| {
            let pos = center(h, i);
            h.event(UiEvent::ModifiersChanged(modifiers));
            h.event(UiEvent::PointerMoved(pos));
            for pressed in [true, false] {
                h.event(UiEvent::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                });
            }
            h.run_steps(3);
            h.event(UiEvent::ModifiersChanged(Modifiers::NONE));
        };
        click(&mut h, 2, Modifiers::SHIFT);
        assert_eq!(h.state().selection.len(), 3, "a à c");
        assert_eq!(selected_name(&h), "c.jpg");
        click(&mut h, 1, Modifiers::COMMAND);
        assert_eq!(h.state().selection.len(), 2, "b retirée");
        click(&mut h, 3, Modifiers::NONE);
        assert!(
            h.state().selection.is_empty(),
            "clic simple : sélection effacée"
        );
    }

    #[test]
    fn compare_needs_two_images_then_rates_and_eliminates_the_active_one() {
        let names = ["a.jpg", "b.jpg", "c.jpg", "d.jpg"];
        let (mut h, dir) = harness_files(&names, 64, 48);
        h.run_steps(3);
        press(&mut h, Key::C, Modifiers::NONE);
        assert_eq!(
            h.state().mode,
            Mode::Grid,
            "une seule image : pas de comparaison"
        );
        assert!(h.query_by_label_contains("au moins 2 images").is_some());
        // a, b, c sélectionnées (mode visuel), puis c.
        press(&mut h, Key::V, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::V, Modifiers::NONE);
        press(&mut h, Key::C, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::Compare);
        assert_eq!(h.state().compare.len(), 3);
        assert_eq!(selected_name(&h), "c.jpg", "l'image courante reste active");
        // Les flèches changent l'image active, sans sortir de la comparaison.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(
            selected_name(&h),
            "c.jpg",
            "dernière image comparée : on y reste"
        );
        press(&mut h, Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(selected_name(&h), "b.jpg");
        // La note va à l'image active seule (pas à toute la sélection).
        press(&mut h, Key::Num4, Modifiers::NONE);
        let photos = dir.path().join("photos");
        let stars = |n: &str| tri::xmp::read(&photos.join(n)).unwrap().stars;
        assert_eq!((stars("a.jpg"), stars("b.jpg"), stars("c.jpg")), (0, 4, 0));
        // Espace élimine l'image active : elle quitte la comparaison et la sélection.
        press(&mut h, Key::Space, Modifiers::NONE);
        assert_eq!(h.state().compare.len(), 2);
        assert!(!h.state().selection.contains(&photos.join("b.jpg")));
        assert_eq!(selected_name(&h), "c.jpg");
        // Échap : retour à la grille, la sélection restante gardée.
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::Grid);
        assert_eq!(h.state().selection.len(), 2);
        // Entrée depuis la comparaison : plein écran sur l'image active.
        press(&mut h, Key::C, Modifiers::NONE);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
    }

    #[test]
    fn peaking_shows_at_one_to_one_only_and_rotates_colours() {
        let (mut h, _dir) = harness_with(200, 150);
        h.run_steps(3);
        press(&mut h, Key::P, Modifiers::NONE);
        assert!(h.state().peaking);
        press(&mut h, Key::Enter, Modifiers::NONE);
        h.run_steps(5);
        assert!(
            h.state().peaking_asked.is_none() && h.state().peaking_texture.is_none(),
            "ajusté : la carte attend le 1:1"
        );
        press(&mut h, Key::Z, Modifiers::NONE);
        run_until(&mut h, |s| s.peaking_texture.is_some());
        let ((path, level), texture) = h.state().peaking_texture.clone().unwrap();
        assert!(path.ends_with("img0.jpg"));
        assert_eq!(level, Level::Full, "le 1:1 d'une image plate");
        assert_eq!(texture.size(), [200, 150], "pixel pour pixel");
        // Retour à l'ajusté : plus de demande en cours.
        press(&mut h, Key::Z, Modifiers::NONE);
        assert!(h.state().peaking_asked.is_none());
        press(&mut h, Key::Z, Modifiers::NONE);
        // Ctrl+P : couleur suivante, en boucle sur les couleurs configurées.
        let n = h.state().gui.sharpness.colors.len();
        for i in 1..=n {
            press(&mut h, Key::P, Modifiers::CTRL);
            assert_eq!(h.state().peaking_color, i % n);
        }
        // Masquée, Ctrl+P la remontre.
        press(&mut h, Key::P, Modifiers::NONE);
        assert!(!h.state().peaking);
        press(&mut h, Key::P, Modifiers::CTRL);
        assert!(h.state().peaking);
    }

    /// Damier de cases de `square` pixels, flouté `blur` fois (moyenne 3 × 3).
    fn checkerboard(square: u32, blur: usize) -> image::RgbImage {
        let mut img = image::RgbImage::from_fn(240, 160, |x, y| {
            if (x / square + y / square).is_multiple_of(2) {
                image::Rgb([220, 220, 220])
            } else {
                image::Rgb([30, 30, 30])
            }
        });
        for _ in 0..blur {
            img = image::imageops::blur(&img, 2.0);
        }
        img
    }

    #[test]
    fn blurry_photos_are_flagged_and_can_be_hidden() {
        let images = vec![
            ("a.png", checkerboard(4, 0)),
            ("b.png", checkerboard(4, 0)),
            ("c.png", checkerboard(4, 3)),
            ("d.png", checkerboard(4, 0)),
        ];
        let (mut h, dir) = harness_images(images);
        run_until(&mut h, |s| s.scores.len() == 4);
        let c = dir.path().join("photos").join("c.png");
        assert!(h.state().is_blurry(&c), "scores : {:?}", h.state().scores);
        assert!(
            !h.state()
                .is_blurry(&dir.path().join("photos").join("a.png"))
        );
        // f b : la floue disparaît ; f b encore : elle revient.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(
            h.state().blur_rule,
            Some(tri::sharpness::BlurRule::HideBlurry)
        );
        assert_eq!(h.state().files.len(), 3);
        assert!(h.state().files.iter().all(|f| f.path != c));
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().files.len(), 4);
        // f Maj+B : seulement la floue.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::SHIFT);
        assert_eq!(h.state().files.len(), 1);
        assert_eq!(h.state().files[0].path, c);
        // f b depuis « floues seulement » : bascule vers « sans les floues ».
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().files.len(), 3);
    }

    #[test]
    fn m_toggles_the_focus_point() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert!(h.state().focus_point, "affiché par défaut");
        press(&mut h, Key::M, Modifiers::NONE);
        assert!(!h.state().focus_point);
        press(&mut h, Key::M, Modifiers::NONE);
        assert!(h.state().focus_point);
    }

    #[test]
    fn dialogs_can_be_moved_by_dragging_their_background() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F2, Modifiers::NONE);
        let title = "Renommer « img0.jpg »";
        let before = h.get_by_label(title).rect().center();
        let after = before + vec2(-120.0, 90.0);
        h.event(UiEvent::PointerMoved(before));
        h.event(UiEvent::PointerButton {
            pos: before,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(1);
        for step in 1..=6 {
            h.event(UiEvent::PointerMoved(
                before + (after - before) * step as f32 / 6.0,
            ));
            h.run_steps(1);
        }
        h.event(UiEvent::PointerButton {
            pos: after,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(3);
        let moved = h.get_by_label(title).rect().center();
        assert!((moved - after).length() < 2.0, "{before:?} → {moved:?}");
        assert!(h.state().file_dialog.is_some(), "toujours ouverte");
    }

    #[test]
    fn the_display_state_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = AppSettings::default();
        settings.cache.dir = Some(dir.path().join("cache"));
        settings.data.dir = Some(dir.path().join("data"));
        settings.places.watch_ms = 0;
        let mut first = TriApp::new(None, settings.clone(), None);
        assert!(
            first.focus_point,
            "valeur de la configuration au premier lancement"
        );
        first.focus_point = false;
        first.histogram = !first.histogram;
        first.save_display();
        let histogram = first.histogram;
        let second = TriApp::new(None, settings.clone(), None);
        assert!(!second.focus_point, "collimateur masqué gardé");
        assert_eq!(second.histogram, histogram);
        // Désactivé : toujours la configuration.
        settings.gui.remember_display = false;
        let third = TriApp::new(None, settings, None);
        assert!(third.focus_point);
    }

    #[test]
    fn the_rating_bar_arrows_change_image_and_the_icon_toggles_auto_advance() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        h.get_by_label("Image suivante").click();
        h.run_steps(3);
        assert_eq!(selected_name(&h), "img1.jpg");
        h.get_by_label("Image précédente").click();
        h.run_steps(3);
        assert_eq!(selected_name(&h), "img0.jpg");
        assert!(!h.state().auto_advance);
        h.get_by_label("Avance automatique après une note : oui / non")
            .click();
        h.run_steps(3);
        assert!(h.state().auto_advance);
    }

    #[test]
    fn z_from_the_grid_opens_at_one_to_one_and_the_bar_has_a_one_to_one_button() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::Z, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
        assert!(h.state().zoom.is_some(), "directement à 1:1");
        h.get_by_label("Zoom 100 % / ajusté").click();
        h.run_steps(3);
        assert!(h.state().zoom.is_none(), "le bouton revient à l'ajusté");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::Grid);
    }

    #[test]
    fn rename_with_f2_typing_and_enter() {
        let names = ["img0.jpg", "img0.png", "img1.jpg"];
        let (mut h, dir) = harness_files(&names, 64, 48);
        h.run_steps(3);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(selected_name(&h), "img0.png");
        press(&mut h, Key::F2, Modifiers::NONE);
        assert!(h.state().file_dialog.is_some());
        assert!(h.ctx.egui_wants_keyboard_input(), "le champ a le focus");
        // Le nom est sélectionné : on tape le nouveau directement ; « q » ne quitte pas.
        h.event(UiEvent::Text("quai".into()));
        h.run_steps(2);
        assert!(h.query_by_label("img0.png devient quai.png").is_some());
        assert!(
            h.query_by_label("img0.jpg devient quai.jpg").is_some(),
            "la paire suit"
        );
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        assert!(!h.state().quit_requested);
        let photos = dir.path().join("photos");
        assert!(photos.join("quai.png").exists() && photos.join("quai.jpg").exists());
        assert!(!photos.join("img0.png").exists());
        assert_eq!(
            selected_name(&h),
            "quai.png",
            "l'image renommée reste sélectionnée"
        );
    }

    #[test]
    fn rename_refuses_a_taken_name_and_escape_cancels() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F2, Modifiers::NONE);
        h.event(UiEvent::Text("img1".into()));
        h.run_steps(2);
        assert!(h.query_by_label_contains("existe déjà").is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(dir.path().join("photos/img0.jpg").exists(), "rien renommé");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        assert_eq!(h.state().mode, Mode::Grid, "Échap ne ferme que la boîte");
    }

    #[test]
    fn move_with_x_typing_tab_and_enter_then_copy_from_recents() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        let dest = dir.path().join("Sélection");
        std::fs::create_dir(&dest).unwrap();
        // x : déplacer l'image courante ; chemin tapé puis complété par Tab.
        press(&mut h, Key::X, Modifiers::NONE);
        assert!(h.state().file_dialog.is_some());
        assert!(h.ctx.egui_wants_keyboard_input(), "le champ a le focus");
        let sep = std::path::MAIN_SEPARATOR;
        h.event(UiEvent::Text(format!("{}{sep}Sél", dir.path().display())));
        h.run_steps(2);
        press(&mut h, Key::Tab, Modifiers::NONE);
        assert!(h.query_by_label_contains("fichier (paires").is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.file_dialog.is_none());
        assert!(dest.join("img0.jpg").exists() && !photos.join("img0.jpg").exists());
        assert_eq!(h.state().files.len(), 2, "dossier relu");
        assert!(h.state().file_error.is_none());
        // y : copier ; la destination récente revient par la flèche du haut.
        press(&mut h, Key::Y, Modifiers::NONE);
        press(&mut h, Key::ArrowUp, Modifiers::NONE);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.file_dialog.is_none());
        assert!(dest.join("img1.jpg").exists() && photos.join("img1.jpg").exists());
        assert_eq!(h.state().files.len(), 2, "une copie ne retire rien");
    }

    #[test]
    fn delete_asks_then_removes_and_selects_the_next_image() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        // Échap annule.
        press(&mut h, Key::Delete, Modifiers::NONE);
        assert!(h.query_by_label("Supprimer définitivement").is_some());
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        assert!(photos.join("img0.jpg").exists());
        // Backspace (touche « effacer » des Mac), puis Entrée.
        press(&mut h, Key::Backspace, Modifiers::NONE);
        assert!(h.state().file_dialog.is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(!photos.join("img0.jpg").exists());
        assert_eq!(h.state().files.len(), 2);
        assert_eq!(selected_name(&h), "img1.jpg");
        assert!(h.state().file_error.is_none());
    }

    #[test]
    fn the_right_click_dialog_offers_the_file_actions() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        h.state_mut().rating_dialog =
            Some((h.state().files[1].path.clone(), egui::pos2(100.0, 100.0)));
        h.run_steps(2);
        h.get_by_label("Renommer…").click();
        h.run_steps(2);
        assert!(h.state().rating_dialog.is_none());
        match &h.state().file_dialog {
            Some(crate::files::FileDialog::Rename { image, .. }) => {
                assert!(image.ends_with("img1.jpg"), "l'image de la boîte")
            }
            _ => panic!("boîte « Renommer » attendue"),
        }
        assert!(h.query_by_label("Mettre à la corbeille…").is_none());
    }

    #[test]
    fn right_click_rates_the_clicked_thumbnail() {
        let (mut h, dir) = harness();
        h.run_steps(5);
        let rect = h
            .state()
            .cell_rects
            .iter()
            .find(|(i, _)| *i == 2)
            .unwrap()
            .1;
        h.event(UiEvent::PointerMoved(rect.center()));
        h.event(UiEvent::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        h.event(UiEvent::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(5);
        assert_eq!(h.state().selected, 2, "le clic droit sélectionne");
        let at = h.state().rating_dialog.as_ref().unwrap().1;
        assert!(
            rect.contains(at),
            "boîte ouverte au point du clic, pas au centre"
        );
        assert!(
            h.query_by_label("Fermer").is_some(),
            "boîte de notation ouverte"
        );
        // La souris passe sur une autre vignette : derrière la boîte, rien ne bouge.
        let other = h
            .state()
            .cell_rects
            .iter()
            .find(|(i, _)| *i == 0)
            .unwrap()
            .1;
        h.event(UiEvent::PointerMoved(other.center() + vec2(1.0, 1.0)));
        h.event(UiEvent::PointerMoved(other.center()));
        h.run_steps(3);
        assert_eq!(h.state().selected, 2, "sélection figée par la boîte modale");
        // Le bandeau et la boîte proposent tous deux « 4 étoiles » : celui de la boîte est
        // le dernier dessiné.
        h.get_all_by_label("4 étoiles").last().unwrap().click();
        h.run_steps(3);
        assert!(
            h.state().rating_dialog.is_some(),
            "reste ouverte pour d'autres notes"
        );
        press(&mut h, Key::Num8, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 2, "navigation bloquée par la boîte");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().rating_dialog.is_none(), "Échap ferme la boîte");
        assert_eq!(h.state().mode, Mode::Grid);
        let photos = dir.path().join("photos");
        assert_eq!(tri::xmp::read(&photos.join("img2.jpg")).unwrap().stars, 4);
        assert_eq!(
            tri::xmp::read(&photos.join("img2.jpg")).unwrap().colors,
            [Color::Green],
            "le clavier note aussi l'image de la boîte"
        );
        assert_eq!(tri::xmp::read(&photos.join("img0.jpg")).unwrap().stars, 0);
    }

    #[test]
    fn the_folder_panel_opens_another_folder() {
        let (mut h, dir) = harness();
        let other = dir.path().join("autre");
        std::fs::create_dir(&other).unwrap();
        for i in 0..2 {
            image::RgbImage::from_pixel(64, 48, image::Rgb([200, 50, 50]))
                .save(other.join(format!("x{i}.jpg")))
                .unwrap();
        }
        // Arborescence limitée au dossier temporaire (pas celle de la machine).
        h.state_mut().tree = Tree::with_places(
            vec![tri::places::Place {
                name: "Essai".into(),
                path: dir.path().to_path_buf(),
                kind: tri::places::PlaceKind::Volume,
            }],
            false,
        );
        h.run_steps(2);
        assert!(h.query_by_label("autre").is_none(), "replié");
        h.get_by_label("Déplier").click();
        h.run_steps(2);
        h.get_by_label("autre").click();
        h.run_steps(3);
        assert_eq!(h.state().folder.dir, other);
        assert_eq!(names(&h), ["x0.jpg", "x1.jpg"]);
        assert_eq!(h.state().selected, 0);
        // La notation suit le nouveau dossier.
        press(&mut h, Key::Num4, Modifiers::NONE);
        assert_eq!(tri::xmp::read(&other.join("x0.jpg")).unwrap().stars, 4);
        // Volume : icône « éjecter », confirmation, annulation (pas de vraie éjection ici).
        h.get_by_label("Éjecter").click();
        h.run_steps(2);
        assert!(h.state().eject_dialog.is_some());
        assert!(h.query_by_label("Annuler").is_some());
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(
            h.state().selected,
            0,
            "grille figée derrière la confirmation"
        );
        h.get_by_label("Annuler").click();
        h.run_steps(2);
        assert!(h.state().eject_dialog.is_none());
        assert!(h.state().ejecting.is_none(), "rien d'éjecté");
        // F5 relit les dossiers : un dossier créé entre-temps apparaît.
        std::fs::create_dir(dir.path().join("nouveau")).unwrap();
        h.run_steps(2);
        assert!(h.query_by_label("nouveau").is_none());
        // F5 relit aussi les emplacements de la machine : on garde ceux du test.
        let place = h.state().tree.places().to_vec();
        press(&mut h, Key::F5, Modifiers::NONE);
        h.state_mut().tree.set_places(place);
        h.run_steps(2);
        assert!(h.query_by_label("nouveau").is_some(), "F5 : dossier relu");
        // Maj+D masque le panneau.
        press(&mut h, Key::D, Modifiers::SHIFT);
        assert!(h.query_by_label("Dossiers").is_none());
    }

    #[test]
    fn without_a_folder_the_app_invites_to_pick_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = AppSettings::default();
        settings.cache.dir = Some(dir.path().join("cache"));
        settings.data.dir = Some(dir.path().join("data"));
        // Textes en français (les tests cherchent les libellés), quelle que soit la langue
        // du système.
        settings.i18n.language = tri::i18n::LanguageSetting::Fr;
        crate::i18n::init(settings.i18n.language);
        // Jamais la corbeille de l'utilisateur : suppression définitive, dans le dossier
        // temporaire.
        settings.files.trash = false;
        let mut h = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(None, settings, None));
        h.run_steps(3);
        assert!(h.state().folder.is_none());
        assert!(h.query_by_label_contains("Choisissez un dossier").is_some());
        assert!(h.query_by_label("Dossiers").is_some());
    }

    #[test]
    fn the_last_folder_is_reopened_without_an_argument() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("séance");
        std::fs::create_dir(&photos).unwrap();
        image::RgbImage::new(8, 8)
            .save(photos.join("a.jpg"))
            .unwrap();
        let settings = |reopen: bool| {
            let mut s = AppSettings::default();
            s.cache.dir = Some(dir.path().join("cache"));
            s.data.dir = Some(dir.path().join("data"));
            s.places.watch_ms = 0;
            s.gui.reopen_last = reopen;
            s
        };
        // Premier lancement, sur le dossier : il est mémorisé.
        let s = settings(true);
        let p = photos.clone();
        let _first = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(Some(p), s, None));
        // Sans argument : rouvert.
        let s = settings(true);
        let again = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(None, s, None));
        assert_eq!(again.state().folder.dir, photos);
        // Réglage désactivé : aucun dossier.
        let s = settings(false);
        let off = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(None, s, None));
        assert!(off.state().folder.is_none());
    }

    #[test]
    fn the_filter_bar_shows_and_hides_with_shift_f() {
        let (mut h, _dir) = harness();
        assert!(
            h.query_by_label("Effacer").is_some(),
            "barre visible par défaut"
        );
        press(&mut h, Key::F, Modifiers::SHIFT);
        assert!(h.query_by_label("Effacer").is_none());
        press(&mut h, Key::F, Modifiers::SHIFT);
        h.get_by_label("RAW").click();
        h.run_steps(3);
        assert_eq!(
            h.state().filter,
            KindFilter::RawOnly,
            "clic : même action que le clavier"
        );
        h.get_by_label("Plates").click();
        h.run_steps(3);
        assert_eq!(h.state().filter, KindFilter::FlatOnly);
        h.get_by_label("Effacer").click();
        h.run_steps(3);
        assert_eq!(h.state().filter, KindFilter::All);
    }

    #[test]
    fn a_malformed_sidecar_is_reported_and_left_alone() {
        let (mut h, dir) = harness();
        let sidecar = dir.path().join("photos").join("img0.jpg.xmp");
        std::fs::write(&sidecar, "<pas du xml").unwrap();
        h.state_mut().folder.notes.reload();
        press(&mut h, Key::Num2, Modifiers::NONE);
        assert!(h.state().note_error.is_some());
        assert_eq!(std::fs::read_to_string(&sidecar).unwrap(), "<pas du xml");
    }

    #[test]
    fn help_opens_and_closes_with_its_keys() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert!(!h.state().help);
        // « ? » ouvre ; une fois ouverte, la recherche a le focus et « ? » s'y tape :
        // F1 et Échap ferment.
        for (key, modifiers) in [
            (Key::F1, Modifiers::NONE),
            (Key::Questionmark, Modifiers::NONE),
            (Key::Questionmark, Modifiers::SHIFT),
        ] {
            press(&mut h, key, modifiers);
            assert!(h.state().help, "{key:?} {modifiers:?} doit ouvrir l'aide");
            press(&mut h, Key::F1, Modifiers::NONE);
            assert!(!h.state().help, "F1 doit refermer l'aide");
        }
    }

    #[test]
    fn help_search_filters_and_typing_is_not_a_shortcut() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.query_by_label("Ligne du dessous").is_some());
        // Rangée par thème sans recherche.
        assert!(h.query_by_label("Navigation").is_some());
        assert!(h.query_by_label("Notation").is_some());
        for c in "wqx".chars() {
            h.event(UiEvent::Text(c.to_string()));
            h.run_steps(1);
        }
        h.run_steps(2);
        assert!(!h.state().quit_requested, "« q » tapé dans la recherche");
        assert!(h.state().help);
        assert_eq!(h.state().help_search.as_deref(), Some("wqx"));
        // « wqx » ne correspond à rien.
        assert!(h.query_by_label("Aucune action ne correspond.").is_some());
        h.state_mut().help_search = Some("pnd".into());
        h.run_steps(2);
        assert!(
            h.query_by_label("Afficher / masquer le panneau des dossiers")
                .is_some()
        );
        assert!(h.query_by_label("Ligne du dessous").is_none());
        // Refermer puis rouvrir : recherche vide.
        press(&mut h, Key::Escape, Modifiers::NONE);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert_eq!(h.state().help_search.as_deref(), Some(""));
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
        assert!(h.query_by_label("Ligne du dessous").is_some());
        assert!(h.query_by_label("Flèche droite / l").is_some());
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
    fn histogram_zones_and_badges() {
        let (mut h, dir) = harness();
        // Une image entièrement blanche : saturée, donc badge et zones.
        let photos = dir.path().join("photos");
        image::RgbImage::from_pixel(64, 48, image::Rgb([255, 255, 255]))
            .save(photos.join("img9.jpg"))
            .unwrap();
        h.state_mut().open_folder(&photos);
        run_until(&mut h, |app| {
            (0..app.files.len()).all(|i| app.ready(i, Level::Exposure))
        });
        h.run_steps(3);
        let white = h
            .state()
            .files
            .iter()
            .position(|f| f.path.ends_with("img9.jpg"))
            .unwrap();
        let white_path = photos.join("img9.jpg");
        let (over, _) = h.state().clip_summary[&white_path];
        assert!(over > 0.99, "image blanche : {over}");
        assert!(over > h.state().gui.histogram.badge_over, "badge affiché");
        // Plein écran sur l'image blanche : histogramme et zones.
        h.state_mut().select(white);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.exposures.contains(&white_path));
        assert!(h.state().histogram, "histogramme affiché par défaut");
        press(&mut h, Key::I, Modifiers::NONE);
        assert!(!h.state().histogram, "i le masque");
        press(&mut h, Key::I, Modifiers::SHIFT);
        assert_eq!(
            h.state().histogram_scale,
            HistogramScale::Linear,
            "Maj+I : linéaire"
        );
        assert!(!h.state().clipping);
        press(&mut h, Key::O, Modifiers::NONE);
        assert!(h.state().clipping, "o : zones");
        h.run_steps(2);
        assert!(
            h.state().mask_textures.contains(&white_path),
            "masque en texture"
        );
        // Maj+C : grand histogramme ouvert sur le canal rouge, image d'analyse ; puis
        // vert, bleu, tous ; Échap ferme.
        press(&mut h, Key::C, Modifiers::SHIFT);
        assert!(
            h.state().histogram_large,
            "Maj+C ouvre le grand histogramme"
        );
        assert_eq!(h.state().histogram_channel, Some(0));
        run_until(&mut h, |app| app.analysis_texture.is_some());
        assert_eq!(
            h.state().analysis_texture.as_ref().unwrap().0,
            (white_path.clone(), Some(0))
        );
        for expected in [Some(1), Some(2), None] {
            press(&mut h, Key::C, Modifiers::SHIFT);
            assert_eq!(h.state().histogram_channel, expected);
        }
        assert!(h.state().histogram_large, "reste ouvert");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().histogram_large);
    }

    #[test]
    fn dialogs_show_a_hand_over_checkboxes_and_grab_elsewhere() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        h.state_mut().config_path = Some(dir.path().join("config.toml"));
        press(&mut h, Key::Comma, Modifiers::CTRL);
        // Une case à cocher (réglage booléen) : pas un bouton d'egui.
        h.event(UiEvent::Text("auto_advance".into()));
        h.run_steps(2);
        let rect = h.get_by_role(egui::accesskit::Role::CheckBox).rect();
        // On a tapé : pointeur masqué jusqu'à ce que la souris bouge.
        h.hover_at(rect.center() + vec2(1.0, 0.0));
        h.run_steps(1);
        h.hover_at(rect.center());
        h.run_steps(2);
        assert_eq!(
            h.output().platform_output.cursor_icon,
            egui::CursorIcon::PointingHand,
            "main sur la case"
        );
        // Le titre de la boîte : on peut la déplacer.
        let title = h.get_by_label("Réglages").rect();
        h.hover_at(title.center());
        h.run_steps(2);
        assert_eq!(
            h.output().platform_output.cursor_icon,
            egui::CursorIcon::Grab
        );
    }

    #[test]
    fn the_settings_window_searches_and_applies_live() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        h.state_mut().config_path = Some(dir.path().join("config.toml"));
        press(&mut h, Key::Comma, Modifiers::CTRL);
        assert!(
            h.state().settings_window.is_some(),
            "Ctrl+, ouvre les réglages"
        );
        assert!(h.ctx.egui_wants_keyboard_input(), "la recherche a le focus");
        h.event(UiEvent::Text("clip_under".into()));
        h.run_steps(2);
        assert!(
            h.query_by_label("clip_under").is_some(),
            "trouvé par la recherche"
        );
        assert!(
            h.query_by_label("thumbnail").is_none(),
            "le reste est filtré"
        );
        // Changement appliqué à chaud : raccourcis et réglages de l'interface.
        let mut gui = h.state().gui.clone();
        gui.auto_advance = true;
        let mut files = h.state().settings.files.clone();
        files.recent = 3;
        let settings = AppSettings {
            gui,
            files,
            keys: [("core.app.quit".to_string(), vec!["Ctrl+q".to_string()])].into(),
            ..AppSettings::default()
        };
        let ctx = h.ctx.clone();
        h.state_mut().apply_settings(&ctx, settings);
        assert!(h.state().gui.auto_advance);
        assert_eq!(h.state().settings.files.recent, 3);
        assert_eq!(h.state().keymap.keys_for(Action::Quit), ["Ctrl+q"]);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().settings_window.is_none(), "Échap ferme");
        assert_eq!(h.state().mode, Mode::Grid);
    }

    #[test]
    fn the_metadata_panel_shows_the_selected_image() {
        let (mut h, _dir) = harness();
        // Lecture en fond : la ligne « Dimensions » finit par apparaître.
        let deadline = Instant::now() + Duration::from_secs(20);
        while h.query_by_label("Dimensions").is_none() {
            assert!(Instant::now() < deadline, "métadonnées jamais affichées");
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            h.query_by_label_contains("64 × 48").is_some(),
            "dimensions de img0"
        );
        press(&mut h, Key::M, Modifiers::SHIFT);
        assert!(
            h.query_by_label("Dimensions").is_none(),
            "Maj+M masque le panneau"
        );
    }

    #[test]
    fn a_card_is_offloaded_from_the_folder_panel() {
        let (mut h, dir) = harness();
        // Fausse carte : deux JPEG dans DCIM.
        let card = dir.path().join("CARTE");
        std::fs::create_dir_all(card.join("DCIM/100_TEST")).unwrap();
        for i in 0..2 {
            image::RgbImage::from_pixel(32, 24, image::Rgb([90, 90, 90]))
                .save(card.join(format!("DCIM/100_TEST/IMG{i}.jpg")))
                .unwrap();
        }
        h.state_mut().tree = Tree::with_places(
            vec![tri::places::Place {
                name: "CARTE".into(),
                path: card.clone(),
                kind: tri::places::PlaceKind::Volume,
            }],
            false,
        );
        h.run_steps(2);
        h.get_by_label("Décharger la carte…").click();
        let deadline = Instant::now() + Duration::from_secs(20);
        let wait_for = |h: &mut Harness<'_, TriApp>, label: &str| {
            while h.query_by_label(label).is_none() {
                assert!(Instant::now() < deadline, "jamais vu : {label}");
                h.run_steps(1);
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        wait_for(&mut h, "Décharger");
        assert!(h.query_by_label_contains("2 à copier").is_some());
        // « Ouvrir le dossier à la fin » est coché par défaut ; « RAW seulement » non.
        h.get_by_label("Décharger").click();
        wait_for(&mut h, "Fermer");
        h.run_steps(3);
        assert!(
            h.query_by_label_contains("2 fichiers copiés et vérifiés")
                .is_some()
        );
        let opened = h.state().folder.dir.clone();
        assert!(
            opened.starts_with(dir.path().join("photos-local")),
            "{opened:?}"
        );
        assert_eq!(
            names(&h),
            ["IMG0.jpg", "IMG1.jpg"],
            "séance ouverte d'elle-même, à plat"
        );
        assert!(h.state().offload.is_some(), "le bilan reste affiché");
        assert!(h.state().ejecting.is_none(), "éjection non cochée : rien");
        h.get_by_label("Fermer").click();
        h.run_steps(2);
        assert!(h.state().offload.is_none());
        let remembered = std::fs::read_to_string(dir.path().join("data/offload-root")).unwrap();
        assert!(
            remembered.ends_with("photos-local"),
            "racine mémorisée : {remembered}"
        );
        assert!(
            opened
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("20"),
            "nom de séance sans dossier d'année : {opened:?}"
        );
        assert_eq!(opened.parent().unwrap(), dir.path().join("photos-local"));
        let history = std::fs::read_to_string(dir.path().join("history.tsv")).unwrap();
        assert_eq!(history.lines().count(), 2, "historique écrit");
    }

    #[test]
    fn status_bar_icons_toggle_the_panels() {
        let (mut h, _dir) = harness();
        h.run_steps(2);
        let click = |h: &mut Harness<'_, TriApp>, what: &str| {
            h.get_by_label_contains(what).click();
            h.run_steps(2);
        };
        assert!(h.state().folder_panel && h.state().metadata_panel);
        click(&mut h, "panneau des dossiers");
        assert!(!h.state().folder_panel);
        click(&mut h, "panneau des métadonnées");
        assert!(!h.state().metadata_panel);
        click(&mut h, "barre de filtres");
        assert!(!h.state().filter_bar);
        click(&mut h, "bandeau de notation");
        assert!(!h.state().rating_bar);
        click(&mut h, "panneau des dossiers");
        assert!(h.state().folder_panel, "et retour");
    }

    #[test]
    fn fixed_volumes_offer_neither_offload_nor_eject() {
        let (mut h, dir) = harness();
        let (work, card) = (dir.path().join("Work"), dir.path().join("CARTE"));
        std::fs::create_dir(&work).unwrap();
        std::fs::create_dir(&card).unwrap();
        let volume = |path: &std::path::Path| tri::places::Place {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: path.to_path_buf(),
            kind: tri::places::PlaceKind::Volume,
        };
        let settings = tri::places::PlaceSettings {
            fixed: vec!["Work".into()],
            ..tri::places::PlaceSettings::default()
        };
        let mut tree = Tree::new(settings, false);
        tree.set_places(vec![volume(&work), volume(&card)]);
        h.state_mut().tree = tree;
        h.run_steps(2);
        // Une seule paire d'icônes : celle de la carte ; Work, fixé, n'en a aucune.
        assert_eq!(h.query_all_by_label("Décharger la carte…").count(), 1);
        assert_eq!(h.query_all_by_label("Éjecter").count(), 1);
    }

    #[test]
    fn the_same_card_twice_counts_duplicates_in_the_chosen_folder() {
        let (mut h, dir) = harness();
        let card = dir.path().join("CARTE");
        std::fs::create_dir_all(card.join("DCIM")).unwrap();
        for i in 0..2 {
            image::RgbImage::new(16, 16)
                .save(card.join(format!("DCIM/IMG{i}.jpg")))
                .unwrap();
        }
        let place = tri::places::Place {
            name: "CARTE".into(),
            path: card.clone(),
            kind: tri::places::PlaceKind::Volume,
        };
        let wait_for = |h: &mut Harness<'_, TriApp>, text: &str| {
            let deadline = Instant::now() + Duration::from_secs(20);
            while h.query_by_label_contains(text).is_none() {
                assert!(Instant::now() < deadline, "jamais vu : {text}");
                h.run_steps(1);
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        for round in 0..2 {
            h.state_mut().tree = Tree::with_places(vec![place.clone()], false);
            h.run_steps(2);
            h.get_by_label("Décharger la carte…").click();
            if round == 0 {
                wait_for(&mut h, "2 à copier");
                h.get_by_label("Décharger").click();
                wait_for(&mut h, "Fermer");
                h.get_by_label("Fermer").click();
                h.run_steps(2);
            } else {
                // Même dossier de séance proposé : tout y est déjà.
                wait_for(&mut h, "2 déjà dans la destination ; 0 à copier");
                h.get_by_label("Annuler").click();
                h.run_steps(2);
            }
        }
    }

    #[test]
    fn an_offload_in_the_background_offers_the_session_without_switching() {
        let (mut h, dir) = harness();
        let card = dir.path().join("CARTE");
        std::fs::create_dir_all(card.join("DCIM")).unwrap();
        image::RgbImage::new(16, 16)
            .save(card.join("DCIM/IMG0.jpg"))
            .unwrap();
        h.state_mut().tree = Tree::with_places(
            vec![tri::places::Place {
                name: "CARTE".into(),
                path: card,
                kind: tri::places::PlaceKind::Volume,
            }],
            false,
        );
        h.run_steps(2);
        let before = h.state().folder.dir.clone();
        h.get_by_label("Décharger la carte…").click();
        let deadline = Instant::now() + Duration::from_secs(20);
        while h.query_by_label("Décharger").is_none() {
            assert!(Instant::now() < deadline);
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
        h.get_by_label("Décharger").click();
        h.run_steps(1); // le clic démarre la copie
        assert!(h.state().offload.as_ref().unwrap().is_running());
        h.state_mut().offload.as_mut().unwrap().hide();
        // La copie se termine en fond : plus de boîte, séance proposée, dossier inchangé.
        while h.state().offload.is_some() {
            assert!(Instant::now() < deadline, "jamais terminé");
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
        h.run_steps(2);
        assert_eq!(
            h.state().folder.dir,
            before,
            "pas de changement de dossier d'office"
        );
        assert!(h.state().offloaded.is_some());
        assert!(
            h.query_by_label_contains("1 copiés").is_some(),
            "bilan dans la barre d'état"
        );
        h.get_by_label_contains("Ouvrir la séance").click();
        h.run_steps(3);
        assert_ne!(h.state().folder.dir, before, "séance ouverte à la demande");
        assert_eq!(names(&h), ["IMG0.jpg"]);
        assert!(h.state().offloaded.is_none());
    }

    #[test]
    fn the_large_histogram_opens_and_closes() {
        let (mut h, _dir) = harness();
        press(&mut h, Key::E, Modifiers::SHIFT);
        assert!(h.state().histogram_large, "Maj+E l'ouvre");
        assert!(h.query_by_label("Échelle linéaire").is_some());
        press(&mut h, Key::I, Modifiers::SHIFT);
        assert_eq!(
            h.state().histogram_scale,
            HistogramScale::Linear,
            "Maj+I dans la boîte"
        );
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 0, "la grille ne bouge pas derrière");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().histogram_large, "Échap la ferme");
        assert_eq!(h.state().mode, Mode::Grid);
        press(&mut h, Key::E, Modifiers::SHIFT);
        press(&mut h, Key::E, Modifiers::SHIFT);
        assert!(!h.state().histogram_large, "la même touche la ferme");
    }

    #[test]
    fn typing_in_a_text_field_triggers_no_shortcut() {
        let (mut h, dir) = harness();
        let card = dir.path().join("CARTE");
        std::fs::create_dir_all(card.join("DCIM")).unwrap();
        image::RgbImage::new(8, 8)
            .save(card.join("DCIM/A.jpg"))
            .unwrap();
        h.state_mut().tree = Tree::with_places(
            vec![tri::places::Place {
                name: "CARTE".into(),
                path: card,
                kind: tri::places::PlaceKind::Volume,
            }],
            false,
        );
        h.run_steps(2);
        h.get_by_label("Décharger la carte…").click();
        let deadline = Instant::now() + Duration::from_secs(20);
        while h.query_by_label("Décharger").is_none() {
            assert!(Instant::now() < deadline);
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
        // Clic dans le champ du dossier de la séance, puis « q » et « 3 ».
        h.ctx
            .memory_mut(|m| m.request_focus(crate::offload::folder_field_id()));
        h.run_steps(2);
        assert!(h.ctx.egui_wants_keyboard_input(), "le champ a le focus");
        press(&mut h, Key::Q, Modifiers::NONE);
        press(&mut h, Key::Num3, Modifiers::NONE);
        assert!(!h.state().quit_requested, "« q » n'a pas quitté");
        let photos = dir.path().join("photos");
        assert_eq!(
            tri::xmp::read(&photos.join("img0.jpg")).unwrap().stars,
            0,
            "« 3 » n'a pas noté"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        h.run_steps(2);
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
        // Depuis la grille, z ouvre le plein écran directement à 100 %.
        press(&mut h, Key::Z, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
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

    fn cursor(h: &Harness<'_, TriApp>) -> egui::CursorIcon {
        h.output().platform_output.cursor_icon
    }

    #[test]
    fn hovering_frames_a_thumbnail_and_clicking_selects_it() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        assert_eq!(h.state().selected, 0);
        let rect = |h: &Harness<'_, TriApp>, n: usize| {
            h.state()
                .cell_rects
                .iter()
                .find(|(i, _)| *i == n)
                .unwrap()
                .1
        };
        // La souris bouge sur la 3e vignette : encadrée, pas sélectionnée.
        // Deux positions : au tout premier événement, egui n'a pas de position précédente
        // et le déplacement vaut zéro.
        let third = rect(&h, 2);
        h.hover_at(third.center() - vec2(3.0, 3.0));
        h.run_steps(1);
        h.hover_at(third.center());
        h.run_steps(2);
        assert_eq!(h.state().selected, 0, "le survol ne sélectionne pas");
        assert_eq!(h.state().hovered, Some(2), "il encadre");
        assert_ne!(cursor(&h), egui::CursorIcon::None);
        // Le bandeau de notation note la sélection, quel que soit le survol en chemin.
        h.get_by_label("2 étoiles").click();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        assert_eq!(tri::xmp::read(&photos.join("img0.jpg")).unwrap().stars, 2);
        assert_eq!(tri::xmp::read(&photos.join("img2.jpg")).unwrap().stars, 0);
        // Le clic sélectionne.
        h.hover_at(third.center());
        h.event(UiEvent::PointerButton {
            pos: third.center(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        });
        h.event(UiEvent::PointerButton {
            pos: third.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(3);
        assert_eq!(h.state().selected, 2, "le clic sélectionne");
        // Le clavier repart de là, masque le pointeur et le cadre.
        press(&mut h, Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(h.state().selected, 1);
        assert_eq!(cursor(&h), egui::CursorIcon::None, "pointeur masqué");
        assert_eq!(h.state().hovered, None, "cadre masqué avec le pointeur");
        // Bouger la souris le fait réapparaître, sans changer la sélection.
        h.hover_at(third.center() + vec2(5.0, 5.0));
        h.run_steps(2);
        assert_ne!(cursor(&h), egui::CursorIcon::None);
        assert_eq!(h.state().selected, 1);
        assert_eq!(h.state().hovered, Some(2));
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

    #[test]
    fn help_rows_are_grouped_then_ranked_by_search() {
        let keymap = Keymap::default();
        let all = help_rows(&keymap, "");
        assert_eq!(all.len(), Action::ALL.len());
        let ranks: Vec<usize> = all
            .iter()
            .map(|(a, _)| Group::ALL.iter().position(|g| *g == a.group()).unwrap())
            .collect();
        assert!(ranks.is_sorted(), "rangées par thème");
        // Recherche par raccourci et par identifiant aussi.
        let by_id = help_rows(&keymap, "core.nav.next");
        assert_eq!(by_id[0].0, Action::NavNext);
        let stars = help_rows(&keymap, "etoile 3");
        assert_eq!(stars[0].0, Action::Stars(3));
    }
}
