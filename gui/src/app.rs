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
use tri::update::Status as UpdateStatus;
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
    similarity: Avail,
    /// Développé par darktable (à la demande).
    developed: Avail,
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
    /// Surveillance du dossier ouvert (fichiers ajoutés, XMP modifiés ailleurs), avec son
    /// intervalle (ms) : relancée si le dossier ou l'intervalle change.
    watch: Option<(tri::watch::FolderWatch, Receiver<tri::watch::Changes>, u64)>,
    /// Lecture en fond des notes du dossier ouvert (issue n° 15 : sur un disque lent, la
    /// fenêtre restait vide le temps de les lire) ; la grille s'affiche sans attendre.
    loading: Option<tri::folder::Loading>,
    /// État du dossier relevé par cette lecture, pour démarrer sa surveillance.
    watch_snapshot: Option<tri::watch::Snapshot>,
    /// Événements du moteur laissés pour l'image suivante (budget dépassé).
    engine_backlog: bool,
    /// Boîte de déchargement d'une carte (éventuellement en arrière-plan).
    offload: Option<OffloadDialog>,
    /// Séance déchargée en arrière-plan, proposée à l'ouverture dans la barre d'état.
    offloaded: Option<PathBuf>,
    /// Volume dont l'éjection a été refusée : « Réessayer l'éjection ».
    eject_failed: Option<PathBuf>,
    /// Volume dont l'éjection attend confirmation.
    eject_dialog: Option<(PathBuf, egui::Pos2)>,
    /// Recherche d'une mise à jour (sur demande) : en cours, ou son résultat affiché.
    update: Option<UpdateCheck>,
    /// Ce binaire : version distribuée ou build personnel (et son commit).
    build: tri::update::Build,
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
    /// Mode concentration : infos et notes dessinées sur l'image (`Ctrl+I`).
    image_info: bool,
    /// Fond choisi pour la grille et le plein écran (`Ctrl+L`) ; `None` : celui du thème.
    background: Option<crate::settings::Color>,
    /// Infos de l'image en plein écran (numéro, source, taille, attentes), écrites par la
    /// vue et montrées dans la barre d'état (demandé le 22/10/2026 : plus rien sur l'image).
    view_info: String,
    /// Avance automatique après une note (`[gui] auto_advance`, Maj+A).
    auto_advance: bool,
    /// Infos sur les vignettes (nom, format, notes, badges), `Maj+T`.
    grid_info: bool,
    /// Taille des vignettes (curseur du bas, `+` / `-`, `*`).
    grid_cell: f32,
    /// Largeur de la grille à la dernière image (« tout à l'écran »).
    grid_width: f32,
    /// « Tout à l'écran » demandé avant que la grille soit mise en page (au démarrage).
    grid_fit_pending: bool,
    /// Clic sur un badge d'une vignette (versions, écrêtage) : l'image et l'action,
    /// faite après la grille.
    cell_action: Option<(usize, Action)>,
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
    /// Tailles des développés darktable (empreinte du sidecar, taille), voir
    /// [`Self::developed_size`].
    developed_sizes: HashMap<PathBuf, (String, Option<(u32, u32)>)>,
    /// Barres de progression masquées jusqu'à cet instant (après un refiltrage :
    /// `[gui] progress_delay_ms`).
    progress_hidden_until: Option<Instant>,
    /// Jugement de netteté changé hors de la grille (curseur en plein écran) : grille
    /// refiltrée seulement au retour, pour ne pas retirer l'image regardée (retour du
    /// 21/10/2026).
    blur_refilter_pending: bool,
    /// Filtre « bien exposées » (ni brûlées ni bouchées, seuils des badges).
    well_exposed: bool,
    /// Filtres inversés : ce qu'ils retirent (notes, netteté, exposition).
    filter_inverted: bool,
    /// Ordre des images (`f s`, `f r`, liste de la barre des filtres).
    order: tri::sort::Order,
    /// Tri par date de prise de vue : des dates sont arrivées depuis le dernier tri
    /// (empreintes de similarité) ; la grille sera triée à nouveau une fois toutes là.
    sort_pending: bool,
    /// Analyses d'exposition arrivées sous le filtre « bien exposées » : grille à
    /// refiltrer une fois les histogrammes finis.
    exposure_refilter_pending: bool,
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
    peaking_texture: Option<((PathBuf, Level, u16), TextureHandle)>,
    /// Carte demandée au chargeur, pas encore reçue.
    peaking_asked: Option<(PathBuf, Level, u16)>,
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
    /// Recadrage en cours (plein écran, `r`) : une version de l'image.
    crop: Option<crate::crop::Crop>,
    /// Version montrée en plein écran : (image, place dans sa liste).
    version_shown: Option<(PathBuf, usize)>,
    /// Versions lues dans les sidecars (à la demande ; vidé quand elles changent).
    versions: HashMap<PathBuf, tri::versions::Versions>,
    /// Profil d'export choisi la dernière fois (session) : pour un rendu (RAW), et pour un
    /// JPEG (boîtier ou d'origine), retenus à part.
    export_choice: [Option<String>; 2],
    /// Export en cours (darktable, en fond), suivi dans la barre d'état.
    export_job: Option<crate::export::Job>,
    /// Journal des exports terminés (rapport d'export, `Ctrl+Maj+E`), gardé dans le dossier
    /// de données ([`tri::export_log`]).
    export_log: Vec<tri::export_log::Entry>,
    /// Rapport d'export ouvert.
    export_report: bool,
    /// Image (passe d'egui) où il s'est ouvert : le clic du bouton de la barre d'état qui
    /// l'ouvre tombe sur son fond dans la même image, et ne doit pas le refermer.
    export_report_since: u64,
    /// Bilan du dernier export dans la barre d'état : tant qu'il y est, un bouton ouvre le
    /// rapport.
    export_notice: Option<String>,
    /// Échec de la dernière action sur un fichier (barre d'état).
    file_error: Option<String>,
    files: Vec<MediaFile>,
    status: Vec<Status>,
    progress: [Progress; 6],
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
    /// Grille : curseur masqué (Échap sans sélection, clic dans le vide), pour une vue
    /// d'ensemble sans image en surbrillance (retour du 18/10/2026, voir les piles).
    cursor_hidden: bool,
    /// darktable trouvé au démarrage du moteur : la source « développé » est proposée.
    darktable: bool,
    /// Mode concentration (F11) : plein écran du système, panneaux et barres masqués (pas
    /// fermés : ils reviennent tels quels en sortant).
    focus_mode: bool,
    /// Entrée en mode concentration : l'indication pour en sortir s'affiche un moment
    /// (la barre d'état, où vont les messages, est masquée).
    focus_mode_since: Option<Instant>,
    /// Mot-symbole « t★ri » de la barre du haut : lettres (teintées selon le thème) et
    /// étoile (chargés à la première image).
    logo: Option<(egui::TextureHandle, egui::TextureHandle)>,
    /// Boîte de regroupement tout juste ouverte : la case « regrouper » prend le clavier.
    group_dialog_fresh: bool,
    /// Recherche par nom de fichier (filtre de la grille, `/`, Ctrl+F).
    name_query: String,
    /// Champ de recherche à qui donner le clavier à la prochaine image.
    focus_request: Option<egui::Id>,
    /// Champ de recherche qui avait le clavier à l'image précédente : egui le retire dès
    /// qu'Échap arrive, avant les raccourcis ; c'est lui qui dit à qui Échap revient.
    search_focused: Option<egui::Id>,
    /// Recherche de dossiers (panneau des dossiers) : texte, parcours en fond, résultats
    /// (meilleurs d'abord) et celui pointé.
    folder_query: String,
    folder_search: Option<tri::places::DirSearch>,
    folder_results: Vec<(PathBuf, i32)>,
    folder_result: usize,
    /// Superpositions (o) : grilles intégrées et celles de l'utilisateur, relues à chaque
    /// changement (un fichier modifié se voit à la pression suivante) ; celle affichée
    /// (0 : aucune) ; logos chargés (`None` : illisible).
    overlays: Vec<Box<dyn tri::overlay::Overlay>>,
    overlay: usize,
    /// Orientation des lignes des superpositions (Maj+O), gardée d'une image à l'autre.
    overlay_flip: tri::overlay::Flip,
    overlay_textures: HashMap<PathBuf, Option<egui::TextureHandle>>,
    /// Défilement de l'aide (tests).
    #[cfg(test)]
    help_offset: f32,
    /// Pointeur au-dessus d'une vignette pendant le dessin de la grille (clic dans le
    /// vide sinon).
    over_cell: bool,
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
    /// Grand histogramme : photo d'analyse affichée à sa droite (`p`).
    histogram_photo: bool,
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
    /// Empreintes de regroupement lues dans le cache, par image.
    signatures: HashMap<PathBuf, tri::similarity::Signature>,
    /// Piles de la grille : la vue de `files` (couvertures des piles repliées).
    stacks: crate::stacks::Stacks,
    /// Piles à recalculer (dossier, filtre, empreinte, note, seuil changés).
    stacks_dirty: bool,
    /// Images des piles dépliées.
    expanded: HashSet<PathBuf>,
    /// Regroupement en piles actif.
    stacking: bool,
    /// Sévérité du regroupement (-1 souple, grandes piles, à 1 strict ; 0 : le centre),
    /// réglée au curseur de la barre des filtres, retenue par dossier.
    group_strictness: f32,
    /// Centre du curseur des piles : seuil calculé d'après le dossier
    /// (`similarity::suggested_threshold`), au dernier regroupement.
    group_centre: f32,
    /// Écart de temps maximal entre deux images d'une pile (secondes).
    /// Sévérité du jugement de netteté (-1 souple à 1 strict, 0 : les seuils des
    /// réglages), réglée au curseur de la barre des filtres, retenue par dossier.
    blur_strictness: f32,
    /// Case « sous-dossiers » du panneau des dossiers : les images des sous-dossiers
    /// s'affichent aussi (`[gui.folders] subfolders_depth` niveaux), choix gardé.
    subfolders: bool,
    group_gap: f32,
    /// Fenêtre de regroupement ouverte (`Ctrl+G`).
    group_dialog: bool,
    /// Couvertures choisies (la préférée de chaque pile), retenues dans les données.
    chosen: HashSet<PathBuf>,
    /// Curseur de similarité en cours de réglage : piles dépliées, fonds alternés.
    grouping_preview: bool,
    /// Liste de l'historique ouverte (`Ctrl+H`).
    history_open: bool,
    /// Panneau qui a le clavier (leader puis direction, façon tmux).
    focus: crate::actions::Scope,
    /// Panneau des métadonnées : ligne pointée au clavier, et les lignes affichées (pour
    /// copier la valeur).
    meta_cursor: usize,
    meta_rows: Vec<(String, String)>,
    /// L'histogramme est en tête du panneau : premier élément pointable (Entrée l'ouvre
    /// en grand).
    meta_has_histogram: bool,
    /// Début d'une sélection de plusieurs lignes (Maj + flèches, Maj+clic) ; `None` : la
    /// ligne pointée seule.
    meta_anchor: Option<usize>,
    /// Curseur clavier déplacé : à faire défiler jusqu'à lui.
    scroll_to_cursor: bool,
    /// Fenêtre de réglages ouverte.
    settings_window: Option<crate::settings_window::SettingsWindow>,
    /// Fichier de configuration modifié par la fenêtre de réglages.
    config_path: Option<PathBuf>,
    /// Recherche dans l'aide (`None` : aide fermée, le champ prendra le focus à
    /// l'ouverture).
    help_search: Option<String>,
    /// Lignes de l'aide pour la recherche en cours, préparées une fois (issue n° 11 : les
    /// recalculer à chaque image, ~75 ms, faisait saccader l'ouverture).
    help_lines: Option<HelpLines>,
    /// Largeurs des colonnes de l'aide (libellés, touches), mesurées au démarrage, ce qui
    /// prépare aussi les caractères des polices ; refaites quand les raccourcis changent.
    help_widths: Option<(f32, f32)>,
    /// Actions à exécuter à la première image (`TRI_GUI_ACTIONS`, développement).
    startup_actions: std::collections::VecDeque<Step>,
    /// Pause en cours dans le script de démarrage.
    script_wait: Option<Instant>,
    /// Vignettes visibles prêtes mais pas encore chargées (budget de l'image dépassé).
    visible_waiting: usize,
    mode: Mode,
    /// Source préférée en plein écran pour les RAW (JPEG boîtier par défaut).
    source: Source,
    /// Source d'avant le développé (boîtier ou RAW) : celle des images qui n'ont pas de
    /// développement quand `source` est « développé » (retour du 20/10/2026).
    base_source: Source,
    /// Zoom 100 % demandé : point de l'image (coordonnées relatives) au centre de la vue.
    /// Conservé d'une image à l'autre pour comparer la netteté au même endroit.
    zoom: Option<Vec2>,
    /// Échelle du zoom (1,0 = 100 % : un pixel de l'image par pixel de l'écran), entre
    /// l'ajusté et `[gui.zoom] max`.
    zoom_scale: f32,
    /// Échelle de l'ajusté pour l'image affichée (relative au 100 %), pour les paliers.
    fit_scale: f32,
    /// Loupe : touche maintenue depuis…, et loupe affichée.
    loupe_since: Option<Instant>,
    loupe_on: bool,
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
        // Pas dans les tests : la langue est globale au processus, et les tests, en
        // parallèle, attendent tous le français (des réglages relus d'un fichier sans
        // `[i18n]` passaient tout le monde en langue système : anglais sur la CI).
        if !cfg!(test) && new.i18n.language != self.settings.i18n.language {
            crate::i18n::init(new.i18n.language);
        }
        if new.gui.theme != self.gui.theme || new.gui.colors != self.gui.colors {
            new.gui.theme.apply(ctx, &new.gui.colors);
        }
        let (keymap, warnings) = new.keymap();
        self.keymap = keymap;
        // Raccourcis ou langue changés : l'aide se prépare à nouveau.
        self.help_lines = None;
        self.help_widths = None;
        self.key_warnings = warnings;
        self.sequencer = new.gui.sequencer();
        // Textures colorées d'après les réglages : recalculées.
        self.mask_textures.clear();
        self.analysis_texture = None;
        self.gui = new.gui;
        self.settings.i18n = new.i18n;
        self.settings.files = new.files;
        self.settings.xmp = new.xmp;
        self.settings.offload = new.offload;
        // Profils d'export, préréglages de recadrage, darktable : aussi à chaud.
        self.settings.export = new.export;
        self.settings.versions = new.versions;
        self.settings.develop = new.develop;
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
        let subfolders = data.as_deref().is_some_and(tri::folder::subfolders);
        let depth = folder_depth(subfolders, &gui);
        let (folder, error) = match &dir {
            Some(dir) => match Folder::scan(dir, depth, settings.xmp.sidecar) {
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
        trace(|| format!("dossier lu : {} images, notes en fond", folder.files.len()));
        let mut tree = Tree::new(settings.places.clone(), gui.folders.hidden);
        if let Some(data) = &data {
            tree.set_favorites(tri::places::favorites(data));
            tree.set_favorite_names(tri::places::favorite_names(data));
        }
        if !folder.is_none() {
            tree.reveal(&folder.dir);
        }
        trace(|| format!("arborescence : {} emplacements", tree.places().len()));
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
            watch: None,
            loading: None,
            watch_snapshot: None,
            engine_backlog: false,
            offload: None,
            offloaded: None,
            eject_failed: None,
            eject_dialog: None,
            update: None,
            build: tri::update::Build::current(),
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
            image_info: gui.view.image_info,
            background: None,
            view_info: String::new(),
            auto_advance: gui.auto_advance,
            grid_info: gui.grid.info,
            grid_cell: gui.grid.cell,
            grid_width: 0.0,
            grid_fit_pending: false,
            cell_action: None,
            rating_dialog: None,
            file_dialog: None,
            crop: None,
            version_shown: None,
            versions: HashMap::new(),
            export_choice: [None, None],
            export_job: None,
            export_log: data
                .as_deref()
                .map(tri::export_log::read)
                .unwrap_or_default(),
            export_report: false,
            export_report_since: 0,
            export_notice: None,
            file_error: None,
            selection: Default::default(),
            scores: HashMap::new(),
            judgements: HashMap::new(),
            blur_rule: None,
            blurry_applied: HashSet::new(),
            blur_refilter_pending: false,
            well_exposed: false,
            filter_inverted: false,
            order: tri::sort::Order {
                key: gui.sort,
                descending: gui.sort_descending,
            },
            sort_pending: false,
            exposure_refilter_pending: false,
            progress_hidden_until: None,
            developed_sizes: HashMap::new(),
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
            progress: [Progress::default(); 6],
            engine: None,
            error,
            keymap,
            sequencer: gui.sequencer(),
            mouse_active: false,
            hovered: None,
            cursor_hidden: false,
            darktable: false,
            focus_mode: false,
            focus_mode_since: None,
            logo: None,
            group_dialog_fresh: false,
            name_query: String::new(),
            focus_request: None,
            search_focused: None,
            folder_query: String::new(),
            folder_search: None,
            folder_results: Vec::new(),
            folder_result: 0,
            overlays: Vec::new(),
            overlay: 0,
            overlay_flip: tri::overlay::Flip::None,
            overlay_textures: HashMap::new(),
            #[cfg(test)]
            help_offset: 0.0,
            over_cell: false,
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
            histogram_photo: gui.histogram.analysis,
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
            signatures: HashMap::new(),
            stacks: crate::stacks::Stacks::default(),
            stacks_dirty: true,
            expanded: HashSet::new(),
            // Grille non groupée par défaut (retour du 19/10/2026) : `[gui.grid] stacks`.
            stacking: settings.similarity.background && gui.grid.stacks,
            group_strictness: 0.0,
            group_centre: settings.similarity.threshold,
            blur_strictness: 0.0,
            subfolders,
            group_gap: settings.similarity.max_gap_s,
            group_dialog: false,
            chosen: settings
                .engine()
                .data_dir()
                .map(|d| tri::similarity::chosen_covers(&d))
                .unwrap_or_default(),
            grouping_preview: false,
            history_open: false,
            focus: crate::actions::Scope::Images,
            meta_cursor: 0,
            meta_rows: Vec::new(),
            meta_has_histogram: false,
            meta_anchor: None,
            scroll_to_cursor: false,
            help_search: None,
            help_lines: None,
            help_widths: None,
            visible_waiting: 0,
            startup_actions: startup_actions().into(),
            script_wait: None,
            mode: Mode::Grid,
            source: Source::default(),
            base_source: Source::default(),
            zoom: None,
            zoom_scale: 1.0,
            fit_scale: 1.0,
            loupe_since: None,
            loupe_on: false,
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
        app.start_loading();
        // L'ordre retenu, aussi au lancement (il ne l'était qu'en changeant de dossier).
        app.apply_sort();
        app.apply_filter();
        app
    }

    /// Lance la lecture en fond des notes du dossier ouvert (et de l'état de sa
    /// surveillance, si elle n'est pas déjà en route).
    fn start_loading(&mut self) {
        self.loading = None;
        self.watch_snapshot = None;
        if self.folder.is_none() || !self.folder.notes.is_pending() {
            return;
        }
        let watching = self.settings.watch.interval_ms > 0
            && !self
                .watch
                .as_ref()
                .is_some_and(|(w, _, _)| w.dir() == self.folder.dir);
        self.loading = Some(tri::folder::Loading::start(
            &self.folder,
            self.folder_depth(),
            watching,
        ));
    }

    /// Notes lues en fond : reprises (les notes posées entre-temps gardées), filtres et
    /// piles revus.
    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some(loading) = &self.loading else {
            return;
        };
        let Some(loaded) = loading.try_take() else {
            // Progression dans la barre d'état.
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        self.loading = None;
        self.folder.notes.adopt(loaded.notes);
        self.watch_snapshot = loaded.snapshot;
        trace(|| format!("notes lues : {}", self.folder.notes.len()));
        if !self.notes_rule.is_empty() {
            self.refilter(Reveal::Minimal);
        }
        self.stacks_dirty = true;
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
            image_info: Some(self.image_info),
            background: self.background,
            sort: Some(self.order),
            auto_advance: Some(self.auto_advance),
            grid_info: Some(self.grid_info),
            grid_cell: Some(self.grid_cell),
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
            (&mut self.image_info, s.image_info),
            (&mut self.auto_advance, s.auto_advance),
            (&mut self.grid_info, s.grid_info),
        ];
        for (field, saved) in pairs {
            if let Some(value) = saved {
                *field = value;
            }
        }
        self.background = s.background;
        if let Some(order) = s.sort {
            self.order = order;
        }
        if let Some(cell) = s.grid_cell {
            let g = self.gui.grid;
            self.grid_cell = cell.clamp(g.cell_min, g.cell_max.max(g.cell_min));
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
        match Folder::scan(dir, self.folder_depth(), self.settings.xmp.sidecar) {
            Ok(folder) => {
                trace(|| format!("dossier ouvert : {}", folder.dir.display()));
                // Mémorisé pour le prochain lancement (sans conséquence en cas d'échec).
                if let Some(data) = self.settings.data_dir() {
                    let _ = tri::folder::remember(&data, &folder.dir);
                }
                self.group_strictness = self
                    .settings
                    .data_dir()
                    .and_then(|d| tri::similarity::folder_strictness(&d, &folder.dir))
                    .unwrap_or(0.0);
                self.blur_strictness = self
                    .settings
                    .data_dir()
                    .and_then(|d| tri::sharpness::folder_strictness(&d, &folder.dir))
                    .unwrap_or(0.0);
                self.folder = folder;
                self.start_loading();
                self.apply_sort();
                self.error = None;
            }
            Err(e) => {
                self.folder = Folder::default();
                self.error = Some(format!("{} : {e}", dir.display()));
            }
        }
        self.files.clear(); // la sélection ne survit pas au changement de dossier
        self.expanded.clear();
        self.selection.clear();
        self.scores.clear();
        self.signatures.clear();
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
        // Histogramme en haut du panneau, en grille comme en plein écran (retour du
        // 17/10/2026) ; panneau masqué : en plein écran, il est sur l'image.
        let settings = self.gui.metadata;
        let focused = self.focus == crate::actions::Scope::Metadata;
        let accent = Color32::from(self.gui.dialogs.title_color);
        self.meta_has_histogram = false;
        if settings.histogram && self.histogram {
            match self.exposure_of(self.selected) {
                Some(e) => {
                    self.meta_has_histogram = true;
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
                    // Pointé au clavier : Entrée l'ouvre en grand.
                    if focused && self.meta_cursor == 0 {
                        ui.painter().rect_stroke(
                            rect.expand(2.0),
                            4.0,
                            egui::Stroke::new(1.5, accent),
                            egui::StrokeKind::Outside,
                        );
                        if std::mem::take(&mut self.scroll_to_cursor) {
                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        }
                    }
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
                let scroll = focused && std::mem::take(&mut self.scroll_to_cursor);
                // Nom : une ligne comme les autres, en tête (demandé le 22/10/2026 : un
                // titre blanc au-dessus de l'histogramme répétait la barre d'état).
                let mut rows = vec![(t!("panel-name"), file_name(&file.path))];
                rows.extend(m.rows());
                // Netteté : une ligne comme les autres (pointable, copiable).
                if let Some(j) = self.judgements.get(&file.path) {
                    let ratio = |r: Option<f32>| {
                        r.map_or("-".to_string(), |r| tri::i18n::decimal(r as f64, 2))
                    };
                    let score = tri::i18n::decimal(j.score as f64, 1);
                    // Dans une pile, jugée contre elle (et le dossier), pas ses voisines.
                    let mut text = match j.group_ratio {
                        Some(g) => t!(
                            "panel-sharpness-value-stack",
                            score = score,
                            folder = ratio(j.folder_ratio),
                            stack = ratio(Some(g))
                        ),
                        None => t!(
                            "panel-sharpness-value",
                            score = score,
                            folder = ratio(j.folder_ratio),
                            neighbours = ratio(j.neighbour_ratio)
                        ),
                    };
                    if j.blurry {
                        text.push_str(&t!("panel-sharpness-blurry"));
                    }
                    rows.push((t!("panel-sharpness"), text));
                }
                let first = usize::from(self.meta_has_histogram);
                self.meta_cursor = self.meta_cursor.min((rows.len() + first).saturating_sub(1));
                // Lignes sélectionnées (rangs parmi les lignes : l'histogramme passe avant).
                let (lo, hi) = self.meta_range();
                let picked = |i: usize| (lo..=hi).contains(&(i + first));
                let selection_fill = ui.visuals().selection.bg_fill.gamma_multiply(0.5);
                let mut clicked = None;
                egui::Grid::new("metadata")
                    .num_columns(2)
                    .spacing(vec2(10.0, 4.0))
                    .show(ui, |ui| {
                        for (i, (label, value)) in rows.iter().enumerate() {
                            let a = ui.add(
                                egui::Label::new(egui::RichText::new(label.as_str()).weak())
                                    .sense(Sense::click()),
                            );
                            // À la ligne plutôt qu'élargir le panneau (noms d'objectif).
                            let b = ui.add(
                                egui::Label::new(value.as_str())
                                    .wrap()
                                    .sense(Sense::click()),
                            );
                            let rect = a.rect.union(b.rect).expand(2.0);
                            // Sélection (y la copie) ; la ligne pointée au clavier encadrée.
                            if focused && picked(i) && hi > lo {
                                ui.painter().rect_filled(rect, 3.0, selection_fill);
                            }
                            if focused && i + first == self.meta_cursor {
                                ui.painter().rect_stroke(
                                    rect,
                                    3.0,
                                    egui::Stroke::new(1.5, accent),
                                    egui::StrokeKind::Outside,
                                );
                                if scroll {
                                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                                }
                            }
                            if a.clicked() || b.clicked() {
                                clicked = Some((i + first, ui.input(|inp| inp.modifiers.shift)));
                            }
                            ui.end_row();
                        }
                    });
                self.meta_rows = rows;
                // Clic : pointe la ligne (et donne le clavier au panneau) ; Maj+clic : étend.
                if let Some((i, shift)) = clicked {
                    self.focus = crate::actions::Scope::Metadata;
                    if shift {
                        self.meta_anchor.get_or_insert(self.meta_cursor);
                    } else {
                        self.meta_anchor = None;
                    }
                    self.meta_cursor = i;
                }
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

    /// Surveillance du dossier ouvert : démarrée pour chaque dossier ouvert ; images
    /// ajoutées, retirées ou modifiées : dossier relu (la sélection reste) ; sidecars
    /// modifiés par un autre logiciel : seules leurs notes sont relues.
    fn watch_folder(&mut self, ctx: &egui::Context) {
        let interval = self.settings.watch.interval_ms;
        if self.folder.is_none() || interval == 0 {
            self.watch = None;
            return;
        }
        let depth = self.folder_depth();
        let current = self.watch.as_ref().is_some_and(|(w, _, ms)| {
            w.dir() == self.folder.dir && w.depth() == depth && *ms == interval
        });
        if !current {
            // L'état du dossier vient de la lecture en fond : on l'attend.
            if self.loading.is_some() {
                return;
            }
            let (tx, rx) = mpsc::channel();
            let repaint = ctx.clone();
            let on_change = move |changes| {
                let _ = tx.send(changes);
                repaint.request_repaint();
            };
            let interval_d = Duration::from_millis(interval);
            let watch = match self.watch_snapshot.take() {
                Some(known) => tri::watch::FolderWatch::start_from(
                    &self.folder.dir,
                    depth,
                    interval_d,
                    known,
                    on_change,
                ),
                None => tri::watch::FolderWatch::start_depth(
                    &self.folder.dir,
                    depth,
                    interval_d,
                    on_change,
                ),
            };
            self.watch = Some((watch, rx, interval));
        }
        let Some((_, rx, _)) = &self.watch else {
            return;
        };
        let mut images = Vec::new();
        let mut sidecars = Vec::new();
        for changes in rx.try_iter() {
            images.extend(changes.images);
            sidecars.extend(changes.sidecars);
        }
        if !images.is_empty() {
            trace(|| format!("dossier modifié ailleurs : {} images", images.len()));
            // Analyses à reprendre du moteur (gardées sinon : ré-annonces ignorées).
            for image in &images {
                self.signatures.remove(image);
                self.scores.remove(image);
            }
            let selected = self.files.get(self.selected).map(|f| f.path.clone());
            self.reload_folder(selected);
            self.notice = Some((t!("watch-reloaded", count = images.len()), false));
        } else if !sidecars.is_empty() {
            // Développés chargés dont le sidecar a changé (recadrage dans darktable…) :
            // oubliés, l'image revient au rendu de base.
            let developed: Vec<usize> = (0..self.status.len())
                .filter(|&i| self.status[i].developed == Avail::Ready)
                .collect();
            for i in developed {
                self.developed_ready(i);
            }
            // Versions peut-être changées ailleurs : relues à la demande.
            self.versions.clear();
            let changed = self.folder.notes.reload_sidecars(&sidecars);
            trace(|| format!("XMP modifiés ailleurs : {}", changed.len()));
            if !changed.is_empty() {
                // Filtre sur les notes, couverture des piles : à revoir.
                if !self.notes_rule.is_empty() {
                    self.refilter(Reveal::Minimal);
                }
                self.stacks_dirty = true;
                self.notice = Some((t!("watch-notes", count = changed.len()), false));
            }
        }
    }

    /// Panneau des dossiers : l'arborescence du moteur ([`Tree`]), dessinée ligne par
    /// ligne. Le triangle déplie ou replie ; un clic sur le nom ouvre le dossier.
    fn folder_panel(&mut self, ui: &mut Ui) {
        let f = self.gui.folders;
        let mut toggle: Option<usize> = None;
        let mut pointed: Option<usize> = None;
        let mut open = None;
        let mut eject = None;
        let mut offload = None;
        let mut menu: Option<(PathBuf, Action)> = None;
        let mut dropped: Option<(Dragged, PathBuf)> = None;
        drag_preview(ui.ctx());
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
            // Descendre dans les sous-dossiers (demandé le 20/10/2026) : leurs images dans
            // la grille aussi ; choix gardé d'une session à l'autre.
            let mut on = self.subfolders;
            let tip = t!(
                "folders-subfolders-tip",
                n = self.gui.folders.subfolders_depth
            );
            if ui
                .checkbox(&mut on, t!("folders-subfolders"))
                .on_hover_text(tip)
                .changed()
            {
                self.set_subfolders(on);
            }
        });
        // Recherche de dossiers (retour du 19/10/2026) : sous le titre ; pendant la
        // recherche, les résultats remplacent l'arborescence.
        if self.folder_search_ui(ui) {
            return;
        }
        ui.separator();
        // Dossier ouvert montré à sa première ligne (son favori en tête, s'il en est un).
        let mut reveal = std::mem::take(&mut self.reveal_folder);
        let focused = self.focus == crate::actions::Scope::Folders;
        let cursor = self.tree.cursor().filter(|_| focused);
        let scroll = focused && std::mem::take(&mut self.scroll_to_cursor);
        let accent = Color32::from(self.gui.dialogs.title_color);
        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            // Une section par nature d'emplacement, chacune sous son titre (demandé le
            // 20/10/2026) : disques, dossier personnel, favoris, puis la racine.
            let mut section = None;
            for (i, row) in self.tree.rows().into_iter().enumerate() {
                if section != Some(row.section) {
                    if section.is_some() {
                        ui.separator();
                    }
                    section = Some(row.section);
                    let title = match row.section {
                        tri::places::Section::Drives => t!("folders-section-drives"),
                        tri::places::Section::Network => t!("folders-section-network"),
                        tri::places::Section::Personal => t!("folders-section-personal"),
                        tri::places::Section::Favorites => t!("folders-section-favorites"),
                        tri::places::Section::System => t!("folders-section-system"),
                    };
                    ui.label(egui::RichText::new(title).small().strong().weak());
                }
                let line = ui.horizontal(|ui| {
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
                        toggle = Some(i);
                        pointed = Some(i);
                    }
                    let current = row.path == self.folder.dir;
                    let label = ui.selectable_label(current, &row.name);
                    // Glisser-déposer : un dossier se glisse (pas un emplacement) ; tout
                    // dossier reçoit un dossier ou des images de la grille.
                    let label = if row.depth > 0 {
                        let label = label.interact(Sense::drag());
                        label.dnd_set_drag_payload(Dragged::Folder(row.path.clone()));
                        label
                    } else {
                        label
                    };
                    let target = |d: &Dragged| !matches!(d, Dragged::Folder(p) if *p == row.path);
                    if label
                        .dnd_hover_payload::<Dragged>()
                        .is_some_and(|d| target(&d))
                    {
                        ui.painter().rect_stroke(
                            label.rect.expand(2.0),
                            3.0,
                            egui::Stroke::new(2.0, accent),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if let Some(d) = label.dnd_release_payload::<Dragged>()
                        && target(&d)
                    {
                        dropped = Some(((*d).clone(), row.path.clone()));
                    }
                    if label.clicked() {
                        pointed = Some(i);
                        if !current {
                            open = Some(row.path.clone());
                        }
                    }
                    if label.double_clicked() {
                        toggle = Some(i);
                    }
                    // Favori nommé ou non : son chemin au survol (plusieurs « Sources »).
                    let label = if row.section == tri::places::Section::Favorites && row.depth == 0
                    {
                        label.on_hover_text(crate::files::tilde(&row.path))
                    } else {
                        label
                    };
                    // Clic droit : les actions du clavier (retour du 18/10/2026), raccourci au
                    // survol.
                    let removable = row.kind == Some(PlaceKind::Volume) && row.ejectable;
                    label.context_menu(|ui| {
                        let mut entry = |ui: &mut Ui, action: Action, text: String| {
                            if ui
                                .button(text)
                                .on_hover_text(self.action_tip(action))
                                .clicked()
                            {
                                menu = Some((row.path.clone(), action));
                                ui.close();
                            }
                        };
                        entry(ui, Action::FolderCreate, t!("folder-menu-new"));
                        if row.depth > 0 {
                            entry(ui, Action::FolderRename, t!("folder-menu-rename"));
                            entry(ui, Action::FolderMove, t!("folder-menu-move"));
                            entry(ui, Action::FolderCopy, t!("folder-menu-copy"));
                            entry(ui, Action::FolderDelete, t!("folder-menu-delete"));
                        }
                        let favorite = if row.favorite {
                            t!("folder-menu-favorite-remove")
                        } else {
                            t!("folder-menu-favorite-add")
                        };
                        entry(ui, Action::FolderFavorite, favorite);
                        if row.favorite {
                            entry(
                                ui,
                                Action::FolderFavoriteName,
                                t!("folder-menu-favorite-name"),
                            );
                        }
                        ui.separator();
                        if removable {
                            entry(ui, Action::FolderOffload, t!("folder-menu-offload"));
                            entry(ui, Action::FolderEject, t!("folder-menu-eject"));
                        } else {
                            entry(ui, Action::FolderOffload, t!("folder-menu-offload-here"));
                        }
                    });
                    if current && reveal {
                        reveal = false;
                        label.scroll_to_me(Some(egui::Align::Center));
                    }
                    if row.favorite {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(size, size), Sense::hover());
                        let color = ui.visuals().weak_text_color();
                        marks::star(ui.painter(), rect.center(), size * 0.4, color);
                        response.on_hover_text(format!(
                            "{}\n{}",
                            t!("folders-favorite"),
                            self.action_tip(Action::FolderFavorite)
                        ));
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
                // Curseur clavier : cadre de la couleur d'accent autour de la ligne.
                if cursor == Some(i) {
                    ui.painter().rect_stroke(
                        line.response.rect.expand(1.0),
                        3.0,
                        egui::Stroke::new(1.5, accent),
                        egui::StrokeKind::Outside,
                    );
                    if scroll {
                        line.response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            }
        });
        if focused {
            panel_focus_frame(ui, accent, self.gui.dialogs.focus_frame_width);
        }
        // Par la ligne cliquée, pas par son dossier : un favori et le même dossier dans
        // l'arborescence se déplient chacun de leur côté.
        if let Some(i) = toggle {
            self.tree.toggle_row(i);
        }
        // Clic sur un dossier : il devient la ligne pointée et le panneau prend le clavier,
        // comme un clic dans les métadonnées (retour du 22/10/2026 : pas cohérent).
        if let Some(i) = pointed {
            self.tree.set_cursor(i);
            self.focus = crate::actions::Scope::Folders;
        }
        // Menu du clic droit : le dossier visé devient la ligne pointée, puis la même
        // action qu'au clavier.
        if let Some((path, action)) = menu {
            self.tree.point_at(&path);
            let ctx = ui.ctx().clone();
            match action {
                Action::FolderOffload | Action::FolderEject => {
                    self.folder_drive_action(&ctx, action)
                }
                // « Ajouter aux favoris » ouvrait la boîte de suppression (cas par défaut
                // de `folder_edit_action`), corrigé le 20/10/2026.
                Action::FolderFavorite => self.toggle_favorite(),
                Action::FolderFavoriteName => self.name_favorite(&ctx),
                Action::FolderCopy | Action::FolderMove => {
                    self.transfer_folder(&ctx, path, action == Action::FolderCopy, None)
                }
                Action::FolderCreate | Action::FolderRename | Action::FolderDelete => {
                    self.folder_edit_action(&ctx, action)
                }
                _ => {}
            }
        }
        // Glisser-déposer sur un dossier : la boîte « Copier / déplacer vers… », remplie,
        // confirme toujours ; copier si la touche de copie est tenue au lâcher.
        if let Some((dragged, dest)) = dropped {
            let ctx = ui.ctx().clone();
            let copy = drag_copies(&ctx);
            match dragged {
                Dragged::Folder(path) => self.transfer_folder(&ctx, path, copy, Some(&dest)),
                Dragged::Images(paths) => {
                    let mode = if copy {
                        tri::files::TransferMode::Copy
                    } else {
                        tri::files::TransferMode::Move
                    };
                    let at = ctx.content_rect().center() - vec2(210.0, 160.0);
                    self.file_dialog = Some(crate::files::FileDialog::transfer(
                        crate::files::Source::Images(paths),
                        mode,
                        Some(&dest),
                        self.data_dir.as_deref(),
                        at,
                    ));
                }
            }
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
    /// Lance la recherche d'une mise à jour dans un fil à part (requête de quelques
    /// secondes au plus) ; le résultat s'affiche dans une boîte.
    fn check_update(&mut self, ctx: &egui::Context) {
        self.help = false;
        let settings = self.settings.update.clone();
        let build = self.build.clone();
        let repaint = ctx.clone();
        let handle = std::thread::spawn(move || {
            let result = tri::update::check(&build, &settings).map_err(|e| e.to_string());
            repaint.request_repaint();
            result
        });
        self.update = Some(UpdateCheck::Running(handle));
    }

    /// Boîte de la recherche d'une mise à jour : en cours, dernière version, ou version
    /// plus récente avec ses notes et le lien vers sa page.
    fn update_dialog(&mut self, ctx: &egui::Context) {
        if let Some(UpdateCheck::Running(handle)) = &self.update
            && handle.is_finished()
            && let Some(UpdateCheck::Running(handle)) = self.update.take()
        {
            let result = handle.join().unwrap_or_else(|_| Err("?".to_string()));
            self.update = Some(UpdateCheck::Done(result));
        }
        let Some(state) = &self.update else { return };
        let current = self.build.version.as_str();
        let build = &self.build;
        let mut close = false;
        let at = ctx.content_rect().center() - vec2(160.0, 80.0);
        let modal = modal_at("update_dialog", at)
            .title(t!("update-title"))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                match state {
                    UpdateCheck::Running(_) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t!("update-checking"));
                        });
                    }
                    UpdateCheck::Done(Ok(UpdateStatus::Latest)) => {
                        ui.label(t!("update-latest", version = current));
                    }
                    UpdateCheck::Done(Ok(UpdateStatus::Build { behind })) => {
                        let [first, second] = tri::update::build_text(build, *behind);
                        ui.label(egui::RichText::new(first).strong());
                        ui.label(second);
                    }
                    UpdateCheck::Done(Ok(UpdateStatus::Newer(release))) => {
                        ui.label(
                            egui::RichText::new(t!(
                                "update-available",
                                version = release.version.as_str(),
                                current = current
                            ))
                            .strong(),
                        );
                        let notes = release.notes_in(tri::i18n::language());
                        if !notes.is_empty() {
                            ui.add_space(4.0);
                            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                crate::markdown::show(ui, notes);
                            });
                        }
                        ui.add_space(6.0);
                        if ui.button(t!("update-open-page")).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(&release.url));
                            close = true;
                        }
                    }
                    UpdateCheck::Done(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                }
                ui.add_space(6.0);
                close |= ui.button(t!("close")).clicked();
            });
        if matches!(self.update, Some(UpdateCheck::Running(_))) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if close || modal.should_close() {
            self.update = None;
        }
    }

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
        // Type de fichier et recherche par nom (`/`) : la base, jamais inversée.
        let base = tri::fuzzy::NameFilter {
            query: &self.name_query,
        }
        .apply(self.folder.filtered(self.filter, &NotesRule::default()));
        // Filtres sur le contenu : notes, netteté (jugée par le moteur d'après les scores
        // connus), exposition (seuils des badges).
        let groups = self
            .notes_rule
            .filter(&self.folder.notes)
            .apply(base.clone());
        let groups = match self.blur_rule {
            Some(rule) => tri::sharpness::BlurFilter {
                judgements: &self.judgements,
                rule,
            }
            .apply(groups),
            None => groups,
        };
        let groups = if self.well_exposed {
            self.load_clip_summary();
            let h = self.gui.histogram;
            tri::exposure::ClipFilter {
                fractions: &self.clip_summary,
                over: h.badge_over,
                under: h.badge_under,
            }
            .apply(groups)
        } else {
            groups
        };
        // Inversé : ce que les filtres sur le contenu ont retiré de la base.
        let groups = if self.filter_inverted && self.content_filtered() {
            tri::filter::invert(base, &groups)
        } else {
            groups
        };
        self.blurry_applied = self.blurry_now();
        self.files = groups.into_iter().flatten().collect();
        self.status = vec![Status::default(); self.files.len()];
        self.requested.clear();
        self.progress = [Progress::default(); 6];
        let delay = Duration::from_millis(self.gui.progress_delay_ms);
        self.progress_hidden_until = Some(Instant::now() + delay);
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
        self.stacks_dirty = true;
        self.tell_shown();
    }

    /// Curseur de sévérité du flou (demandé le 20/10/2026), à côté des gouttes : au
    /// centre les seuils des réglages, à droite plus strict, à gauche plus souple ; effet
    /// en direct (badges, filtre), retenu pour le dossier au lâcher, double-clic : retour
    /// au centre (repère).
    fn blur_slider(&mut self, ui: &mut Ui) {
        let (slider, t) = strictness_slider(
            ui,
            self.gui.grid.blur_slider,
            self.blur_strictness,
            t!("blur-slider-tip"),
        );
        if t != self.blur_strictness {
            self.set_blur_strictness(t);
        }
        if slider.drag_stopped() || slider.double_clicked_slider(ui) {
            self.remember_strictness();
        }
    }

    /// Curseur de taille des vignettes (barre du bas, en grille) ; double-clic : taille
    /// de la configuration.
    fn grid_size_slider(&mut self, ui: &mut Ui) {
        let g = self.gui.grid;
        ui.spacing_mut().slider_width = g.size_slider;
        let mut cell = self.grid_cell;
        let keys = [Action::ZoomOut, Action::ZoomIn]
            .map(|a| self.keymap.keys_for(a).join(" / "))
            .join(" ; ");
        let slider = ui
            .add(
                egui::Slider::new(&mut cell, g.cell_min..=g.cell_max.max(g.cell_min))
                    .logarithmic(true)
                    .show_value(false),
            )
            .on_hover_text(format!("{} ({keys})", t!("grid-size")));
        if slider.double_clicked_slider(ui) {
            cell = g.cell;
        }
        if cell != self.grid_cell {
            self.grid_cell = cell;
            self.reveal = Some(Reveal::Minimal);
        }
    }

    /// Seuil de regroupement en vigueur : le centre du dossier à la sévérité choisie.
    fn group_threshold(&self) -> f32 {
        let span = self.settings.similarity.strictness_span;
        tri::similarity::with_strictness(self.group_centre, self.group_strictness, span)
    }

    /// Retient la sévérité des piles pour le dossier ouvert (oubliée au centre).
    fn remember_group_strictness(&self) {
        if let Some(data) = self.settings.data_dir()
            && !self.folder.is_none()
        {
            let keep = (self.group_strictness != 0.0).then_some(self.group_strictness);
            let _ = tri::similarity::remember_strictness(&data, &self.folder.dir, keep);
        }
    }

    /// Retient la sévérité du flou pour le dossier ouvert (oubliée au centre).
    fn remember_strictness(&self) {
        if let Some(data) = self.settings.data_dir()
            && !self.folder.is_none()
        {
            let keep = (self.blur_strictness != 0.0).then_some(self.blur_strictness);
            let _ = tri::sharpness::remember_strictness(&data, &self.folder.dir, keep);
        }
    }

    /// Nouvelle sévérité du flou : jugement refait (instantané, les scores sont connus),
    /// et la grille refiltrée si un filtre sur le flou est actif ; hors de la grille,
    /// seulement au retour ([`Self::blur_refilter`]).
    fn set_blur_strictness(&mut self, strictness: f32) {
        self.blur_strictness = strictness;
        self.rejudge();
        self.blur_refilter_pending = true;
        self.blur_refilter();
    }

    /// Refiltre la grille si le jugement de netteté a changé depuis le dernier filtrage,
    /// un filtre sur le flou actif ; en grille seulement (plein écran : l'image regardée
    /// ne disparaît pas sous le pointeur).
    fn blur_refilter(&mut self) {
        if !self.blur_refilter_pending || self.mode != Mode::Grid {
            return;
        }
        self.blur_refilter_pending = false;
        if self.blur_rule.is_some() && self.blurry_now() != self.blurry_applied {
            self.refilter(Reveal::Minimal);
        }
    }

    /// Un filtre sur le contenu est actif (notes, netteté, exposition) : ce que
    /// l'inversion retourne.
    fn content_filtered(&self) -> bool {
        !self.notes_rule.is_empty() || self.blur_rule.is_some() || self.well_exposed
    }

    /// Parts saturées / bouchées de tout le dossier ouvert, lues dans le cache d'analyse
    /// pour celles qui manquent (filtre « bien exposées » : les images masquées comptent
    /// aussi). Une image pas encore analysée est absente, donc gardée par le filtre.
    fn load_clip_summary(&mut self) {
        let Some(engine) = &self.engine else { return };
        for file in &self.folder.files {
            if self.clip_summary.contains_key(&file.path) {
                continue;
            }
            if let Some(e) = preview::exposure(&engine.cache, file, &engine.preview) {
                self.clip_summary
                    .insert(file.path.clone(), (e.over_fraction(), e.under_fraction()));
            }
        }
    }

    /// Trie les images du dossier selon `order` (dates de prise de vue : celles des
    /// empreintes de similarité, déjà lues en fond pour les piles ; inconnues : à la fin).
    fn apply_sort(&mut self) {
        let signatures = &self.signatures;
        tri::sort::sort(&mut self.folder.files, self.order, |p| {
            signatures.get(p).and_then(|s| s.time)
        });
    }

    /// Change l'ordre : la grille suit, la même image reste sélectionnée.
    fn set_order(&mut self, order: tri::sort::Order) {
        self.order = order;
        self.sort_pending = false;
        self.apply_sort();
        self.refilter(Reveal::Minimal);
        self.notice = Some((
            t!(
                "sort-notice",
                key = sort_name(order.key),
                direction = if order.descending {
                    t!("sort-descending")
                } else {
                    t!("sort-ascending")
                }
            ),
            false,
        ));
    }

    /// Tri par date de prise de vue : la grille triée à nouveau une fois les dates
    /// arrivées (toutes, pas à chacune), en grille seulement (comme les filtres sur
    /// l'analyse).
    fn sort_refilter(&mut self) {
        let p = self.progress[5];
        if self.sort_pending && self.mode == Mode::Grid && (p.total == 0 || p.done == p.total) {
            self.sort_pending = false;
            self.apply_sort();
            self.refilter(Reveal::Minimal);
        }
    }

    /// Filtre « bien exposées » : grille refiltrée une fois les analyses d'exposition
    /// arrivées finies (pas à chacune), en grille seulement (comme la netteté).
    fn exposure_refilter(&mut self) {
        let p = self.progress[3];
        if self.exposure_refilter_pending
            && self.mode == Mode::Grid
            && (p.total == 0 || p.done == p.total)
        {
            self.exposure_refilter_pending = false;
            if self.well_exposed {
                self.refilter(Reveal::Minimal);
            }
        }
    }

    /// Juge la netteté de tout le dossier, dans l'ordre du dossier quels que soient les
    /// filtres (les voisines d'une photo sont celles prises juste avant et après) ; dans
    /// les piles d'images semblables quand les empreintes sont connues
    /// (`sharpness::judge_in_groups`, seuil `[sharpness] group_threshold`).
    fn rejudge(&mut self) {
        let files = &self.folder.files;
        let ordered: Vec<(PathBuf, Option<f32>)> = files
            .iter()
            .map(|f| (f.path.clone(), self.scores.get(&f.path).copied()))
            .collect();
        let s = &self.settings;
        let groups = if s.similarity.background && !self.signatures.is_empty() {
            let sigs: Vec<Option<&tri::similarity::Signature>> =
                files.iter().map(|f| self.signatures.get(&f.path)).collect();
            let links = tri::similarity::distances(&sigs, s.similarity.shape_weight);
            tri::similarity::group(&links, s.sharpness.group_threshold, s.similarity.max_gap_s)
        } else {
            Vec::new()
        };
        let sharpness = s.sharpness.with_strictness(self.blur_strictness);
        self.judgements = tri::sharpness::judge_in_groups(&ordered, &groups, &sharpness);
        // Curseur à fond à gauche : aucune photo floue, toutes visibles (retour du
        // 21/10/2026 : même au plus souple, il en manquait, c'était déroutant).
        if self.blur_strictness <= -1.0 {
            for j in self.judgements.values_mut() {
                j.blurry = false;
            }
        }
        self.stacks_dirty = true;
    }

    /// Recalcule les piles (regroupement du moteur sur les empreintes connues), et ce que
    /// la grille en montre ; l'image sélectionnée, si elle est cachée dans une pile
    /// repliée, laisse la place à la couverture.
    fn regroup(&mut self) {
        let s = &self.settings.similarity;
        let signatures: Vec<Option<&tri::similarity::Signature>> = self
            .files
            .iter()
            .map(|f| self.signatures.get(&f.path))
            .collect();
        let links = tri::similarity::distances(&signatures, s.shape_weight);
        self.group_centre = tri::similarity::suggested_threshold(&links, s);
        let ranges = tri::similarity::group(&links, self.group_threshold(), self.group_gap);
        let files = &self.files;
        let notes = &self.folder.notes;
        let scores = &self.scores;
        // Couverture : l'acceptée, sinon la mieux notée, sinon la plus nette, sinon la
        // première (décidé le 18/10/2026).
        let chosen = &self.chosen;
        let cover = |r: &std::ops::Range<usize>| {
            // La préférée choisie par l'utilisateur, sinon la meilleure.
            if let Some(i) = r.clone().find(|&i| chosen.contains(&files[i].path)) {
                return i;
            }
            r.clone()
                .max_by(|&a, &b| {
                    let key = |i: usize| {
                        let n = notes.notes(&files[i].path);
                        (
                            n.flag == Flag::Accepted,
                            n.flag != Flag::Rejected,
                            n.stars,
                            scores.get(&files[i].path).copied().unwrap_or(0.0),
                        )
                    };
                    let (ka, kb) = (key(a), key(b));
                    (ka.0, ka.1, ka.2)
                        .cmp(&(kb.0, kb.1, kb.2))
                        .then(ka.3.total_cmp(&kb.3))
                        // À égalité : la première.
                        .then(b.cmp(&a))
                })
                .unwrap_or(r.start)
        };
        // Réglage du curseur de similarité avec Maj : tout déplié, pour voir les piles se
        // former.
        let preview = self.grouping_preview;
        let expanded = |r: &std::ops::Range<usize>| {
            preview || r.clone().any(|i| self.expanded.contains(&files[i].path))
        };
        self.stacks =
            crate::stacks::Stacks::build(self.files.len(), ranges, self.stacking, cover, expanded);
        // En mode pile (plein écran), on parcourt aussi les images cachées d'une pile.
        let browsing = self.mode == Mode::View && self.stacking;
        if !self.files.is_empty() && !browsing {
            let visible = self.stacks.visible(self.selected);
            if visible != self.selected {
                self.selected = visible;
                self.tell_shown();
            }
        }
        self.stacks_dirty = false;
    }

    /// Fait de l'image sélectionnée la couverture de sa pile (la préférée), ou l'oublie
    /// si elle l'était ; une seule par pile, retenue dans les données de tri.
    fn choose_cover(&mut self) {
        let Some(range) = self.stacks.range_of(self.selected).cloned() else {
            return;
        };
        if range.len() < 2 {
            return;
        }
        let path = self.files[self.selected].path.clone();
        let others: Vec<PathBuf> = range
            .map(|i| self.files[i].path.clone())
            .filter(|p| *p != path)
            .collect();
        let on = !self.chosen.contains(&path);
        for o in &others {
            self.chosen.remove(o);
        }
        if on {
            self.chosen.insert(path.clone());
            self.notice = Some((t!("stack-chosen", name = file_name(&path)), false));
        } else {
            self.chosen.remove(&path);
        }
        if let Some(data) = self.settings.data_dir() {
            let _ = tri::similarity::choose_cover(&data, &path, &others, on);
        }
        self.stacks_dirty = true;
    }

    /// Déplie ou replie la pile de l'image sélectionnée.
    fn toggle_stack(&mut self) {
        let Some(range) = self.stacks.range_of(self.selected).cloned() else {
            return;
        };
        if range.len() < 2 {
            return;
        }
        let paths: Vec<PathBuf> = range.map(|i| self.files[i].path.clone()).collect();
        if paths.iter().any(|p| self.expanded.contains(p)) {
            for p in &paths {
                self.expanded.remove(p);
            }
        } else {
            self.expanded.extend(paths);
        }
        self.stacks_dirty = true;
        self.reveal = Some(Reveal::Minimal);
    }

    /// Déplie toutes les piles, ou les replie toutes si l'une est dépliée.
    fn toggle_all_stacks(&mut self) {
        if self.expanded.is_empty() {
            self.expanded = self.files.iter().map(|f| f.path.clone()).collect();
        } else {
            self.expanded.clear();
        }
        self.stacks_dirty = true;
        self.reveal = Some(Reveal::Minimal);
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
        // darktable, s'il est installé : rendus développés à la demande (configuration
        // propre dans le cache de tri, jamais celle de l'utilisateur).
        let darktable = tri::develop::find_darktable(&self.settings.develop.darktable).map(|cli| {
            tri::develop::Darktable {
                cli,
                config_dir: cache.root().join("darktable"),
                high_quality: self.settings.develop.high_quality,
            }
        });
        trace(|| format!("darktable : {:?}", darktable.as_ref().map(|d| &d.cli)));
        self.darktable = darktable.is_some();
        let producer = CachedPreviews {
            cache: cache.clone(),
            settings: preview,
            darktable,
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
            // Déjà connu : une ré-annonce du moteur (après chaque refiltrage), rien à relire
            // (une lecture du cache, et un accès au fichier source, par image).
            if let Some(file) = self.files.get(index)
                && !self.scores.contains_key(&file.path)
                && let Some(score) = preview::sharpness(&engine.cache, file, &engine.preview)
            {
                self.scores.insert(file.path.clone(), score.value);
                changed = true;
            }
        }
        if changed {
            self.rejudge();
        }
        // Nouveaux scores : grille refiltrée une fois tous connus (pas à chaque score).
        if changed {
            self.blur_refilter_pending = true;
        }
        let p = self.progress[4];
        if p.total == 0 || p.done == p.total {
            self.blur_refilter();
        }
    }

    /// Intègre les événements du moteur reçus depuis la dernière image.
    fn drain_events(&mut self) {
        let mut scored = Vec::new();
        self.drain_engine(&mut scored);
        self.take_scores(scored);
        self.exposure_refilter();
        self.sort_refilter();
    }

    fn drain_engine(&mut self, scored: &mut Vec<usize>) {
        let Some(engine) = &self.engine else { return };
        let generation = engine.generation;
        // Empreinte arrivée : les piles du jugement de netteté changent.
        let mut signed = false;
        // Pas plus que le budget d'une image : sur un dossier déjà calculé, des milliers
        // d'analyses arrivent d'un coup (issue n° 15) ; la suite à l'image suivante.
        let deadline = Instant::now() + self.gui.load_budget();
        self.engine_backlog = false;
        loop {
            if Instant::now() >= deadline {
                self.engine_backlog = true;
                break;
            }
            let Ok(event) = engine.events.try_recv() else {
                break;
            };
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
                    // Seulement une analyse nouvelle (issue n° 10) : refiltrer relance le
                    // moteur sur la liste, qui ré-annonce celles déjà en cache ; les compter
                    // refiltrait sans fin, et chaque fois la grille revenait à la sélection.
                    if level == Level::Exposure
                        && self.well_exposed
                        && let Some(f) = self.files.get(index)
                        && !self.clip_summary.contains_key(&f.path)
                    {
                        self.exposure_refilter_pending = true;
                    }
                    if level == Level::Similarity
                        && let Some(file) = self.files.get(index)
                        && !self.signatures.contains_key(&file.path)
                        && let Some(sig) = preview::signature(&engine.cache, file, &engine.preview)
                    {
                        // Une date nouvelle seulement (comme pour l'exposition, issue n° 10 :
                        // refiltrer fait ré-annoncer celles déjà en cache).
                        let new = self.signatures.insert(file.path.clone(), sig).is_none();
                        if new && self.order.key == tri::sort::SortKey::Captured {
                            self.sort_pending = true;
                        }
                        self.stacks_dirty = true;
                        signed = true;
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
        if signed && !self.scores.is_empty() {
            self.rejudge();
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
        let focus = self.effective_focus();
        // Le grand histogramme prend les touches le temps d'être ouvert ; le panneau actif
        // reste mémorisé pour après.
        if focus != crate::actions::Scope::Histogram {
            self.focus = focus;
        }
        self.keymap.set_focus(focus);
        // Touche ambiguë (`y` avant `y w`) restée sans suite : son action, d'abord.
        let mut actions: Vec<Action> = self.sequencer.expire(now).into_iter().collect();
        for chord in chords {
            actions.extend(self.sequencer.feed(&self.keymap, chord, now));
        }
        if let Some(at) = self.sequencer.deadline() {
            ctx.request_repaint_after(at.saturating_duration_since(now));
        }
        for action in actions {
            // Regroupement : Échap ou Ctrl+G ferme.
            if self.group_dialog {
                if matches!(action, Action::Back | Action::GroupingWindow) {
                    self.group_dialog = false;
                }
                continue;
            }
            // Rapport d'export : Échap ou son raccourci le ferment.
            if self.export_report {
                if matches!(action, Action::Back | Action::ExportReport) {
                    self.export_report = false;
                }
                continue;
            }
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
            // Pendant l'enregistrement d'un raccourci, même pas : Échap peut en être un.
            if let Some(window) = &self.settings_window {
                if matches!(action, Action::Back | Action::Settings) && !window.is_recording() {
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
            // Recherche d'une mise à jour : seule la fermeture.
            if self.update.is_some() {
                if action == Action::Back {
                    self.update = None;
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
            // Recadrage : ses touches, Échap abandonne.
            if self.crop.is_some() {
                match action {
                    Action::Back => self.crop = None,
                    Action::CropMove(_)
                    | Action::CropSize(_)
                    | Action::CropRatio(_)
                    | Action::CropRotate
                    | Action::CropSave
                    | Action::CropPreset
                    | Action::CropPresetPrev
                    // La carte de netteté s'éteint aussi pendant le recadrage.
                    | Action::TogglePeaking => self.perform(ctx, action),
                    _ if action.works_over_overlay() => self.perform(ctx, action),
                    _ => {}
                }
                continue;
            }
            // Grand histogramme : échelle, fermeture (Échap, même touche).
            if self.histogram_large {
                match action {
                    Action::Back | Action::HistogramLarge => self.histogram_large = false,
                    Action::HistogramScale
                    | Action::HistogramChannel
                    | Action::HistogramRed
                    | Action::HistogramGreen
                    | Action::HistogramBlue
                    | Action::HistogramAll
                    | Action::HistogramPhoto => self.perform(ctx, action),
                    // Se déplacer d'image en image sous la boîte (retour du 19/10/2026) :
                    // l'histogramme suit l'image courante.
                    Action::NavPrev
                    | Action::NavNext
                    | Action::NavUp
                    | Action::NavDown
                    | Action::NavFirst
                    | Action::NavLast
                    | Action::NavPageUp
                    | Action::NavPageDown => self.perform(ctx, action),
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
            // Curseur masqué : une action sur l'image courante (noter, supprimer, ouvrir,
            // se déplacer…) le fait seulement réapparaître, sans agir sur une image qu'on
            // ne voit pas.
            if self.cursor_hidden && self.mode == Mode::Grid && uses_cursor(action) {
                self.cursor_hidden = false;
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
            Action::CheckUpdate => self.check_update(ctx),
            Action::ReportIssue => {
                let url = tri::update::issue_url(&self.build, &self.settings.update);
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
            Action::Undo => {
                let result = self.history.undo(self.settings.xmp.sidecar);
                self.after_history(result.map(|a| vec![a]).map_err(Some));
            }
            Action::Redo => {
                let result = self.history.redo(self.settings.xmp.sidecar);
                self.after_history(result.map(|a| vec![a]).map_err(Some));
            }
            Action::History => self.history_open = !self.history_open,
            Action::ExportReport => {
                self.export_report = !self.export_report;
                self.export_report_since = ctx.cumulative_pass_nr();
            }
            Action::FocusLeft | Action::FocusRight | Action::FocusNext => self.move_focus(action),
            Action::FolderUp => self.tree_cursor(|t| t.move_cursor(-1)),
            Action::FolderDown => self.tree_cursor(|t| t.move_cursor(1)),
            Action::FolderExpand => self.tree_cursor(Tree::cursor_right),
            Action::FolderCollapse => self.tree_cursor(Tree::cursor_left),
            Action::FolderFirst => self.tree_cursor(|t| t.cursor_to_end(true)),
            Action::FolderLast => self.tree_cursor(|t| t.cursor_to_end(false)),
            Action::FolderOpen => {
                if let Some(path) = self.tree.cursor_path() {
                    if path != self.folder.dir {
                        self.open_folder(&path);
                    }
                    // Choix du 18/10/2026 : on enchaîne sur le tri.
                    self.focus = crate::actions::Scope::Images;
                }
            }
            Action::MetaUp | Action::MetaSelectUp => {
                if action == Action::MetaSelectUp {
                    self.meta_anchor.get_or_insert(self.meta_cursor);
                } else {
                    self.meta_anchor = None;
                }
                self.meta_cursor = self.meta_cursor.saturating_sub(1);
                self.scroll_to_cursor = true;
            }
            Action::MetaDown | Action::MetaSelectDown => {
                if action == Action::MetaSelectDown {
                    self.meta_anchor.get_or_insert(self.meta_cursor);
                } else {
                    self.meta_anchor = None;
                }
                let first = usize::from(self.meta_has_histogram);
                let last = (self.meta_rows.len() + first).saturating_sub(1);
                self.meta_cursor = (self.meta_cursor + 1).min(last);
                self.scroll_to_cursor = true;
            }
            Action::MetaOpen => {
                if self.meta_has_histogram && self.meta_cursor == 0 {
                    self.histogram_large = true;
                }
            }
            Action::FolderOffload | Action::FolderEject => self.folder_drive_action(ctx, action),
            Action::FolderCreate | Action::FolderRename | Action::FolderDelete => {
                self.folder_edit_action(ctx, action)
            }
            Action::FolderFavorite => self.toggle_favorite(),
            Action::FolderFavoriteName => self.name_favorite(ctx),
            Action::FolderCopy | Action::FolderMove => {
                if let Some(i) = self.tree.cursor() {
                    let path = self.tree.rows()[i].path.clone();
                    self.transfer_folder(ctx, path, action == Action::FolderCopy, None);
                }
            }
            Action::MetaCopy | Action::MetaCopyAll => {
                let first = usize::from(self.meta_has_histogram);
                let (lo, hi) = if action == Action::MetaCopyAll {
                    (0, usize::MAX)
                } else {
                    self.meta_range()
                };
                let rows: Vec<&(String, String)> = self
                    .meta_rows
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| (lo..=hi).contains(&(i + first)))
                    .map(|(_, r)| r)
                    .collect();
                match rows.as_slice() {
                    [] => {}
                    // Une ligne : la valeur seule (à coller dans un champ).
                    [(label, value)] => {
                        ctx.copy_text(value.clone());
                        self.notice = Some((t!("metadata-copied", label = label.clone()), false));
                    }
                    // Plusieurs : « libellé : valeur », une par ligne.
                    many => {
                        let text: Vec<String> = many
                            .iter()
                            .map(|(l, v)| {
                                t!("metadata-copy-line", label = l.clone(), value = v.clone())
                            })
                            .collect();
                        ctx.copy_text(text.join("\n"));
                        self.notice = Some((t!("metadata-copied-lines", n = many.len()), false));
                    }
                }
            }
            // Échap dans un champ de recherche : la recherche d'abord.
            Action::Back if self.search_escape(ctx) => {}
            Action::Back if self.focus != crate::actions::Scope::Images => {
                self.focus = crate::actions::Scope::Images;
            }
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
                } else if self.mode == Mode::Grid {
                    // Plus rien à fermer : le curseur se masque (vue d'ensemble).
                    self.cursor_hidden = true;
                }
            }
            Action::Open => {
                if self.mode == Mode::Grid && !self.files.is_empty() {
                    self.mode = Mode::View;
                } else if self.mode == Mode::View && self.stacking {
                    // Mode pile : Entrée choisit la préférée.
                    self.choose_cover();
                }
            }
            // Un seul interrupteur (retour du 18/10/2026) : piles en grille, et en plein
            // écran ← / → dans la pile, ↑ / ↓ de pile en pile ; sans piles, de photo en photo.
            Action::StackMode => {
                self.stacking = !self.stacking;
                self.stacks_dirty = true;
            }
            Action::StackChoose => self.choose_cover(),
            Action::ToggleSource => {
                // Zoomé, on reste au même endroit : seul le 1:1 demandé change.
                // Le développé n'entre dans la rotation que s'il existe déjà pour l'image
                // (retour du 19/10/2026 : jamais 10 s d'attente pour rien) ; Maj+B le
                // calcule.
                let developed = self.darktable && self.developed_ready(self.selected);
                self.set_source(self.source.next(developed));
                self.request_zoom();
            }
            Action::Compare => self.start_compare(),
            Action::TogglePeaking => self.peaking = !self.peaking,
            Action::SharpnessLenient | Action::SharpnessStrict => {
                let step = self.gui.sharpness.step;
                let delta = if action == Action::SharpnessStrict {
                    step
                } else {
                    -step
                };
                let t = (self.blur_strictness + delta).clamp(-1.0, 1.0);
                // Pas en virgule flottante : 0 et les bouts retombent juste dessus (le
                // centre ; à fond à gauche, toutes les photos visibles).
                let t = if t.abs() < step / 2.0 {
                    0.0
                } else if t.abs() > 1.0 - step / 2.0 {
                    t.signum()
                } else {
                    t
                };
                self.set_blur_strictness(t);
                self.remember_strictness();
                let percent = (t * 100.0).round() as i64;
                self.notice = Some((t!("sharpness-strictness", value = percent), false));
            }
            Action::ToggleFocusPoint => self.focus_point = !self.focus_point,
            Action::OverlayNext => self.cycle_overlay(),
            Action::OverlayOrient => {
                self.overlay_flip = self.overlay_flip.next();
                let rank = self.overlay_flip.rank();
                self.notice = Some((t!("overlay-orientation", n = rank), false));
            }
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
                    self.zoom_scale = 1.0;
                    self.zoom = match self.zoom {
                        Some(_) => None,
                        None => Some(self.zoom_center(ctx)),
                    };
                    self.request_zoom();
                }
            }
            // Rechercher : `/` (ajusté en plein écran) en grille et dans les dossiers.
            Action::Search => self.start_search(),
            Action::Develop => self.develop_current(),
            Action::Crop => self.start_crop(),
            Action::CropMove(dir) | Action::CropSize(dir) => {
                let step = self.gui.crop.step;
                let (dx, dy) = match dir {
                    crate::actions::Dir::Left => (-step, 0.0),
                    crate::actions::Dir::Right => (step, 0.0),
                    crate::actions::Dir::Up => (0.0, -step),
                    crate::actions::Dir::Down => (0.0, step),
                };
                if let Some(crop) = &mut self.crop {
                    if matches!(action, Action::CropMove(_)) {
                        crop.nudge(dx, dy);
                    } else {
                        crop.grow(dx, dy);
                    }
                }
            }
            Action::CropRatio(n) => {
                if let (Some(crop), Some(&ratio)) = (
                    &mut self.crop,
                    tri::versions::Ratio::PRESETS.get(usize::from(n)),
                ) {
                    crop.set_ratio(ratio);
                }
            }
            Action::CropRotate => {
                if let Some(crop) = &mut self.crop {
                    crop.rotate();
                }
            }
            Action::CropSave => self.save_crop(),
            Action::CropPreset => self.next_preset(true),
            Action::CropPresetPrev => self.next_preset(false),
            Action::VersionNext => {
                if self.mode == Mode::View
                    && let Some(path) = self.files.get(self.selected).map(|f| f.path.clone())
                {
                    let n = self.versions_of(&path).list.len();
                    let current = self
                        .version_shown
                        .as_ref()
                        .filter(|(p, _)| *p == path)
                        .map(|(_, i)| *i);
                    let next = match current {
                        None if n > 0 => Some(0),
                        Some(i) if i + 1 < n => Some(i + 1),
                        _ => None,
                    };
                    self.version_shown = next.map(|i| (path, i));
                    self.zoom = None;
                }
            }
            Action::FocusMode => {
                self.focus_mode = !self.focus_mode;
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.focus_mode));
                self.focus_mode_since = self.focus_mode.then(Instant::now);
            }
            Action::ZoomFit
                if self.mode == Mode::Grid || self.focus == crate::actions::Scope::Folders =>
            {
                self.start_search()
            }
            // En grille, comme dans Lightroom : + / - changent la taille des vignettes, *
            // les met toutes à l'écran.
            Action::ZoomIn | Action::ZoomOut if self.mode == Mode::Grid => {
                let g = self.gui.grid;
                self.grid_cell = grid::step_cell(
                    self.grid_cell,
                    action == Action::ZoomIn,
                    g.cell_min,
                    g.cell_max,
                );
                self.reveal = Some(Reveal::Minimal);
            }
            Action::Zoom100 if self.mode == Mode::Grid => self.grid_fit_all(),
            // Paliers (+, -), 100 % (*), ajusté (/) : comme Lightroom.
            Action::ZoomIn | Action::ZoomOut | Action::ZoomFit | Action::Zoom100
                if matches!(self.mode, Mode::View | Mode::Compare) =>
            {
                let z = &self.gui.zoom;
                let current = if self.zoom.is_some() {
                    self.zoom_scale
                } else {
                    self.fit_scale
                };
                let target = match action {
                    Action::ZoomFit => None,
                    Action::Zoom100 => Some(1.0),
                    // Déjà au bout (min, max) : rien ne change.
                    _ => match view::next_step(
                        current,
                        &z.steps,
                        action == Action::ZoomIn,
                        (z.min, z.max),
                    ) {
                        Some(scale) => Some(scale),
                        None => return,
                    },
                };
                match target {
                    // Un palier qui tombe sur l'ajusté : pas de zoom (100 % reste 100 %).
                    Some(scale)
                        if action != Action::Zoom100 && (scale - self.fit_scale).abs() < 1e-3 =>
                    {
                        self.zoom = None
                    }
                    Some(scale) => {
                        self.zoom_scale = scale;
                        if self.zoom.is_none() {
                            self.zoom = Some(self.zoom_center(ctx));
                        }
                    }
                    None => self.zoom = None,
                }
                self.request_zoom();
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
            // Mode pile : ← / → dans la pile (arrêt ou boucle au bout), ↑ / ↓ de pile en pile.
            Action::NavPrev | Action::NavNext if self.mode == Mode::View && self.stacking => {
                let step = if action == Action::NavNext { 1 } else { -1 };
                let wrap = self.gui.view.stack_wrap;
                if let Some(i) = self.stacks.step_in_stack(self.selected, step, wrap) {
                    self.select(i);
                }
            }
            Action::NavUp | Action::NavDown if self.mode == Mode::View && self.stacking => {
                let forward = action == Action::NavDown;
                if let Some(i) = self.stacks.neighbour_group(self.selected, forward) {
                    self.select(i);
                }
            }
            Action::NavUp | Action::NavDown | Action::NavPageUp | Action::NavPageDown
                if self.mode == Mode::View => {}
            Action::Stars(_) | Action::Color(_) | Action::Flag(_) => self.rate_selected(action),
            Action::ToggleAutoAdvance => self.auto_advance = !self.auto_advance,
            Action::FileDeleteStack | Action::FileCopyStack => {
                // Toute la pile de l'image (images cachées dans une pile repliée comprises).
                let range = self.stacks.range_of(self.selected).cloned();
                match range.filter(|_| self.stacking) {
                    Some(range) => {
                        let paths = self.files[range].iter().map(|f| f.path.clone()).collect();
                        let file_action = if action == Action::FileDeleteStack {
                            Action::FileDelete
                        } else {
                            Action::FileCopy
                        };
                        let at = ctx.content_rect().center() - vec2(160.0, 80.0);
                        self.file_action(file_action, paths, at);
                    }
                    None => {
                        let keys = self.keymap.keys_for(Action::StackMode).join(" / ");
                        self.notice = Some((t!("stack-actions-need-stacks", keys = keys), true));
                    }
                }
            }
            Action::FileRename
            | Action::FileDelete
            | Action::FileReveal
            | Action::FileEdit
            | Action::FileCopy
            | Action::FileMove
            | Action::FileExport
            | Action::Send(_) => {
                // Renommer, afficher : l'image courante ; les autres : la sélection.
                let paths = match action {
                    Action::FileDelete
                    | Action::FileEdit
                    | Action::FileCopy
                    | Action::FileMove
                    | Action::FileExport
                    | Action::Send(_) => self.selection.targets(&self.files, self.selected),
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
                self.name_query.clear();
                self.filter = KindFilter::All;
                self.notes_rule = NotesRule::default();
                self.blur_rule = None;
                self.well_exposed = false;
                self.filter_inverted = false;
                self.apply_filter();
            }
            // « Floues seulement » : « nettes » inversé (retour du 21/10/2026).
            Action::FilterBlurry => {
                let rule = tri::sharpness::BlurRule::HideBlurry;
                self.blur_rule = (self.blur_rule != Some(rule)).then_some(rule);
                self.apply_filter();
            }
            Action::FilterWellExposed => {
                self.well_exposed = !self.well_exposed;
                self.apply_filter();
            }
            Action::FilterInvert => {
                self.filter_inverted = !self.filter_inverted;
                self.apply_filter();
            }
            Action::SortNext => self.set_order(tri::sort::Order {
                key: self.order.key.next(),
                ..self.order
            }),
            Action::SortReverse => self.set_order(tri::sort::Order {
                descending: !self.order.descending,
                ..self.order
            }),
            Action::ToggleFilterBar => self.filter_bar = !self.filter_bar,
            Action::ToggleRatingBar => self.rating_bar = !self.rating_bar,
            Action::ToggleImageInfo => self.image_info = !self.image_info,
            Action::BackgroundNext => {
                let theme = self.theme_background(&ctx.global_style().visuals);
                self.background =
                    next_background(self.background, theme, &self.gui.colors.backgrounds);
            }
            Action::ToggleGridInfo => self.grid_info = !self.grid_info,
            Action::RefreshFolders => self.tree.refresh(),
            Action::ToggleHistogram => self.histogram = !self.histogram,
            Action::HistogramScale => self.histogram_scale = self.histogram_scale.toggled(),
            Action::HistogramLarge => self.histogram_large = !self.histogram_large,
            // Le grand histogramme s'ouvre au besoin : c'est là que le canal se voit.
            Action::HistogramRed => self.histogram_channel = Some(0),
            Action::HistogramGreen => self.histogram_channel = Some(1),
            Action::HistogramBlue => self.histogram_channel = Some(2),
            Action::HistogramAll => self.histogram_channel = None,
            Action::HistogramPhoto => self.histogram_photo = !self.histogram_photo,
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
            Action::StackToggle => self.toggle_stack(),
            Action::StacksAll => self.toggle_all_stacks(),
            Action::GroupingWindow => {
                self.group_dialog = !self.group_dialog;
                self.group_dialog_fresh = self.group_dialog;
            }
            // Navigation dans la vue : les images cachées dans une pile repliée sont
            // sautées.
            nav => {
                if self.stacks_dirty {
                    self.regroup();
                }
                let page = self.layout.rows_in(self.view_height);
                let shown = &self.stacks.shown;
                if shown.is_empty() {
                    let new =
                        grid::navigate(self.selected, self.files.len(), &self.layout, page, nav);
                    self.select(new);
                } else {
                    let pos = self.stacks.position(self.selected).unwrap_or(0);
                    let new = grid::navigate(pos, shown.len(), &self.layout, page, nav);
                    self.select(shown[new]);
                }
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
        if self.stacks_dirty {
            self.regroup();
        }
        let pos = self.stacks.position(self.selected);
        let next = match pos {
            Some(pos) => self.stacks.shown[pos + 1..]
                .iter()
                .copied()
                .find(|&i| !siblings.contains(&self.files[i].path)),
            None => (self.selected + 1..self.files.len())
                .find(|&i| !siblings.contains(&self.files[i].path)),
        };
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
        // Couverture des piles : la meilleure, qui a pu changer.
        self.stacks_dirty = true;
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
        self.cursor_hidden = false;
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
            if self.source_for(&self.files[i]) == Source::Raw && kind == MediaKind::Raw {
                wanted.push((rank == 0, i, Level::Standard));
            }
            // Développé : seulement s'il est déjà fait (en cache), jamais calculé d'office
            // (Maj+B le demande pour l'image affichée).
            if self.source == Source::Developed && self.developed_ready(i) {
                wanted.push((rank == 0, i, Level::Developed));
            }
            if self.zoom.is_some() || (self.loupe_on && rank == 0) {
                let level = view::zoom_level(kind, self.source_for(&self.files[i]));
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
            Level::Similarity => &s.similarity,
            Level::Developed => &s.developed,
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
                let key = (loaded.path.clone(), loaded.level, loaded.threshold);
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
        let selected_source = self
            .files
            .get(self.selected)
            .map_or(self.source, |f| self.source_for(f));
        if self.zoom.is_some() && selected_source == Source::Raw {
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
        // Zoomé, les voisines aussi en 1:1 (préchargement) ; loupe : le 1:1 de l'image.
        let zoomed = self.zoom.is_some();
        let mut loupe_extra = None;
        if self.loupe_on
            && !zoomed
            && let Some(file) = self.files.get(self.selected)
        {
            let level = view::zoom_level(file.kind, self.source_for(file));
            let key = (file.path.clone(), level);
            if self.ready(self.selected, level)
                && !self.loaded(self.selected, level)
                && !self.missing.contains(&key)
            {
                loupe_extra = Some((file.clone(), level));
            }
        }
        for index in displayed {
            let kind = self.files[index].kind;
            let source = self.source_for(&self.files[index]);
            let level = view::pick(kind, source, zoomed, |l| self.ready(index, l));
            if let Some(level) = level.filter(|&l| l != Level::Thumbnail) {
                let key = (self.files[index].path.clone(), level);
                if !self.loaded(index, level) && !self.missing.contains(&key) {
                    wanted.push((self.files[index].clone(), level));
                }
            }
        }
        if let Some(extra) = loupe_extra {
            wanted.insert(0, extra);
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

    /// Touches de la recherche, pour les bulles et les indications.
    fn search_keys(&self) -> String {
        let mut keys = vec!["/".to_string()];
        keys.extend(self.keymap.keys_for(Action::Search));
        keys.join(", ")
    }

    /// Ouvre la recherche : dans le panneau des dossiers s'il a le clavier, sinon celle de
    /// la grille (barre des filtres montrée, retour à la grille depuis le plein écran).
    fn start_search(&mut self) {
        if self.focus == crate::actions::Scope::Folders && self.folder_panel {
            self.focus_request = Some(folder_search_id());
            return;
        }
        if self.mode != Mode::Grid {
            if self.mode == Mode::Compare {
                self.leave_compare();
            }
            self.mode = Mode::Grid;
            self.zoom = None;
        }
        self.filter_bar = true;
        self.focus = crate::actions::Scope::Images;
        self.focus_request = Some(grid_search_id());
    }

    /// Échap dans un champ de recherche : la grille efface son filtre, le panneau des
    /// dossiers referme la recherche. Vrai si l'action est prise.
    fn search_escape(&mut self, ctx: &egui::Context) -> bool {
        let field = self.search_focused.take();
        if field == Some(grid_search_id()) {
            ctx.memory_mut(|m| m.surrender_focus(grid_search_id()));
            if !self.name_query.is_empty() {
                self.name_query.clear();
                self.refilter(Reveal::Minimal);
            }
            return true;
        }
        if field == Some(folder_search_id()) {
            ctx.memory_mut(|m| m.surrender_focus(folder_search_id()));
            self.close_folder_search();
            return true;
        }
        false
    }

    /// Referme la recherche de dossiers (l'arborescence revient).
    fn close_folder_search(&mut self) {
        self.folder_query.clear();
        self.folder_search = None;
        self.folder_results.clear();
        self.folder_result = 0;
    }

    /// Relance la recherche de dossiers sur le texte tapé : depuis le dossier pointé dans
    /// le panneau (sinon celui ouvert, sinon le dossier personnel), `~/…` ou `/…`.
    fn restart_folder_search(&mut self, ctx: &egui::Context) {
        self.folder_results.clear();
        self.folder_result = 0;
        if self.folder_query.trim().is_empty() {
            self.folder_search = None;
            return;
        }
        let home = tri::places::home();
        let current = self
            .tree
            .cursor_path()
            .or_else(|| (!self.folder.is_none()).then(|| self.folder.dir.clone()))
            .or_else(|| home.clone())
            .unwrap_or_else(|| PathBuf::from("/"));
        let query = tri::places::parse_dir_query(&self.folder_query, &current, home.as_deref());
        let f = self.gui.folders;
        let repaint = ctx.clone();
        self.folder_search = Some(tri::places::DirSearch::start(
            query,
            f.search_depth,
            f.hidden,
            f.search_results,
            move || repaint.request_repaint(),
        ));
    }

    /// Ouvre le dossier trouvé `path` : la recherche se referme, l'arborescence le montre.
    fn open_found_folder(&mut self, path: PathBuf) {
        self.close_folder_search();
        self.tree.reveal(&path);
        self.tree.point_at(&path);
        self.open_folder(&path);
        self.reveal_folder = true;
        self.focus = crate::actions::Scope::Images;
    }

    /// Champ de recherche du panneau des dossiers et, pendant une recherche, ses
    /// résultats (↑ / ↓ choisissent, Entrée ou un clic ouvre). Vrai si les résultats
    /// remplacent l'arborescence.
    fn folder_search_ui(&mut self, ui: &mut Ui) -> bool {
        let focused_search = ui.memory(|m| m.has_focus(folder_search_id()));
        if focused_search {
            let (up, down) = ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            let last = self.folder_results.len().saturating_sub(1);
            if up {
                self.folder_result = self.folder_result.saturating_sub(1);
            }
            if down {
                self.folder_result = (self.folder_result + 1).min(last);
            }
        }
        let field = ui
            .add(
                egui::TextEdit::singleline(&mut self.folder_query)
                    .id(folder_search_id())
                    .hint_text(t!("search-folder-hint"))
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text(t!("search-folder-tip", keys = self.search_keys()));
        if self.focus_request == Some(folder_search_id()) {
            field.request_focus();
            self.focus_request = None;
        }
        if field.changed() {
            let ctx = ui.ctx().clone();
            self.restart_folder_search(&ctx);
        }
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if field.has_focus() {
            self.search_focused = Some(folder_search_id());
        }
        let Some(search) = self.folder_search.as_ref() else {
            return false;
        };
        let new = search.take();
        let base = search.query().base.clone();
        let running = !search.is_done();
        if !new.is_empty() {
            self.folder_results.extend(new);
            // Meilleurs d'abord ; à score égal, les moins profonds.
            self.folder_results.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then(a.0.components().count().cmp(&b.0.components().count()))
            });
        }
        ui.separator();
        ui.weak(t!("search-folder-in", base = base.display().to_string()));
        if running {
            ui.weak(t!("search-folder-running"));
        } else if self.folder_results.is_empty() {
            ui.weak(t!("search-folder-none"));
        }
        let mut open = None;
        if enter && let Some((path, _)) = self.folder_results.get(self.folder_result) {
            open = Some(path.clone());
        }
        let accent = Color32::from(self.gui.dialogs.title_color);
        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            for (i, (path, _)) in self.folder_results.iter().enumerate() {
                let relative = path.strip_prefix(&base).unwrap_or(path);
                let label =
                    ui.selectable_label(i == self.folder_result, relative.display().to_string());
                if i == self.folder_result {
                    ui.painter().rect_stroke(
                        label.rect.expand(1.0),
                        3.0,
                        egui::Stroke::new(1.5, accent),
                        egui::StrokeKind::Outside,
                    );
                    if focused_search {
                        label.scroll_to_me(None);
                    }
                }
                if label.clicked() {
                    open = Some(path.clone());
                }
            }
        });
        if let Some(path) = open {
            self.open_found_folder(path);
        }
        true
    }

    /// Barre du haut : le mot-symbole « t★ri » à gauche (idée de l'utilisateur, 19/10/2026 :
    /// plus net qu'une icône suivie du nom). Lettres de la couleur du texte du thème,
    /// étoile jaune.
    fn title_bar(&mut self, ui: &mut Ui) {
        let (letters, star) = self
            .logo
            .get_or_insert_with(|| {
                let load = |name: &str, bytes: &[u8]| {
                    let image = image::load_from_memory(bytes)
                        .expect("mot-symbole embarqué lisible")
                        .to_rgba8();
                    let size = [image.width() as usize, image.height() as usize];
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, &image);
                    ui.ctx()
                        .load_texture(name, pixels, egui::TextureOptions::LINEAR)
                };
                (
                    load("wordmark", include_bytes!("../assets/wordmark-letters.png")),
                    load(
                        "wordmark-star",
                        include_bytes!("../assets/wordmark-star.png"),
                    ),
                )
            })
            .clone();
        let height = self.gui.title_bar_size;
        let size = letters.size_vec2() * (height / letters.size_vec2().y);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let painter = ui.painter();
        painter.image(letters.id(), rect, uv, ui.visuals().strong_text_color());
        painter.image(star.id(), rect, uv, Color32::WHITE);
        response.on_hover_text(format!("tri {}", self.build.label()));
    }

    /// Mode concentration : l'indication pour en sortir, en haut au centre, un moment.
    fn focus_mode_hint(&mut self, ctx: &egui::Context) {
        let Some(since) = self.focus_mode_since.filter(|_| self.focus_mode) else {
            return;
        };
        let duration = Duration::from_millis(self.gui.focus_mode_hint_ms);
        if since.elapsed() >= duration {
            self.focus_mode_since = None;
            return;
        }
        ctx.request_repaint_after(duration - since.elapsed());
        let keys = self.keymap.keys_for(Action::FocusMode).join(" / ");
        egui::Area::new(egui::Id::unique("focus_mode_hint"))
            .anchor(Align2::CENTER_TOP, vec2(0.0, 16.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(t!("focus-mode-on", keys = keys));
                });
            });
    }

    /// Le développé de l'image `index` existe, **pour son sidecar actuel** : la clé de
    /// cache suit le sidecar (taille, date), un développement modifié dans darktable n'y
    /// est plus. Un développé déjà chargé mais périmé est oublié (état, texture) : `b` ne
    /// le montre plus, Maj+B le refait (retour du 19/10/2026 : recadrage changé, l'ancien
    /// restait affiché).
    /// Change la source choisie ; boîtier ou RAW deviennent aussi la source des images
    /// sans développement quand on passe ensuite au développé.
    fn set_source(&mut self, source: Source) {
        if source != Source::Developed {
            self.base_source = source;
        }
        self.source = source;
    }

    /// Source affichée pour `file` : la source choisie, sauf « développé » sur une image
    /// qui n'a pas (encore) de développement : alors celle d'avant, avec son 1:1 (retour
    /// du 20/10/2026 : après un Maj+B, plus aucune autre image ne zoomait).
    fn source_for(&self, file: &MediaFile) -> Source {
        if self.source != Source::Developed {
            return self.source;
        }
        let developed = self.engine.as_ref().is_some_and(|engine| {
            tri::preview::key(file, Level::Developed, &engine.preview)
                .is_ok_and(|key| engine.cache.contains(&key))
        });
        if developed {
            Source::Developed
        } else {
            self.base_source
        }
    }

    fn developed_ready(&mut self, index: usize) -> bool {
        let (Some(engine), Some(file)) = (&self.engine, self.files.get(index)) else {
            return false;
        };
        let current = tri::preview::key(file, Level::Developed, &engine.preview)
            .is_ok_and(|key| engine.cache.contains(&key));
        if !current && self.status[index].developed == Avail::Ready {
            let path = file.path.clone();
            trace(|| format!("développé périmé : {}", path.display()));
            self.status[index].developed = Avail::Pending;
            self.requested.remove(&(path.clone(), Level::Developed));
            self.big.remove(&(path, Level::Developed));
            if index == self.selected {
                let keys = self.keymap.keys_for(Action::Develop).join(" / ");
                self.notice = Some((t!("develop-stale", keys = keys), false));
            }
        }
        current
    }

    /// Rapport largeur / hauteur de `path` tel qu'affiché (redressé), d'après une image déjà
    /// chargée ; `None` si aucune ne l'est.
    fn image_aspect(&mut self, path: &Path) -> Option<f64> {
        let big = [
            Level::Developed,
            Level::Standard,
            Level::Embedded,
            Level::EmbeddedFull,
            Level::Full,
        ]
        .into_iter()
        .find_map(|level| {
            self.big
                .get(&(path.to_path_buf(), level))
                .map(|b| b.original)
        });
        let size = big.or_else(|| {
            self.textures
                .get(&path.to_path_buf())
                .map(|t| t.size_vec2())
        })?;
        (size.y > 0.0).then(|| f64::from(size.x / size.y))
    }

    /// Résolution de `file` telle qu'on la regarde (pixels, orientation appliquée) : celle
    /// du 100 % de la source affichée (le 1:1 s'il est chargé ; sinon les dimensions lues
    /// dans les métadonnées pour un rendu RAW ou une image plate, celle du dernier 1:1 de
    /// JPEG boîtier vu) ; pour une `version`, son cadre, ramené au côté long de son
    /// préréglage d'export s'il en réduit la taille. Développé : aucune pour l'instant.
    fn shown_resolution(
        &mut self,
        ctx: &egui::Context,
        file: &MediaFile,
        version: Option<&tri::versions::Version>,
    ) -> Option<(u32, u32)> {
        let source = self.source_for(file);
        let level = view::zoom_level(file.kind, source);
        let loaded = |app: &mut Self, level| {
            app.big
                .get(&(file.path.clone(), level))
                .map(|b| b.original)
                .filter(|o| o.x > 0.0 && o.y > 0.0)
        };
        let read = self
            .meta
            .get_or_insert_with(|| MetaLoader::new(ctx.clone()))
            .get(file)
            .and_then(|m| m.as_ref().ok())
            .cloned();
        let size = |s: Option<(u32, u32)>| s.map(|(w, h)| vec2(w as f32, h as f32));
        let native = if source == Source::Developed {
            // Développé : darktable rend à la taille de l'aperçu ; sa vraie résolution,
            // celle de l'image entière recadrée dans darktable, se lit dans le sidecar.
            let full = read.as_ref()?.size?;
            size(self.developed_size(&file.path, full))?
        } else {
            loaded(self, level).or_else(|| match (file.kind, source) {
                // Rendu boîtier : la taille du JPEG embarqué, lue dans son en-tête.
                (MediaKind::Raw, Source::Embedded) => size(read.as_ref()?.embedded_size),
                _ => size(read.as_ref()?.size),
            })?
        };
        let (mut w, mut h) = (native.x as f64, native.y as f64);
        if let Some(v) = version {
            w *= v.crop.w;
            h *= v.crop.h;
            let side = v
                .profile
                .as_deref()
                .and_then(|id| self.settings.export.profiles.get(id))
                .map_or(0, |p| p.max_side);
            let long = w.max(h);
            if side > 0 && long > f64::from(side) {
                let k = f64::from(side) / long;
                w *= k;
                h *= k;
            }
        }
        Some((w.round() as u32, h.round() as u32))
    }

    /// Taille du développé darktable de `path` (image de `full` pixels) : recadrage lu
    /// dans le sidecar (`develop::developed_size`), relu seulement s'il a changé.
    fn developed_size(&mut self, path: &Path, full: (u32, u32)) -> Option<(u32, u32)> {
        let sidecar = tri::develop::darktable_sidecar_path(path)?;
        let stamp = tri::develop::stamp(&sidecar);
        if let Some((known, size)) = self.developed_sizes.get(path)
            && *known == stamp
        {
            return *size;
        }
        let size = std::fs::read_to_string(&sidecar)
            .ok()
            .and_then(|xmp| tri::develop::developed_size(&xmp, full));
        self.developed_sizes
            .insert(path.to_path_buf(), (stamp, size));
        size
    }

    /// Version montrée en plein écran (image et version), s'il y en a une.
    fn shown_version(&self) -> Option<(PathBuf, tri::versions::Version)> {
        if self.mode != Mode::View {
            return None;
        }
        let file = self.files.get(self.selected)?;
        let (path, index) = self.version_shown.as_ref()?;
        if *path != file.path {
            return None;
        }
        let version = self.versions.get(path)?.list.get(*index)?.clone();
        Some((path.clone(), version))
    }

    /// Versions de `path` (lues dans son sidecar à la première demande).
    fn versions_of(&mut self, path: &Path) -> &tri::versions::Versions {
        self.versions
            .entry(path.to_path_buf())
            .or_insert_with(|| tri::versions::read(path).unwrap_or_default())
    }

    /// Rouvre le cadre de la version `index` de `path`, sur l'image où elle a été faite.
    fn edit_version(&mut self, path: &Path, index: usize) {
        let Some(version) = self.versions_of(path).list.get(index).cloned() else {
            return;
        };
        self.set_source(match version.source {
            tri::versions::Source::Embedded => Source::Embedded,
            tri::versions::Source::Raw => Source::Raw,
            tri::versions::Source::Developed => Source::Developed,
        });
        self.zoom = None;
        self.version_shown = None;
        let others = self.versions_of(path).without(Some(index));
        self.crop = Some(crate::crop::Crop::edit(
            path.to_path_buf(),
            index,
            &version,
            others,
        ));
        self.request_zoom();
    }

    /// Supprime la version `index` de `path` (annulable).
    fn delete_version(&mut self, path: &Path, index: usize) {
        let convention = self.settings.xmp.sidecar;
        let result = tri::versions::read(path).and_then(|mut versions| {
            let before = versions.items();
            if index >= versions.list.len() {
                return Ok(None);
            }
            let removed = versions.list.remove(index);
            tri::versions::write(path, &versions, convention)?;
            Ok(Some((before, versions.items(), removed.name)))
        });
        self.versions.remove(path);
        self.version_shown = None;
        match result {
            Ok(Some((before, after, name))) => {
                self.history.push(tri::history::Step::Versions {
                    image: path.to_path_buf(),
                    before,
                    after,
                    change: tri::history::VersionChange::Deleted(name.clone()),
                });
                self.notice = Some((t!("versions-deleted", name = name), false));
            }
            Ok(None) => {}
            Err(e) => self.notice = Some((t!("crop-failed", error = e.to_string()), true)),
        }
    }

    /// `r` : recadrer l'image (nouvelle version) ; en plein écran, ajusté (depuis la grille,
    /// l'ouvre). On recadre ce qu'on voit : la source affichée.
    fn start_crop(&mut self) {
        let Some(file) = self.files.get(self.selected) else {
            return;
        };
        let path = file.path.clone();
        if self.mode != Mode::View {
            self.mode = Mode::View;
        }
        self.zoom = None;
        // On recadre ce qu'on voit.
        let source = match self.source_for(file) {
            Source::Embedded => tri::versions::Source::Embedded,
            Source::Raw => tri::versions::Source::Raw,
            Source::Developed => tri::versions::Source::Developed,
        };
        let aspect = self.image_aspect(&path);
        let others = self.versions_of(&path).clone();
        self.crop = Some(crate::crop::Crop::new(path, source, aspect, others));
        self.request_zoom();
    }

    /// Applique le préréglage n° `index` au recadrage en cours.
    fn apply_preset(&mut self, index: usize) {
        let Some(preset) = self.settings.versions.presets.0.get(index).cloned() else {
            return;
        };
        // Profil : seulement s'il existe et s'applique (jamais « tel quel » pour une version).
        let flat = self.crop.as_ref().is_some_and(|c| {
            c.source == tri::versions::Source::Embedded
                || tri::media::kind_of(&c.image) == Some(MediaKind::Flat)
        });
        let profile = self
            .settings
            .export
            .applicable(flat)
            .into_iter()
            .find(|p| p.id == preset.profile && !p.is_as_is())
            .map(|p| p.id.clone());
        if let Some(crop) = &mut self.crop {
            crop.apply_preset(&preset.id, preset_name(&preset), preset.ratio, profile);
        }
    }

    /// Tab / Maj+Tab pendant le recadrage : préréglage suivant / précédent.
    fn next_preset(&mut self, forward: bool) {
        let presets = &self.settings.versions.presets.0;
        if presets.is_empty() {
            return;
        }
        let current = self
            .crop
            .as_ref()
            .and_then(|c| c.preset.as_ref())
            .and_then(|id| presets.iter().position(|p| p.id == *id));
        let n = presets.len();
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        self.apply_preset(next);
    }

    /// Enregistre le cadre en cours comme préréglage (rapport, nom, profil), dans le
    /// fichier de configuration (commentaires et autres clés gardés), puis l'applique.
    fn save_preset(&mut self) {
        let Some(crop) = &self.crop else { return };
        let name = crop.name.trim().to_string();
        let (ratio, profile) = (crop.ratio, crop.profile.clone().unwrap_or_default());
        let Some(path) = self
            .config_path
            .clone()
            .or_else(tri::settings::default_path)
        else {
            self.notice = Some((t!("settings-no-path"), true));
            return;
        };
        let presets = &self.settings.versions.presets.0;
        // Pas deux préréglages du même nom (issue n° 17) : « Story (2) ».
        let name = tri::versions::free_name(&name, |n| {
            presets
                .iter()
                .any(|p| tri::versions::same_name(&preset_name(p), n))
        });
        let id = tri::versions::new_id(&name, |id| presets.iter().any(|p| p.id == id));
        let section = format!("versions.presets.{id}");
        let result = tri::config_edit::ConfigFile::open(&path).and_then(|mut file| {
            use tri::config_edit::Value;
            file.set(&section, "name", &Value::Str(name.clone()))?;
            file.set(&section, "ratio", &Value::Str(ratio.label()))?;
            file.set(&section, "profile", &Value::Str(profile))?;
            let settings = tri::settings::parse::<AppSettings>(&file.text())?;
            file.save()
                .map_err(|e| format!("{} : {e}", path.display()))?;
            Ok(settings)
        });
        match result {
            Ok(settings) => {
                self.settings.versions = settings.versions;
                if let Some(crop) = &mut self.crop {
                    crop.preset = Some(id);
                }
                self.notice = Some((t!("crop-preset-saved", name = name), false));
            }
            Err(e) => self.notice = Some((e, true)),
        }
    }

    /// Enregistre la version du recadrage en cours (annulable), dans le sidecar de l'image.
    fn save_crop(&mut self) {
        // Proportions de l'image pas encore connues (rien d'affiché) : le cadre serait faux.
        if !self.crop.as_ref().is_some_and(crate::crop::Crop::ready) {
            return;
        }
        // Nom vide ou déjà pris (issue n° 14) : le bouton est grisé, le clavier aussi.
        if let Some(crop) = self.crop.as_ref().filter(|c| !c.name_ok()) {
            if crop.name_taken() {
                let name = crop.name.trim().to_string();
                self.notice = Some((t!("crop-name-taken-notice", name = name), true));
            }
            return;
        }
        let Some(crop) = self.crop.take() else {
            return;
        };
        let image = crop.image.clone();
        let convention = self.settings.xmp.sidecar;
        let result = tri::versions::read(&image).and_then(|mut versions| {
            let before = versions.items();
            let mut version = crop.version();
            let editing = crop.editing.filter(|&i| i < versions.list.len());
            // Lu juste avant d'écrire : un nom pris entre-temps est numéroté.
            version.name = versions.without(editing).new_name(&version.name);
            let change = match editing {
                Some(i) => {
                    versions.list[i] = version.clone();
                    tri::history::VersionChange::Changed(version.name.clone())
                }
                None => {
                    versions.list.push(version.clone());
                    tri::history::VersionChange::Created(version.name.clone())
                }
            };
            tri::versions::write(&image, &versions, convention)?;
            Ok((before, versions.items(), change, version.name))
        });
        match result {
            Ok((before, after, change, name)) => {
                self.versions.remove(&image);
                // La version enregistrée est montrée.
                let index = self
                    .versions_of(&image)
                    .list
                    .iter()
                    .position(|v| v.name == name);
                self.version_shown = index.map(|i| (image.clone(), i));
                self.history.push(tri::history::Step::Versions {
                    image: image.clone(),
                    before,
                    after,
                    change,
                });
                self.notice = Some((t!("crop-saved", name = name), false));
            }
            Err(e) => {
                self.notice = Some((t!("crop-failed", error = e.to_string()), true));
                // Le recadrage reste ouvert : rien n'est perdu.
                self.crop = Some(crop);
            }
        }
    }

    /// Maj+B ou le bouton : développer l'image affichée dans darktable, puis l'afficher (à
    /// la demande seulement : quelques secondes).
    fn develop_current(&mut self) {
        if !self.darktable {
            self.notice = Some((t!("develop-not-found"), true));
            return;
        }
        if self.files.is_empty() {
            return;
        }
        if self.mode == Mode::Grid {
            self.mode = Mode::View;
        }
        self.set_source(Source::Developed);
        // Périmé (sidecar modifié depuis) : oublié ici, donc recalculé.
        self.developed_ready(self.selected);
        self.request(self.selected, Level::Developed);
        self.request_zoom();
    }

    /// Relit les superpositions : grilles intégrées (couleur et opacité des réglages),
    /// puis les fichiers du dossier `overlays` des données. Renvoie la première erreur.
    fn reload_overlays(&mut self) -> Option<String> {
        let [r, g, b] = self.gui.colors.overlay.0;
        let alpha = (self.gui.view.overlay_opacity.clamp(0.0, 1.0) * 255.0) as u8;
        let mut list = tri::overlay::builtin(tri::overlay::Rgba([r, g, b, alpha]));
        let mut error = None;
        if let Some(data) = &self.data_dir {
            let (user, errors) = tri::overlay::load_dir(&tri::overlay::user_dir(data));
            list.extend(
                user.into_iter()
                    .map(|o| Box::new(o) as Box<dyn tri::overlay::Overlay>),
            );
            error = errors
                .into_iter()
                .next()
                .map(|(file, e)| t!("overlay-error", file = folder_name(&file), error = e));
        }
        self.overlays = list;
        // Un logo a pu changer : rechargé à son prochain dessin.
        self.overlay_textures.clear();
        self.overlay = self.overlay.min(self.overlays.len());
        error
    }

    /// Superposition suivante ; aucune après la dernière.
    fn cycle_overlay(&mut self) {
        let error = self.reload_overlays();
        self.overlay = (self.overlay + 1) % (self.overlays.len() + 1);
        self.notice = Some(match error {
            Some(e) => (e, true),
            None => match self
                .overlay
                .checked_sub(1)
                .and_then(|i| self.overlays.get(i))
            {
                Some(o) => (t!("overlay-shown", name = o.name()), false),
                None => (t!("overlay-none"), false),
            },
        });
    }

    /// Superposition affichée, sur l'image dessinée en `rect` (formes du moteur en
    /// coordonnées relatives, mises à l'échelle de l'image, zoom compris).
    fn draw_overlay(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Option<Rect>) {
        let Some(rect) = rect.filter(|r| r.height() > 0.0) else {
            return;
        };
        let Some(overlay) = self
            .overlay
            .checked_sub(1)
            .and_then(|i| self.overlays.get(i))
        else {
            return;
        };
        let shapes = overlay.shapes(rect.width() / rect.height());
        let at = |p: [f32; 2]| rect.min + vec2(p[0] * rect.width(), p[1] * rect.height());
        let color =
            |c: tri::overlay::Rgba| Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]);
        let width = self.gui.view.overlay_line_width;
        for shape in shapes {
            match shape {
                tri::overlay::Shape::Line { from, to, color: c } => {
                    // Lignes seulement : un texte ou un logo ne se retournent pas.
                    let (from, to) = (self.overlay_flip.apply(from), self.overlay_flip.apply(to));
                    // Liseré sombre dessous : lisible sur un ciel comme sur une ombre.
                    let shadow = Color32::from_black_alpha(c.0[3] / 3);
                    painter
                        .line_segment([at(from), at(to)], egui::Stroke::new(width + 2.0, shadow));
                    painter.line_segment([at(from), at(to)], egui::Stroke::new(width, color(c)));
                }
                tri::overlay::Shape::Rect {
                    min,
                    max,
                    stroke,
                    fill,
                } => {
                    let r = Rect::from_min_max(at(min), at(max));
                    if let Some(f) = fill {
                        painter.rect_filled(r, 0.0, color(f));
                    }
                    if let Some(s) = stroke {
                        painter.rect_stroke(
                            r,
                            0.0,
                            egui::Stroke::new(width, color(s)),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                tri::overlay::Shape::Text {
                    at: p,
                    anchor,
                    text,
                    size,
                    color: c,
                } => {
                    let [fx, fy] = anchor.factors();
                    let align = |f: f32| match f {
                        f if f < 0.25 => egui::Align::Min,
                        f if f > 0.75 => egui::Align::Max,
                        _ => egui::Align::Center,
                    };
                    let font = FontId::proportional((size * rect.height()).max(4.0));
                    painter.text(at(p), Align2([align(fx), align(fy)]), text, font, color(c));
                }
                tri::overlay::Shape::Image {
                    path,
                    min,
                    max,
                    opacity,
                } => {
                    let texture = self
                        .overlay_textures
                        .entry(path.clone())
                        .or_insert_with(|| {
                            let image = image::open(&path).ok()?.to_rgba8();
                            let size = [image.width() as usize, image.height() as usize];
                            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, &image);
                            Some(ctx.load_texture(
                                format!("overlay:{}", path.display()),
                                pixels,
                                egui::TextureOptions::LINEAR,
                            ))
                        });
                    if let Some(texture) = texture {
                        painter.image(
                            texture.id(),
                            Rect::from_min_max(at(min), at(max)),
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE.gamma_multiply(opacity.clamp(0.0, 1.0)),
                        );
                    }
                }
            }
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

    /// Carte de netteté de `file` sur l'image affichée (niveau `level`, en `rect` ; `uv` :
    /// la partie montrée, celle d'une version), en vue ajustée comme en 1:1 (retour du
    /// 21/10/2026 ; avant : en 1:1 seulement), au seuil du curseur de sévérité : dessinée si
    /// elle est chargée, demandée au chargeur sinon ; sans objet, la demande en cours est
    /// retirée.
    fn peaking_overlay(
        &mut self,
        painter: &egui::Painter,
        file: &MediaFile,
        level: Option<Level>,
        rect: Option<Rect>,
        uv: Rect,
    ) -> Peaking {
        let sharpness = self
            .settings
            .sharpness
            .with_strictness(self.blur_strictness);
        let wanted = level
            .filter(|&l| self.peaking && l != Level::Thumbnail)
            .map(|l| (file.path.clone(), l, sharpness.threshold));
        let Some(key) = wanted else {
            if self.peaking_asked.take().is_some()
                && let Some(engine) = &self.engine
            {
                engine.loader.want_peaking(None);
            }
            return Peaking::Computing;
        };
        if let (Some((loaded, texture)), Some(rect)) = (&self.peaking_texture, rect)
            && *loaded == key
        {
            let colors = &self.gui.sharpness.colors;
            let color = colors
                .get(self.peaking_color % colors.len().max(1))
                .map_or(Color32::RED, |&c| Color32::from(c))
                .gamma_multiply(self.gui.sharpness.opacity);
            painter.image(texture.id(), rect, uv, color);
            return Peaking::Shown;
        }
        if self.peaking_asked.as_ref() != Some(&key)
            && let Some(engine) = &self.engine
        {
            engine
                .loader
                .want_peaking(Some((file.clone(), key.1, sharpness)));
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
            painter.rect_filled(rect, 0.0, self.view_background());
            let source = self.source_for(&file);
            let zoom_level = view::zoom_level(file.kind, source);
            let desired = view::pick(file.kind, source, zoomed, |l| self.ready(index, l));
            let shown = view::pick(file.kind, source, zoomed, |l| {
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
                        let size = original * self.zoom_scale / ctx.pixels_per_point();
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
                self.draw_overlay(&ctx, &painter, Some(image_rect));
                // Repères du capteur : faux sur un développé recadré.
                let sensor = shown != Some(Level::Developed);
                self.draw_focus_point(&ctx, &painter, &file, sensor.then_some(image_rect));
                if self.clipping
                    && sensor
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
                let color = self.gui.theme.selection(&self.gui.colors, ui.visuals());
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

    /// Zoom à la souris et au pavé tactile, en continu (décidé le 18/10/2026) : pincer, ou
    /// Ctrl / Cmd + molette, zoome autour du pointeur (entre l'ajusté et `[gui.zoom]
    /// max`) ; défiler déplace l'image zoomée. `rect` : l'image telle qu'affichée, `px` :
    /// la taille de son 100 % (pixels).
    fn zoom_input(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        view_rect: Rect,
        rect: Rect,
        px: Vec2,
    ) {
        if !response.contains_pointer() {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let (factor, scroll, pointer) =
            ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        if (factor - 1.0).abs() > 1e-4
            && let Some(pos) = pointer
        {
            let fit = self.fit_scale;
            let current = if self.zoom.is_some() {
                self.zoom_scale
            } else {
                fit
            };
            let z = &self.gui.zoom;
            // Sous l'ajusté aussi, jusqu'à `min` ; l'ajusté lui-même : pas de zoom.
            let new = (current * factor).clamp(z.min.min(fit), z.max.max(fit));
            if (new - fit).abs() <= fit * 1e-3 {
                self.zoom = None;
            } else {
                let point = view::relative(rect, pos);
                self.zoom_scale = new;
                let size = px * new / ppp;
                self.zoom = Some(view::center_keeping(view_rect, size, point, pos));
            }
            self.request_zoom();
            ctx.request_repaint();
        } else if scroll != Vec2::ZERO
            && let Some(center) = self.zoom
        {
            // Défiler déplace l'image zoomée (deux doigts sur le pavé tactile).
            let size = px * self.zoom_scale / ppp;
            let c = center - vec2(scroll.x / size.x, scroll.y / size.y);
            self.zoom = Some(view::clamp_center(view_rect.size(), size, c));
            ctx.request_repaint();
        }
    }

    /// Loupe (décidé le 18/10/2026) : en vue ajustée, tant que la touche de
    /// `[gui.zoom] loupe_key` est maintenue (après `loupe_delay_ms`), l'image à
    /// `loupe_scale` (100 %) sous le pointeur, dans un carré ; le 1:1 est chargé pour
    /// l'occasion, l'aperçu agrandi en attendant.
    fn loupe(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        index: usize,
        image_rect: Option<Rect>,
    ) {
        let z = self.gui.zoom.clone();
        let held = ctx.input(|i| z.loupe_key.held(i));
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let over = match (image_rect, pointer) {
            (Some(r), Some(p)) => r.contains(p).then_some((r, p)),
            _ => None,
        };
        if !held || self.zoom.is_some() || over.is_none() {
            self.loupe_since = None;
            self.loupe_on = false;
            return;
        }
        let since = *self.loupe_since.get_or_insert_with(Instant::now);
        let delay = Duration::from_millis(z.loupe_delay_ms);
        if since.elapsed() < delay {
            ctx.request_repaint_after(delay - since.elapsed());
            return;
        }
        if !self.loupe_on {
            self.loupe_on = true;
            self.request_zoom();
        }
        let Some((rect, pos)) = over else { return };
        let file = &self.files[index];
        let level = view::zoom_level(file.kind, self.source_for(file));
        let ppp = ctx.pixels_per_point();
        // Le 1:1 s'il est chargé, sinon l'image affichée (agrandie, floue un instant).
        let full = self.big.get(&(file.path.clone(), level)).cloned();
        let (texture, px, loading) = match full {
            Some(b) => (b.texture.id(), b.original, false),
            None => match self
                .big
                .get(&(file.path.clone(), Level::Standard))
                .cloned()
                .or_else(|| self.big.get(&(file.path.clone(), Level::Embedded)).cloned())
            {
                Some(b) => {
                    let px = self
                        .last_zoom_size
                        .get(&level)
                        .map(|&last| view::estimate_zoom_size(last, b.original.x / b.original.y))
                        .unwrap_or(b.original);
                    (b.texture.id(), px, true)
                }
                None => return,
            },
        };
        let side = z.loupe_size;
        let square = Rect::from_center_size(pos, vec2(side, side));
        let size = px * z.loupe_scale / ppp;
        let point = view::relative(rect, pos);
        let whole = Rect::from_min_size(pos - vec2(point.x * size.x, point.y * size.y), size);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let clipped = painter.with_clip_rect(square);
        clipped.rect_filled(square, 0.0, self.view_background());
        clipped.image(texture, whole, uv, Color32::WHITE);
        painter.rect_stroke(
            square,
            4.0,
            egui::Stroke::new(2.0, Color32::from_gray(220)),
            egui::StrokeKind::Outside,
        );
        if loading {
            overlay(
                painter,
                square.left_top() + vec2(6.0, 6.0),
                Align2::LEFT_TOP,
                &t!("view-loupe-loading"),
            );
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Plein écran : l'image sélectionnée, ajustée ou à 100 %, avec ce qui est affiché.
    fn viewer(&mut self, ui: &mut Ui) {
        self.apply_pending_focus_zoom();
        let ctx = ui.ctx().clone();
        let index = self.selected;
        let file = self.files[index].clone();
        let zoomed = self.zoom.is_some();
        let source = self.source_for(&file);
        let desired = view::pick(file.kind, source, zoomed, |l| self.ready(index, l));
        // En attendant que l'image voulue soit chargée, la meilleure déjà affichable.
        let shown = view::pick(file.kind, source, zoomed, |l| {
            self.ready(index, l) && self.loaded(index, l)
        });
        if (index, shown) != self.last_shown {
            trace(|| format!("affichée : image {index} {shown:?} (voulue {desired:?})"));
            self.last_shown = (index, shown);
        }
        // En attente tant que l'image voulue n'est pas affichée, ou que le 1:1 demandé
        // n'est pas encore calculé.
        let zoom_level = view::zoom_level(file.kind, source);
        self.view_waiting =
            shown != desired || shown.is_none() || (zoomed && shown != Some(zoom_level));

        let (view_rect, response) =
            ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(view_rect);
        painter.rect_filled(view_rect, 0.0, self.view_background());
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
        // Version montrée (ni zoomée ni en cours de recadrage) : son cadre.
        let shown_crop = self
            .version_shown
            .as_ref()
            .filter(|(p, _)| *p == file.path && self.zoom.is_none() && self.crop.is_none())
            .and_then(|(p, i)| self.versions.get(p).and_then(|v| v.list.get(*i)))
            .map(|v| v.crop);
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
                    // Échelle 1,0 : un pixel de l'image = un pixel de l'écran.
                    let size = px * self.zoom_scale / ctx.pixels_per_point();
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
                // Version montrée : seulement son cadre, ajusté à la vue.
                _ => match shown_crop {
                    Some(c) => view::fit_rect(
                        view_rect,
                        vec2(original.x * c.w as f32, original.y * c.h as f32),
                    ),
                    None => view::fit_rect(view_rect, original),
                },
            };
            // Échelle de l'ajusté, rapportée au 100 % (connu ou estimé), pour les paliers.
            let px = zoom_px
                .or_else(|| {
                    self.last_zoom_size
                        .get(&zoom_level)
                        .map(|&last| view::estimate_zoom_size(last, original.x / original.y))
                })
                .unwrap_or(original);
            let ppp = ctx.pixels_per_point();
            self.fit_scale = (view_rect.width() * ppp / px.x).min(view_rect.height() * ppp / px.y);
            let uv = match shown_crop {
                Some(c) => {
                    Rect::from_min_size(pos2(c.x as f32, c.y as f32), vec2(c.w as f32, c.h as f32))
                }
                None => uv,
            };
            painter.image(texture.id(), rect, uv, Color32::WHITE);
            image_rect = Some(rect);
            // Recadrage : la souris règle le cadre, pas le zoom.
            if self.crop.is_none() {
                self.zoom_input(&ctx, &response, view_rect, rect, px);
            }
            // Zones saturées / bouchées, étirées sur l'image (aussi zoomée). Pas sur le
            // développé : recadré, il ne correspond plus au capteur.
            if self.clipping
                && shown != Some(Level::Developed)
                && shown_crop.is_none()
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
        // Recadrage : voile, cadre et poignées ; la superposition se cale sur le cadre.
        let crop_frame = match (&mut self.crop, image_rect) {
            (Some(crop), Some(rect)) if crop.image == file.path => {
                Some(crop.show(&painter, &response, rect, &self.gui.crop))
            }
            _ => None,
        };
        self.draw_overlay(&ctx, &painter, crop_frame.or(image_rect));
        // Collimateur : placé sur le capteur entier, faux sur un développé recadré.
        let sensor_rect =
            image_rect.filter(|_| shown != Some(Level::Developed) && shown_crop.is_none());
        self.draw_focus_point(&ctx, &painter, &file, sensor_rect);
        self.loupe(&ctx, &painter, index, image_rect);
        // Carte de netteté, sur l'image affichée (ajustée ou 1:1 ; une version : son cadre).
        let peaking_uv = match shown_crop {
            Some(c) => {
                Rect::from_min_size(pos2(c.x as f32, c.y as f32), vec2(c.w as f32, c.h as f32))
            }
            None => uv,
        };
        let peaking = self.peaking_overlay(&painter, &file, shown, image_rect, peaking_uv);

        self.open_rating_dialog(&response, index);
        // Double-clic : 100 % centré sur le point cliqué, ou retour à l'ajusté.
        if response.double_clicked() {
            self.zoom_anchor = None;
            self.zoom_scale = 1.0;
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

        // Ce qui est affiché : dans la barre d'état, après le nom (demandé le 22/10/2026 :
        // le nom y était déjà, plus rien sur l'image) ; sur l'image seulement en mode
        // concentration, où la barre d'état est masquée (Ctrl+I les masque).
        let position = format!("{} / {}", index + 1, self.files.len());
        let mut info = String::new();
        if self.selection.contains(&file.path) {
            info.push_str(&t!("view-info-selected"));
        }
        if self.zoom.is_some() {
            let percent = (self.zoom_scale * 100.0).round() as i64;
            info.push_str(&t!("view-info-zoom", percent = percent));
        }
        if self.peaking {
            info.push_str(&match peaking {
                Peaking::Shown => t!("view-info-peaking"),
                Peaking::Computing => t!("view-info-peaking-computing"),
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
        // Version montrée : son nom.
        if shown_crop.is_some()
            && let Some(name) = self
                .version_shown
                .as_ref()
                .and_then(|(p, i)| self.versions.get(p).and_then(|v| v.list.get(*i)))
                .map(|v| v.name.clone())
        {
            info.push_str(" · ");
            info.push_str(&name);
        }
        // Résolution de ce qu'on regarde (demandé le 21/10/2026) : celle de l'image à
        // 100 % ; une version, son cadre, réduit si son préréglage d'export le réduit.
        // Pendant le recadrage, le cadre en cours et le préréglage d'export de la barre
        // (retour du 21/10/2026 : on lisait encore la taille de l'image entière).
        let editing = self
            .crop
            .as_ref()
            .filter(|c| c.image == file.path)
            .map(|c| c.version());
        let version = editing.or(shown_crop.and(
            self.version_shown
                .as_ref()
                .and_then(|(p, i)| self.versions.get(p).and_then(|v| v.list.get(*i)))
                .cloned(),
        ));
        if let Some((w, h)) = self.shown_resolution(&ctx, &file, version.as_ref()) {
            info.push_str(&t!("view-info-resolution", width = w, height = h));
        }
        // Mode RAW, mais faute de mieux c'est encore le JPEG qui est affiché : le dire.
        let raw_pending = self.source_for(&self.files[index]) == Source::Raw
            && file.kind == MediaKind::Raw
            && matches!(
                shown,
                Some(Level::Thumbnail | Level::Embedded | Level::EmbeddedFull)
            );
        // Développé : en cours, ou pas de développement pour cette image (raison au
        // survol de la barre d'état, ici en clair).
        let developed = &self.status[index].developed;
        if self.source == Source::Developed && shown != Some(Level::Developed) {
            let asked = self
                .requested
                .contains(&(file.path.clone(), Level::Developed));
            info.push_str(&match developed {
                Avail::Failed(e) => format!(" · {e}"),
                _ if asked => t!("view-info-developed-pending"),
                _ => t!(
                    "view-info-developed-none",
                    keys = self.keymap.keys_for(Action::Develop).join(" / ")
                ),
            });
        } else if raw_pending {
            info.push_str(&t!("view-info-raw-pending"));
        } else if zoomed && shown != Some(zoom_level) {
            info.push_str(&match zoom_level {
                Level::Full => t!("view-info-full-computing"),
                _ => t!("view-info-one-to-one-loading"),
            });
        }
        if self.focus_mode && self.image_info {
            overlay(
                &painter,
                view_rect.left_top() + vec2(10.0, 10.0),
                Align2::LEFT_TOP,
                &format!("{position} · {}{info}", file_name(&file.path)),
            );
        }
        self.view_info = format!("{position}{info}");
        // Notes sur l'image seulement sans le bandeau du bas, qui les montre déjà (issue
        // n° 8) : bandeau masqué (Maj+N) ou mode concentration.
        let notes = self.folder.notes.notes(&file.path);
        let size = self.gui.view.marks;
        let bar_shown = self.rating_bar && !self.focus_mode;
        if *notes != tri::xmp::Notes::EMPTY && !bar_shown && self.image_info {
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
        let c = self.gui.colors.clone();
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
            let notes = self.loading.as_ref().map_or(Progress::default(), |l| {
                let (done, total) = l.progress();
                Progress { done, total }
            });
            for (name, p) in [
                (t!("progress-notes"), notes),
                (t!("progress-thumbnails"), self.progress[0]),
                (t!("progress-embedded"), self.progress[1]),
                (t!("progress-previews"), self.progress[2]),
                (t!("progress-histograms"), self.progress[3]),
                (t!("progress-sharpness"), self.progress[4]),
                (t!("progress-groups"), self.progress[5]),
            ] {
                // Rien à calculer en fond pour ce niveau, ou déjà fini.
                // Juste après un refiltrage : le cache répond souvent avant le délai.
                let waiting = self.progress_hidden_until.is_some_and(|t| {
                    let now = Instant::now();
                    if now < t {
                        ui.ctx().request_repaint_after(t - now);
                    }
                    now < t
                });
                if p.total == 0 || waiting || (self.gui.progress_hide_done && p.done >= p.total) {
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
            if let Some(job) = &self.export_job {
                let done = job.done.load(std::sync::atomic::Ordering::Relaxed);
                let fill = self
                    .gui
                    .colors
                    .progress
                    .map_or(ui.visuals().widgets.inactive.bg_fill, Color32::from);
                let text = t!(
                    "export-progress",
                    profile = job.profile.as_str(),
                    done = done,
                    total = job.total
                );
                ui.add(
                    egui::ProgressBar::new(done as f32 / job.total.max(1) as f32)
                        .desired_width(self.gui.progress_width)
                        .fill(fill)
                        .text(text),
                )
                .on_hover_text(crate::files::tilde(&job.dest));
                if ui
                    .small_button("×")
                    .on_hover_text(t!("export-cancel"))
                    .clicked()
                {
                    job.cancel();
                }
            }
            // Nom de l'image en plein écran seulement : en grille, il est sous la vignette
            // (retour du 21/10/2026 : sans intérêt, plusieurs images pouvant être choisies).
            if self.mode != Mode::Grid
                && let Some(file) = self.files.get(self.selected)
            {
                ui.separator();
                ui.label(file_name(&file.path));
                // Plein écran : numéro, source, taille, attentes (écrits par la vue).
                if self.mode == Mode::View && !self.view_info.is_empty() {
                    ui.weak(&self.view_info);
                }
            }
            // Mode pile : où l'on est dans la pile.
            if self.mode == Mode::View
                && self.stacking
                && let Some(r) = self.stacks.range_of(self.selected)
                && r.len() > 1
            {
                ui.separator();
                ui.colored_label(
                    Color32::from(self.gui.dialogs.title_color),
                    t!(
                        "stack-position",
                        n = self.selected - r.start + 1,
                        total = r.len()
                    ),
                );
            }
            // Panneau qui a le clavier (façon tmux) : dit, puisqu'il change l'effet des
            // touches.
            let focus = match self.focus {
                crate::actions::Scope::Folders => Some(t!("focus-folders")),
                crate::actions::Scope::Metadata => Some(t!("focus-metadata")),
                crate::actions::Scope::Images
                | crate::actions::Scope::Histogram
                | crate::actions::Scope::Crop => None,
            };
            if let Some(focus) = focus {
                ui.separator();
                ui.colored_label(Color32::from(self.gui.dialogs.title_color), focus);
            }
            if let Some((text, error)) = &self.notice {
                ui.separator();
                // Bilan d'un export : le rapport, à un clic (ou Ctrl+Maj+E) ; avant le texte,
                // qu'un long chemin ne le pousse pas hors de la fenêtre.
                if self.export_notice.as_ref() == Some(text)
                    && ui
                        .small_button(t!("export-report-open"))
                        .on_hover_text(self.action_tip(Action::ExportReport))
                        .clicked()
                {
                    clicked = Some(StatusClick::Action(Action::ExportReport));
                }
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

    /// Photo d'analyse dans un bloc carré de côté `side` (la hauteur de l'histogramme, à
    /// sa droite). Sans légende (retour du 19/10/2026 : les couleurs parlent d'elles-mêmes,
    /// et le carré seul est plus élégant).
    fn analysis_block(&self, ui: &mut Ui, texture: &TextureHandle, side: f32) {
        let settings = &self.gui.histogram;
        let margin = self.gui.dialogs.spacing.max(4.0);
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        let painter = ui.painter();
        let fill = ui.visuals().window_fill;
        let k = settings.analysis_lighten.clamp(0.0, 1.0);
        let lighten = |v: u8| (v as f32 + (255.0 - v as f32) * k) as u8;
        painter.rect_filled(
            rect,
            4.0,
            Color32::from_rgb(lighten(fill.r()), lighten(fill.g()), lighten(fill.b())),
        );
        let room = rect.shrink(margin);
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
        let mut toggle_photo = false;
        let modal = dialog.show(ctx, |ui| {
            // Photo d'analyse à droite de l'histogramme, à sa hauteur (retour du
            // 19/10/2026 : boîte plus compacte) ; sans elle, l'histogramme prend toute la
            // largeur.
            let spacing = self.gui.dialogs.spacing;
            let photo = analysis
                .as_ref()
                .filter(|_| self.histogram_photo && exposure.is_some());
            let block = photo.map_or(0.0, |_| size.y + spacing);
            let row = ui.horizontal_top(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(size.x - block, size.y), Sense::hover());
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
                        let direct = [
                            Action::HistogramRed,
                            Action::HistogramGreen,
                            Action::HistogramBlue,
                            Action::HistogramAll,
                        ];
                        // Échelle : icône à gauche des pastilles, qui montre celle en cours.
                        let button = histogram::scale_button(rect, self.gui.histogram.channel_dot);
                        let (current, other) = match self.histogram_scale {
                            HistogramScale::Ev => {
                                (t!("histogram-scale-ev"), t!("histogram-scale-linear"))
                            }
                            HistogramScale::Linear => {
                                (t!("histogram-scale-linear"), t!("histogram-scale-ev"))
                            }
                        };
                        let tip = t!(
                            "histogram-scale-tip",
                            current = current,
                            other = other,
                            keys = self.keymap.keys_for(Action::HistogramScale).join(" / ")
                        );
                        let response = ui
                            .interact(button, egui::Id::unique("histogram_scale"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tip);
                        let color = if response.hovered() {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        histogram::scale_icon(ui.painter(), button, self.histogram_scale, color);
                        if response.clicked() {
                            scale = true;
                        }
                        // Photo d'analyse : icône à gauche de l'échelle, en couleur d'accent
                        // quand elle est montrée.
                        let button = histogram::photo_button(rect, self.gui.histogram.channel_dot);
                        let key = self.keymap.keys_for(Action::HistogramPhoto).join(" / ");
                        let response = ui
                            .interact(button, egui::Id::unique("histogram_photo"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(t!("histogram-photo-tip", keys = key));
                        let color = if self.histogram_photo {
                            ui.visuals().selection.stroke.color
                        } else if response.hovered() {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        histogram::photo_icon(ui.painter(), button, color);
                        if response.clicked() {
                            toggle_photo = true;
                        }
                        for (i, (dot, name)) in dots.into_iter().zip(names).enumerate() {
                            let tip = t!(
                                "histogram-channel-tip",
                                channel = name,
                                key = self.keymap.keys_for(direct[i]).join(" / "),
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
                if let Some(texture) = photo {
                    ui.add_space(spacing);
                    self.analysis_block(ui, texture, size.y);
                }
            });
            let _ = row;
            if let Some(e) = &exposure {
                // Statistiques sous l'histogramme, sur toute la largeur.
                self.histogram_stats(ui, e);
            }
            ui.horizontal(|ui| {
                close = ui.button(t!("close")).clicked();
                ui.weak(t!("or-escape"));
                // Sur des données RAW, pas de luminance : chaque photosite ne mesure qu'une
                // couleur ; le vert, qui en porte l'essentiel, en tient lieu. Sur la ligne de
                // « Fermer », à droite (retour du 19/10/2026) : sous le tableau, elle
                // changeait la hauteur de la boîte en passant d'un RAW à une image plate.
                if exposure
                    .as_ref()
                    .is_some_and(|e| matches!(e.source, tri::exposure::Source::Raw { .. }))
                {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(t!("stats-raw-luminance-short"))
                            .on_hover_text(t!("stats-raw-luminance"));
                    });
                }
            });
        });
        if scale {
            self.histogram_scale = self.histogram_scale.toggled();
        }
        if let Some(channel) = channel {
            self.histogram_channel = channel;
        }
        if toggle_photo {
            self.histogram_photo = !self.histogram_photo;
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
            Action::Send(_) => {
                let Some(sender) = action.sender() else {
                    return;
                };
                let n = paths.len();
                match sender.send(&paths) {
                    Ok(()) => {
                        let name = sender.name();
                        self.notice = Some((t!("send-started", name = name, n = n), false));
                        Ok(())
                    }
                    Err(e) => {
                        self.file_error = Some(format!("{} : {e}", sender.name()));
                        return;
                    }
                }
            }
            Action::FileExport if self.shown_version().is_some() => {
                // Version montrée : elle seule, sur l'image où elle a été recadrée, avec son
                // profil s'il en a un.
                let Some((path, version)) = self.shown_version() else {
                    return;
                };
                let darktable =
                    tri::develop::find_darktable(&self.settings.develop.darktable).is_some();
                let source = match version.source {
                    tri::versions::Source::Embedded => Source::Embedded,
                    tri::versions::Source::Raw => Source::Raw,
                    tri::versions::Source::Developed => Source::Developed,
                };
                let render = crate::export::Render::of(Some(source), darktable);
                let images = vec![path];
                let flat = crate::export::exports_flat(render, &images);
                // Son profil, sinon le dernier choisi, sinon celui des réglages.
                let preferred = version
                    .profile
                    .clone()
                    .or_else(|| self.export_choice[usize::from(flat)].clone());
                let dialog = crate::export::ExportDialog::new(
                    &images,
                    &self.settings.export,
                    preferred.as_deref(),
                    darktable,
                    render,
                    at,
                )
                .with_cut(tri::export::Cut {
                    crop: version.crop,
                    name: version.name.clone(),
                });
                self.file_dialog = Some(crate::files::FileDialog::Export(Box::new(dialog)));
                Ok(())
            }
            Action::FileExport => {
                let darktable =
                    tri::develop::find_darktable(&self.settings.develop.darktable).is_some();
                // On exporte ce qu'on regarde : en plein écran, la source affichée.
                let render = crate::export::Render::of(
                    self.files
                        .get(self.selected)
                        .filter(|_| self.mode != Mode::Grid)
                        .map(|f| self.source_for(f)),
                    darktable,
                );
                // Un JPEG (boîtier ou d'origine) : « JPEG tel quel » proposé d'abord (retour
                // du 19/10/2026) ; le choix de la séance est retenu à part de celui des rendus.
                let jpeg = crate::export::exports_flat(render, &paths);
                // Le dernier choisi dans la séance, sinon celui des réglages
                // (`default_raw`, `default_flat`).
                let preferred = self.export_choice[usize::from(jpeg)].clone();
                let dialog = crate::export::ExportDialog::new(
                    &paths,
                    &self.settings.export,
                    preferred.as_deref(),
                    darktable,
                    render,
                    at,
                );
                self.file_dialog = Some(crate::files::FileDialog::Export(Box::new(dialog)));
                Ok(())
            }
            Action::FileCopy | Action::FileMove => {
                let mode = if action == Action::FileCopy {
                    tri::files::TransferMode::Copy
                } else {
                    tri::files::TransferMode::Move
                };
                self.file_dialog = Some(crate::files::FileDialog::transfer(
                    crate::files::Source::Images(paths),
                    mode,
                    None,
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
            crate::files::Outcome::Folder {
                step,
                point,
                renamed,
            } => {
                self.file_dialog = None;
                if let Some(step) = step {
                    self.history.push(step);
                }
                self.after_folder_change(renamed, point);
            }
            crate::files::Outcome::FavoriteNamed { path, name } => {
                self.file_dialog = None;
                let Some(data) = self.data_dir.clone() else {
                    return;
                };
                match tri::places::name_favorite(&data, &path, &name) {
                    Ok(names) => {
                        self.tree.set_favorite_names(names);
                        let shown = self
                            .tree
                            .favorite_name(&path)
                            .map_or_else(|| folder_name(&path), str::to_string);
                        self.notice = Some((t!("favorite-named", name = shown), false));
                    }
                    Err(e) => {
                        let error = e.to_string();
                        self.notice = Some((t!("folders-favorite-error", error = error), true));
                    }
                }
            }
            crate::files::Outcome::Failed(e) => {
                self.file_dialog = None;
                self.file_error = Some(e);
                // Un renommage annulé à mi-chemin est remis en place, mais une corbeille
                // partielle non : relire le dossier.
                self.reload_folder(None);
            }
            crate::files::Outcome::Export {
                images,
                profile,
                folder,
                render,
                cut,
            } => {
                self.file_dialog = None;
                self.export_choice[usize::from(crate::export::exports_flat(render, &images))] =
                    Some(profile.id.clone());
                self.start_export(ctx, images, profile, folder, render, cut);
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

    /// Lance l'export en fond (darktable) ; un seul à la fois.
    fn start_export(
        &mut self,
        ctx: &egui::Context,
        images: Vec<PathBuf>,
        profile: tri::export::Profile,
        folder: String,
        render: crate::export::Render,
        cut: Option<tri::export::Cut>,
    ) {
        if self.export_job.is_some() {
            self.notice = Some((t!("export-busy"), true));
            return;
        }
        let Some(engine) = &self.engine else { return };
        // Configuration à part de celle des rendus développés : deux darktable peuvent
        // tourner en même temps (un rendu demandé par Maj+B pendant un export). Sans
        // darktable : LibRaw (rendu de base).
        let darktable = (render == crate::export::Render::Darktable)
            .then(|| tri::develop::find_darktable(&self.settings.develop.darktable))
            .flatten();
        let local = match render {
            crate::export::Render::Embedded => tri::export::Local::Embedded,
            _ => tri::export::Local::LibRaw,
        };
        let dt = darktable.map(|cli| tri::develop::Darktable {
            cli,
            config_dir: engine.cache.root().join("darktable-export"),
            high_quality: true,
        });
        trace(|| format!("export : {} images vers {folder}", images.len()));
        let order = crate::export::Order {
            images,
            profile,
            folder,
            cut,
            sharpening: self.settings.export.sharpening,
            pair_suffix: self.settings.export.camera_suffix.clone(),
        };
        self.export_job = Some(crate::export::Job::start(dt, local, order, ctx));
    }

    /// Export fini : bilan dans la barre d'état.
    fn poll_export(&mut self) {
        let Some(job) = &mut self.export_job else {
            return;
        };
        let Some(result) = job.finished() else {
            return;
        };
        let dest = crate::files::tilde(&job.dest);
        // Au rapport d'export, gardé d'une session à l'autre.
        let entry = job.entry(&result);
        let max = self.settings.export.report_max;
        if let Some(data) = &self.data_dir
            && let Err(e) = tri::export_log::append(data, &entry, max)
        {
            self.file_error = Some(e.to_string());
        }
        self.export_log.push(entry);
        if max > 0 && self.export_log.len() > max {
            let extra = self.export_log.len() - max;
            self.export_log.drain(..extra);
        }
        self.notice = Some(match result {
            Ok(report) if report.failed == 0 => {
                let mut text = t!("export-done", n = report.written.len(), dest = dest);
                if report.passthrough > 0 {
                    text.push_str(&t!("export-done-passthrough", n = report.passthrough));
                }
                (text, false)
            }
            Ok(report) => (
                t!(
                    "export-partial",
                    n = report.written.len(),
                    failed = report.failed,
                    dest = dest
                ),
                true,
            ),
            Err(tri::develop::DevelopError::Cancelled) => (t!("export-cancelled"), false),
            Err(e) => (e.to_string(), true),
        });
        self.export_notice = self.notice.as_ref().map(|(text, _)| text.clone());
        self.export_job = None;
    }

    /// Niveaux parcourus à l'ouverture d'un dossier (1 : le dossier seul).
    fn folder_depth(&self) -> usize {
        folder_depth(self.subfolders, &self.gui)
    }

    /// Case « sous-dossiers » : retenue, et le dossier ouvert relu à la nouvelle
    /// profondeur.
    fn set_subfolders(&mut self, on: bool) {
        self.subfolders = on;
        if let Some(data) = self.settings.data_dir() {
            let _ = tri::folder::remember_subfolders(&data, on);
        }
        if !self.folder.is_none() {
            self.reload_folder(None);
        }
    }

    /// Relit le dossier ouvert après une action sur ses fichiers, en gardant la vue (grille
    /// ou plein écran, zoom) ; `select` : image à sélectionner (nouveau nom), sinon la
    /// sélection garde sa place (l'image suivante, après une suppression).
    fn reload_folder(&mut self, select: Option<PathBuf>) {
        self.versions.clear();
        match Folder::scan(
            &self.folder.dir,
            self.folder_depth(),
            self.settings.xmp.sidecar,
        ) {
            Ok(mut folder) => {
                // Les notes connues en attendant la relecture en fond.
                folder.notes.seed(&self.folder.notes);
                self.folder = folder;
                self.start_loading();
                self.apply_sort();
            }
            Err(e) => {
                self.file_error = Some(format!("{} : {e}", self.folder.dir.display()));
                return;
            }
        }
        // Dossier relu : la surveillance repart de cet état (un changement fait par tri
        // lui-même n'est pas signalé une seconde fois).
        if let Some((watch, rx, _)) = &self.watch {
            watch.sync();
            rx.try_iter().for_each(drop);
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
        let (colors, size) = (self.gui.colors.clone(), self.gui.grid.marks + 8.0);
        let mut clicked = None;
        let mut file_action = None;
        let mut close = false;
        let modal = modal_at("rating_dialog", at)
            .title(file_name(&path))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    clicked = marks::controls(ui, &notes, &colors, size, &|a| self.action_tip(a))
                });
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
                        Action::Send("plugin.airdrop.send"),
                    ]
                    .into_iter()
                    .filter(|a| a.available())
                    {
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
        let (colors, size) = (self.gui.colors.clone(), self.gui.grid.marks + 4.0);
        let marked = self.selection.visible(&self.files);
        let mut clicked = None;
        // Versions de l'image (plein écran) : pastilles, puis modifier / supprimer.
        let path = file.path.clone();
        let versions: Vec<String> = if self.mode == Mode::View {
            self.versions_of(&path)
                .list
                .iter()
                .map(|v| v.name.clone())
                .collect()
        } else {
            Vec::new()
        };
        let shown = self
            .version_shown
            .as_ref()
            .filter(|(p, _)| *p == path)
            .map(|(_, i)| *i);
        let mut version_action: Option<VersionAction> = None;
        // Plein écran : écrêtage et flou de l'image, comme sur les vignettes (issue n° 9).
        let (clip, blurry) = if self.mode == Mode::View {
            let index = self.selected;
            let deadline = Instant::now() + Duration::from_millis(5);
            (self.clip_fractions(index, deadline), self.is_blurry(&path))
        } else {
            (None, false)
        };
        let histogram = self.gui.histogram;
        ui.horizontal(|ui| {
            ui.label(t!("rating-bar-title"));
            clicked = marks::controls(ui, &notes, &colors, size, &|a| self.action_tip(a));

            // Versions : une liste sur la ligne des notes (issue n° 5 : sur une ligne à
            // elles, elles apparaissaient selon l'image et changeaient sa taille).
            if !versions.is_empty() {
                ui.separator();
                let current = shown
                    .and_then(|i| versions.get(i).cloned())
                    .unwrap_or_else(|| t!("versions-original"));
                egui::ComboBox::from_id_salt("versions")
                    .selected_text(t!("versions-current", name = current))
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(shown.is_none(), t!("versions-original"))
                            .clicked()
                        {
                            version_action = Some(VersionAction::Show(None));
                        }
                        for (i, name) in versions.iter().enumerate() {
                            if ui.selectable_label(shown == Some(i), name).clicked() {
                                version_action = Some(VersionAction::Show(Some(i)));
                            }
                        }
                        if let Some(i) = shown {
                            ui.separator();
                            if ui.button(t!("versions-edit")).clicked() {
                                version_action = Some(VersionAction::Edit(i));
                            }
                            if ui.button(t!("versions-delete")).clicked() {
                                version_action = Some(VersionAction::Delete(i));
                            }
                        }
                    });
            }
            // Brûlée, bouchée (clic : grand histogramme), floue (clic : carte de netteté).
            let mut badges: Vec<(String, Option<bool>, Action)> = Vec::new();
            if let Some((over, under)) = clip {
                for (fraction, threshold, what, is_over) in [
                    (over, histogram.badge_over, "badge-over", true),
                    (under, histogram.badge_under, "badge-under", false),
                ] {
                    if fraction > threshold {
                        let percent = tri::i18n::decimal((fraction * 1000.0).round() / 10.0, 1);
                        let tip = t!(what, percent = percent);
                        badges.push((tip, Some(is_over), Action::HistogramLarge));
                    }
                }
            }
            if blurry {
                badges.push((t!("badge-blurry-view"), None, Action::TogglePeaking));
            }
            if !badges.is_empty() {
                ui.separator();
            }
            for (tip, over, action) in badges {
                let (rect, response) = marks::clickable(ui, size, &tip);
                match over {
                    Some(is_over) => marks::clip_badge(ui.painter(), rect, is_over),
                    None => {
                        let color = ui.visuals().text_color();
                        marks::blur_drop_outline(ui.painter(), rect, color);
                    }
                }
                if response.clicked() {
                    clicked = Some(action);
                }
            }

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
                // Le bouton 1:1 allumé à 100 % seulement (pas à 13 % ni à 200 %).
                let zoomed = self.mode != Mode::Grid
                    && self.zoom.is_some()
                    && (self.zoom_scale - 1.0).abs() < 1e-3;
                // Plein écran : l'interrupteur des piles, toujours là ; piles actives, ↑ / ↓
                // (pile d'avant, d'après) à côté de ‹ ›.
                let mut actions = vec![Action::ToggleAutoAdvance, Action::NavNext, Action::NavPrev];
                if self.mode == Mode::View {
                    if self.stacking {
                        actions.extend([Action::NavDown, Action::NavUp]);
                    }
                    actions.push(Action::StackMode);
                }
                // Développer dans darktable (si darktable est là), à côté du 1:1.
                if self.mode == Mode::View && self.darktable {
                    actions.push(Action::Develop);
                }
                actions.push(Action::ToggleZoom);
                for action in actions {
                    let (rect, response) =
                        marks::icon_button(ui, icon, &action.label(), &self.action_tip(action));
                    let active = match action {
                        Action::ToggleAutoAdvance => self.auto_advance,
                        Action::ToggleZoom => zoomed,
                        Action::StackMode => self.stacking,
                        Action::Develop => self.source == Source::Developed,
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
                // Grille : infos des vignettes, tout à l'écran et taille des vignettes, à
                // côté de ‹ › (demandé le 21/10/2026). Seulement s'il reste la place : sur
                // une fenêtre étroite, ils recouvriraient les notes (le clavier reste).
                let spacing = ui.spacing().item_spacing.x;
                let buttons = 2.0 * (icon + spacing) + 2.0 * spacing;
                let room = ui.available_width() - self.gui.dialogs.spacing;
                if self.mode == Mode::Grid && room >= buttons {
                    ui.separator();
                    let tip = self.action_tip(Action::ToggleGridInfo);
                    let (rect, response) =
                        marks::icon_button(ui, icon, &Action::ToggleGridInfo.label(), &tip);
                    if self.grid_info {
                        ui.painter()
                            .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                    }
                    let color = ui.style().interact(&response).fg_stroke.color;
                    marks::grid_info_icon(ui.painter(), rect.shrink(icon * 0.2), color);
                    if response.clicked() {
                        clicked = Some(Action::ToggleGridInfo);
                    }
                    let keys = self.keymap.keys_for(Action::Zoom100).join(" / ");
                    let tip = format!("{} ({keys})", t!("grid-fit"));
                    let (rect, response) = marks::icon_button(ui, icon, &t!("grid-fit"), &tip);
                    let color = ui.style().interact(&response).fg_stroke.color;
                    marks::grid_fit_icon(ui.painter(), rect.shrink(icon * 0.25), color);
                    if response.clicked() {
                        self.grid_fit_all();
                    }
                    if room >= buttons + self.gui.grid.size_slider + spacing {
                        self.grid_size_slider(ui);
                    }
                }
                // Carte de netteté affichée : son curseur de sévérité (le même que celui de
                // la barre des filtres), précédé du triangle de netteté (posés de droite à
                // gauche).
                if self.mode == Mode::View && self.peaking {
                    ui.separator();
                    self.blur_slider(ui);
                    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
                    marks::sharp_triangle(ui.painter(), rect, Color32::from(colors.blur_drop));
                }
            });
        });
        match clicked {
            Some(Action::ToggleAutoAdvance) => self.auto_advance = !self.auto_advance,
            Some(
                action @ (Action::NavPrev
                | Action::NavNext
                | Action::NavUp
                | Action::NavDown
                | Action::StackMode
                | Action::Develop
                | Action::ToggleZoom
                | Action::ToggleGridInfo
                | Action::HistogramLarge
                | Action::TogglePeaking),
            ) => {
                let ctx = ui.ctx().clone();
                self.perform(&ctx, action);
            }
            Some(action) => self.rate_selected(action),
            None => {}
        }
        match version_action {
            Some(VersionAction::Show(i)) => {
                self.version_shown = i.map(|i| (path.clone(), i));
                self.zoom = None;
            }
            Some(VersionAction::Edit(i)) => self.edit_version(&path, i),
            Some(VersionAction::Delete(i)) => self.delete_version(&path, i),
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
        if self.blur_rule.is_some() {
            parts.push(t!("filter-bar-no-blurry"));
        }
        if self.well_exposed {
            parts.push(t!("filter-bar-well-exposed"));
        }
        if self.filter_inverted && self.content_filtered() {
            parts.push(t!("filter-bar-inverted"));
        }
        if !self.name_query.trim().is_empty() {
            parts.push(t!("filter-summary-name", query = self.name_query.trim()));
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
        let mut refilter = false;
        let mut sort_choice: Option<tri::sort::SortKey> = None;
        let colors = self.gui.colors.clone();
        // Passe à la ligne si la fenêtre est étroite (panneau des dossiers ouvert).
        // Icônes et textes courts, détail au survol (retour du 17/10/2026 : sur un écran
        // de portable, la barre débordait).
        ui.horizontal_wrapped(|ui| {
            // Deux interrupteurs, RAW et JPEG (retour du 21/10/2026, à la place de « Tout »,
            // « RAW », « Plates ») : allumé, ce type est montré ; un des deux reste allumé.
            let (raw, jpeg) = match self.filter {
                KindFilter::All => (true, true),
                KindFilter::RawOnly => (true, false),
                KindFilter::FlatOnly => (false, true),
            };
            let height = self.gui.grid.marks + 10.0;
            for (is_raw, on, label, tip) in [
                (true, raw, "RAW", t!("filter-bar-raw-tip")),
                (false, jpeg, "JPG", t!("filter-bar-flat-tip")),
            ] {
                let tip = format!("{tip}\n{}", self.action_tip(Action::FilterCycle));
                let (rect, response) =
                    ui.allocate_exact_size(vec2(height * 1.3, height), Sense::click());
                let name = if is_raw { "RAW" } else { "JPEG" };
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::accesskit::Role::Button, true, on, name)
                });
                let response = response
                    .on_hover_text(tip)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.hovered() {
                    let fill = ui.visuals().widgets.hovered.weak_bg_fill;
                    ui.painter().rect_filled(rect.expand(2.0), 4.0, fill);
                }
                // Allumé : le rectangle seul prend l'accent (retour du 21/10/2026 : pas de
                // grand carré autour), texte de la sélection d'egui, lisible dessus.
                let (color, back) = if on {
                    (marks::accent(ui.ctx()), ui.visuals().selection.stroke.color)
                } else {
                    (ui.visuals().weak_text_color(), ui.visuals().panel_fill)
                };
                marks::file_type(ui.painter(), rect.shrink(1.0), label, color, back);
                if response.clicked() {
                    let (raw, jpeg) = if is_raw { (!raw, jpeg) } else { (raw, !jpeg) };
                    let kind = match (raw, jpeg) {
                        (true, true) => Some(KindFilter::All),
                        (true, false) => Some(KindFilter::RawOnly),
                        (false, true) => Some(KindFilter::FlatOnly),
                        // Les deux éteints : rien à montrer, refusé.
                        (false, false) => None,
                    };
                    if let Some(kind) = kind {
                        // Le type tourne en boucle (même action que le clavier) : autant de
                        // crans que nécessaire.
                        let order = [KindFilter::All, KindFilter::RawOnly, KindFilter::FlatOnly];
                        let from = order.iter().position(|&k| k == self.filter).unwrap_or(0);
                        let to = order.iter().position(|&k| k == kind).unwrap_or(0);
                        for _ in 0..(to + 3 - from) % 3 {
                            clicked.push(Action::FilterCycle);
                        }
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
                    marks::star_color(ui.ctx(), &colors)
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
            // Triangle (nettes) et soleil (bien exposées), à l'accent quand ils sont
            // allumés, gris sinon, comme les étoiles (retour du 21/10/2026 ; la goutte
            // « floues seulement » est devenue « nettes » inversé).
            for (action, on) in [
                (Action::FilterBlurry, self.blur_rule.is_some()),
                (Action::FilterWellExposed, self.well_exposed),
            ] {
                let tip = self.action_tip(action);
                let (rect, response) = marks::clickable(ui, size, &tip);
                let color = if on {
                    marks::accent_mark(ui.ctx())
                } else {
                    ui.visuals().weak_text_color()
                };
                if action == Action::FilterBlurry {
                    marks::sharp_triangle(ui.painter(), rect, color);
                } else {
                    marks::sun(ui.painter(), rect, color);
                }
                if response.clicked() {
                    clicked.push(action);
                }
            }
            // Seulement avec un filtre sur la netteté, comme celui des piles avec les piles.
            if self.blur_rule.is_some() {
                self.blur_slider(ui);
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
                        egui::Stroke::new(2.0, marks::accent_mark(ui.ctx())),
                    );
                }
                if response.clicked() {
                    clicked.push(Action::FilterColor(color));
                }
            }
            // Piles : curseur de similarité (retour du 18/10/2026, comme Capture One) ;
            // pendant le réglage, les piles se déplient sur des fonds alternés.
            // L'interrupteur des piles, toujours là (fond d'accent quand elles sont
            // actives, comme en plein écran) ; clic droit : les réglages fins.
            ui.separator();
            let size = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
            let tip = format!(
                "{}\n{}",
                self.action_tip(Action::StackMode),
                t!(
                    "stack-icon-tip",
                    keys = self.keymap.keys_for(Action::GroupingWindow).join(" / ")
                )
            );
            let (rect, icon) = marks::icon_button(ui, size, &Action::StackMode.label(), &tip);
            if self.stacking {
                ui.painter()
                    .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
            }
            let color = ui.style().interact(&icon).fg_stroke.color;
            marks::bar_icon(
                ui.painter(),
                rect.shrink(size * 0.2),
                Action::StackMode,
                color,
                &colors,
            );
            if icon.clicked() {
                clicked.push(Action::StackMode);
            }
            if icon.secondary_clicked() {
                clicked.push(Action::GroupingWindow);
            }
            if self.stacking {
                // Même logique que le flou (idée de l'utilisateur, 20/10/2026) : centre
                // calculé d'après le dossier, à droite plus strict (petites piles).
                let (slider, t) = strictness_slider(
                    ui,
                    self.gui.grid.stack_slider,
                    self.group_strictness,
                    t!(
                        "stack-slider-tip",
                        keys = self.keymap.keys_for(Action::GroupingWindow).join(" / ")
                    ),
                );
                if t != self.group_strictness {
                    self.group_strictness = t;
                    self.stacks_dirty = true;
                }
                // Piles dépliées pendant le réglage seulement si Maj est tenue (retour du
                // 20/10/2026 : repliées, on voit directement les piles obtenues).
                let shift = ui.input(|i| i.modifiers.shift);
                let adjusting = (slider.dragged() || slider.has_focus()) && shift;
                if adjusting != self.grouping_preview {
                    self.grouping_preview = adjusting;
                    self.stacks_dirty = true;
                }
                if slider.drag_stopped() || slider.double_clicked_slider(ui) {
                    self.remember_group_strictness();
                }
                // Tout déplier / tout replier (Maj+S), à côté du curseur.
                let (rect, all) = marks::icon_button(
                    ui,
                    size,
                    &Action::StacksAll.label(),
                    &self.action_tip(Action::StacksAll),
                );
                // Allumé quand les piles sont repliées (retour du 18/10/2026 : « dans
                // l'autre sens »).
                if self.expanded.is_empty() {
                    ui.painter()
                        .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                }
                let color = ui.style().interact(&all).fg_stroke.color;
                marks::bar_icon(
                    ui.painter(),
                    rect.shrink(size * 0.2),
                    Action::StacksAll,
                    color,
                    &colors,
                );
                if all.clicked() {
                    clicked.push(Action::StacksAll);
                }
            }
            ui.separator();
            let content = self.content_filtered();
            let any = self.filter != KindFilter::All
                || content
                || self.filter_inverted
                || !self.name_query.trim().is_empty();
            // Recherche par nom (retour du 19/10/2026) : filtre en direct, à la fzf.
            ui.add_space(self.gui.dialogs.spacing);
            let hint = t!("search-grid-hint", keys = self.search_keys());
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.name_query)
                    .id(grid_search_id())
                    .hint_text(hint)
                    .desired_width(self.gui.grid.search_width),
            );
            if self.focus_request == Some(grid_search_id()) {
                field.request_focus();
                self.focus_request = None;
            }
            if field.changed() {
                refilter = true;
            }
            if field.has_focus() {
                self.search_focused = Some(grid_search_id());
            }
            // Tout à droite (retour du 21/10/2026) : effacer les filtres (entonnoir barré,
            // grisé sans filtre), et juste à sa gauche le nombre d'images.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = self.action_tip(Action::FilterClear);
                ui.add_enabled_ui(any, |ui| {
                    let (rect, response) =
                        marks::icon_button(ui, size, &t!("filter-bar-clear"), &tip);
                    let color = ui.style().interact(&response).fg_stroke.color;
                    marks::funnel_cleared(ui.painter(), rect.shrink(size * 0.18), color);
                    if response.clicked() {
                        clicked.push(Action::FilterClear);
                    }
                });
                // Inverser les filtres sur le contenu : grisé sans eux ; allumé, à
                // l'accent.
                let tip = self.action_tip(Action::FilterInvert);
                ui.add_enabled_ui(content, |ui| {
                    let (rect, response) = marks::clickable(ui, size, &tip);
                    let color = if self.filter_inverted && content {
                        marks::accent_mark(ui.ctx())
                    } else {
                        ui.style().interact(&response).fg_stroke.color
                    };
                    // À la taille des pastilles de couleur (retour du 21/10/2026 : plus
                    // grande, elle attirait l'œil).
                    let dot = self.gui.grid.marks + 4.0;
                    let icon = Rect::from_center_size(rect.center(), vec2(dot, dot));
                    marks::invert_icon(ui.painter(), icon, color);
                    if response.clicked() {
                        clicked.push(Action::FilterInvert);
                    }
                });
                ui.weak(t!(
                    "filter-bar-count",
                    shown = self.files.len(),
                    n = self.folder.files.len()
                ))
                .on_hover_text(t!("filter-bar-count-tip"));
                // Ordre : sens, puis la clé (posés de droite à gauche).
                ui.separator();
                let (rect, response) =
                    marks::clickable(ui, size, &self.action_tip(Action::SortReverse));
                let color = ui.style().interact(&response).fg_stroke.color;
                marks::sort_arrow(
                    ui.painter(),
                    rect.shrink(size * 0.1),
                    self.order.descending,
                    color,
                );
                if response.clicked() {
                    clicked.push(Action::SortReverse);
                }
                let mut key = self.order.key;
                egui::ComboBox::from_id_salt("sort")
                    .selected_text(sort_name(key))
                    .show_ui(ui, |ui| {
                        for k in tri::sort::SortKey::ALL {
                            ui.selectable_value(&mut key, k, sort_name(k));
                        }
                    })
                    .response
                    .on_hover_text(self.action_tip(Action::SortNext));
                if key != self.order.key {
                    sort_choice = Some(key);
                }
            });
        });
        if refilter {
            self.refilter(Reveal::Minimal);
        }
        if let Some(key) = sort_choice {
            self.set_order(tri::sort::Order { key, ..self.order });
        }
        let ctx = ui.ctx().clone();
        for action in clicked {
            self.perform(&ctx, action);
        }
    }

    /// Vignettes à la plus grande taille qui les montre toutes à l'écran (bouton du bas,
    /// `*` en grille), dans les bornes du curseur.
    fn grid_fit_all(&mut self) {
        let g = self.gui.grid;
        let n = if self.stacks.shown.is_empty() {
            self.files.len()
        } else {
            self.stacks.shown.len()
        };
        self.grid_fit_pending = self.grid_width <= 0.0 || self.view_height <= 0.0;
        if self.grid_fit_pending {
            return;
        }
        self.grid_cell = grid::fit_all(
            n,
            self.grid_width,
            self.view_height,
            g.gap,
            g.cell_min,
            g.cell_max,
        );
        self.reveal = Some(Reveal::Top);
    }

    fn grid(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        #[cfg(test)]
        self.cell_rects.clear();
        self.hovered = None;
        self.over_cell = false;
        let g = self.gui.grid;
        let cell = self.grid_cell;
        self.grid_width = ui.available_width();
        self.layout = Layout::fit(ui.available_width(), cell, g.gap);
        if self.stacks_dirty || self.stacks.shown.iter().any(|&i| i >= self.files.len()) {
            self.regroup();
        }
        // Vue de la grille : couvertures des piles repliées, contenu des piles dépliées.
        let shown: Vec<usize> = if self.stacks.shown.is_empty() {
            (0..self.files.len()).collect()
        } else {
            self.stacks.shown.clone()
        };
        let rows = self.layout.rows(shown.len());
        ui.spacing_mut().item_spacing = vec2(g.gap, g.gap);
        let mut area = ScrollArea::vertical().auto_shrink(false);
        let row = shown.iter().position(|&i| i == self.selected).unwrap_or(0) / self.layout.columns;
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
        let output = area.show_rows(ui, cell, rows, |ui, visible| {
            for row in visible {
                ui.horizontal(|ui| {
                    ui.add_space((margin - g.gap).max(0.0));
                    let start = row * self.layout.columns;
                    let end = (start + self.layout.columns).min(shown.len());
                    for &index in &shown[start..end] {
                        self.cell(ui, &ctx, index, deadline);
                    }
                });
            }
        });
        self.scroll_offset = output.state.clamped_offset().y;
        self.view_height = output.inner_rect.height();
        if self.grid_fit_pending {
            self.grid_fit_all();
        }
        // Badge cliqué : l'image est choisie, puis recadrage ou grand histogramme.
        if let Some((index, action)) = self.cell_action.take() {
            if !self.selection.is_visual() {
                self.selection.clear();
            }
            self.select(index);
            match action {
                Action::HistogramLarge => self.histogram_large = true,
                _ => self.perform(&ctx, action),
            }
        }
        // Clic dans le vide (entre ou après les vignettes) : curseur masqué et sélection
        // vidée, comme dans un gestionnaire de fichiers.
        // Seulement si le clic tombe sur la grille elle-même, pas sur une boîte ou une liste
        // déroulante posée dessus (retour du 19/10/2026 : choisir un profil d'export à la
        // souris vidait la sélection).
        let layer = ui.layer_id();
        let clicked_empty = ui.input(|i| {
            i.pointer.primary_clicked()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| output.inner_rect.contains(p))
        }) && ui
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| ui.ctx().layer_id_at(p) == Some(layer));
        if clicked_empty && !self.over_cell && !self.selection.is_visual() {
            self.cursor_hidden = true;
            self.selection.clear();
        }
    }

    fn cell(&mut self, ui: &mut Ui, ctx: &egui::Context, index: usize, deadline: Instant) {
        let (g, colors) = (self.gui.grid, self.gui.colors.clone());
        let cell = self.grid_cell;
        let (rect, response) = ui.allocate_exact_size(vec2(cell, cell), Sense::click_and_drag());
        if response.contains_pointer() {
            self.over_cell = true;
        }
        // Glisser vers un dossier du panneau : l'image, ou toute la sélection si elle en
        // fait partie.
        if response.drag_started() {
            let path = self.files[index].path.clone();
            let images = if self.selection.contains(&path) {
                self.selection.targets(&self.files, index)
            } else {
                vec![path]
            };
            response.dnd_set_drag_payload(Dragged::Images(images));
        }
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
        let selected = index == self.selected && !self.cursor_hidden;
        let visuals = ui.visuals();
        let background = if selected {
            self.gui.theme.selection(&colors, visuals)
        } else if let Some(group) = self
            .stacks
            .group_index(index)
            .filter(|_| self.grouping_preview || self.stacking)
        {
            // Piles actives : deux gris en alternance d'un groupe au suivant, une photo
            // isolée formant un groupe d'une image (retours du 18/10/2026 : cadres et
            // traits ne se lisaient pas ; « pourquoi 3 cas quand 2 suffisent »).
            let (band, alt) = self.gui.theme.stack_bands(&colors);
            if group % 2 == 1 { band } else { alt }
        } else {
            self.gui.theme.cell(&colors, visuals)
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

        // Pile repliée : des feuilles derrière la vignette, et le nombre d'images (pile
        // dépliée : son fond, plus haut).
        let accent = Color32::from(colors.stack_mark);
        if let Some(count) = self.stacks.collapsed_count(index) {
            for k in 1..=2 {
                let o = k as f32 * 3.0;
                painter.rect_stroke(
                    rect.translate(vec2(o, -o)).shrink(1.0),
                    4.0,
                    egui::Stroke::new(1.0, accent.gamma_multiply(1.0 / k as f32)),
                    egui::StrokeKind::Inside,
                );
            }
            let label = painter.layout_no_wrap(
                count.to_string(),
                FontId::proportional(12.0),
                Color32::BLACK,
            );
            let at = rect.min + vec2(g.padding + 2.0, g.padding + 2.0);
            let back = Rect::from_min_size(at, label.size()).expand2(vec2(5.0, 2.0));
            painter.rect_filled(back, 6.0, accent);
            painter.galley(at, label, Color32::BLACK);
        }
        // Infos masquées (Maj+T) : la vignette prend aussi la place du texte.
        let info = self.grid_info;
        let bottom = if info { g.label + 4.0 } else { g.padding };
        let image_area = Rect::from_min_max(
            rect.min + vec2(g.padding, g.padding),
            pos2(rect.max.x - g.padding, rect.max.y - bottom),
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
                let mark = Rect::from_center_size(image_area.center(), vec2(32.0, 32.0));
                marks::warning(ui.painter(), mark, colors.warning.into());
                response.clone().on_hover_text(error.clone());
            }
            (None, _) => {} // en attente : cellule vide
        }

        if !info {
            return;
        }
        let text_color = if selected {
            self.gui
                .theme
                .selected_text(&colors, visuals_text(ui, true))
        } else {
            self.gui.theme.cell_text(&colors, visuals_text(ui, false))
        };
        // Sous la vignette, tout de la couleur du texte de la cellule (retour du
        // 21/10/2026) : le nom sans extension à gauche, le format à droite (il remplace
        // l'extension), la goutte des photos floues juste avant. Le nom est écrit en
        // dernier, raccourci à la place laissée par les badges.
        let badge_rect = ui.painter().text(
            pos2(rect.max.x - g.padding, rect.max.y - 4.0),
            Align2::RIGHT_BOTTOM,
            format_label(&file.path, file.kind),
            FontId::monospace(11.0),
            match (colors.badge, selected) {
                (Some(c), false) => c.into(),
                _ => text_color,
            },
        );
        let mut tips = Vec::new();
        // De droite à gauche après le format : goutte (floue), puis versions.
        let mut caption_left = badge_rect.min.x;
        if g.blur_badge && self.is_blurry(&file.path) {
            let side = badge_rect.height();
            let drop = Rect::from_min_size(
                pos2(caption_left - 4.0 - side, badge_rect.min.y),
                vec2(side, side),
            );
            marks::blur_drop_outline(ui.painter(), drop, text_color);
            caption_left = drop.min.x;
            tips.push(t!("badge-blurry"));
        }
        // Versions (recadrages) dans la légende, sans fond (retour du 21/10/2026) : icône
        // de recadrage et leur nombre ; un clic passe en recadrage.
        let versions = self.versions_of(&file.path).list.len();
        if versions > 0 {
            let side = badge_rect.height();
            let painter = ui.painter();
            let galley = painter.layout_no_wrap(
                versions.to_string(),
                FontId::proportional(11.0),
                text_color,
            );
            let number_x = caption_left - 6.0 - galley.size().x;
            let icon = Rect::from_min_size(
                pos2(number_x - 2.0 - side, badge_rect.min.y),
                vec2(side, side),
            );
            painter.galley(
                pos2(number_x, icon.center().y - galley.size().y / 2.0),
                galley,
                text_color,
            );
            marks::crop_icon(painter, icon.shrink(side * 0.1), text_color);
            let area = Rect::from_min_max(icon.min, pos2(caption_left - 4.0, icon.max.y));
            let tip = t!("badge-versions", n = versions);
            let badge = ui.interact(
                area.expand(2.0),
                response.id.with("versions"),
                Sense::click(),
            );
            badge.widget_info(|| {
                egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, &tip)
            });
            let clicked = badge
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(&tip)
                .clicked();
            if clicked {
                self.cell_action = Some((index, Action::Crop));
            }
            caption_left = icon.min.x;
        }
        let name_x = rect.min.x + g.padding;
        let mut name = truncate(&file_stem(&file.path), g.name_chars);
        let font = FontId::proportional(12.0);
        let room = caption_left - 6.0 - name_x;
        while name.chars().count() > 1
            && ui
                .painter()
                .layout_no_wrap(name.clone(), font.clone(), text_color)
                .size()
                .x
                > room
        {
            let keep = name.trim_end_matches('…').chars().count().saturating_sub(1);
            name = name.chars().take(keep).collect::<String>() + "…";
        }
        ui.painter().text(
            pos2(name_x, rect.max.y - 4.0),
            Align2::LEFT_BOTTOM,
            name,
            font,
            text_color,
        );
        // Notes sur la vignette : étoiles en bas à gauche, drapeau en bas à droite,
        // couleurs en haut à droite, chaque marque cerclée de sombre.
        let notes = self.folder.notes.notes(&file.path);
        marks::thumbnail_notes(ui.painter(), image_area, notes, g.marks, &colors);
        // Écrêtage en haut à gauche : triangles mi-blanc mi-noir (retour du 21/10/2026 :
        // les petits histogrammes ne se lisaient pas) ; un clic ouvre le grand histogramme.
        if let Some((over, under)) = self.clip_fractions(index, deadline) {
            let h = self.gui.histogram;
            let side = g.marks * 1.3;
            let mut at = image_area.min + vec2(4.0, 4.0);
            for (fraction, threshold, what, is_over) in [
                (over, h.badge_over, "badge-over", true),
                (under, h.badge_under, "badge-under", false),
            ] {
                if fraction > threshold {
                    let badge = Rect::from_min_size(at, vec2(side, side));
                    marks::clip_badge(ui.painter(), badge, is_over);
                    at.x += side + 4.0;
                    let percent = tri::i18n::decimal((fraction * 1000.0).round() / 10.0, 1);
                    let tip = t!(what, percent = percent);
                    let badge = ui.interact(badge, response.id.with(what), Sense::click());
                    badge.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, &tip)
                    });
                    let clicked = badge
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(&tip)
                        .clicked();
                    if clicked {
                        self.cell_action = Some((index, Action::HistogramLarge));
                    }
                }
            }
        }
        if !tips.is_empty() {
            response.on_hover_text(tips.join("\n"));
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
                Step::Settings(section) => {
                    self.open_settings();
                    if let Some(window) = &mut self.settings_window {
                        window.show_section(&section);
                    }
                }
                Step::Search(text) => {
                    if let Some(window) = &mut self.settings_window {
                        window.search(&text);
                    } else if self.help {
                        self.help_search = Some(text);
                    }
                }
            }
        }
    }

    /// Panneau qui peut avoir le clavier : les dossiers seulement s'ils sont affichés (grille),
    /// les métadonnées aussi ; sinon les images.
    fn effective_focus(&self) -> crate::actions::Scope {
        use crate::actions::Scope;
        // Grand histogramme ouvert : r, g, b, a choisissent le canal.
        // Recadrage : le cadre prend les touches (flèches ou hjkl, rapports, x, Entrée).
        if self.crop.is_some() {
            return Scope::Crop;
        }
        if self.histogram_large {
            return Scope::Histogram;
        }
        match self.focus {
            Scope::Folders if self.folder_panel && self.mode == Mode::Grid => Scope::Folders,
            Scope::Metadata if self.metadata_panel && !self.folder.is_none() => Scope::Metadata,
            _ => Scope::Images,
        }
    }

    /// Leader puis direction : panneau de gauche, de droite, ou suivant (dossiers, images,
    /// métadonnées). Un panneau masqué s'affiche ; les dossiers n'existent qu'en grille.
    fn move_focus(&mut self, action: Action) {
        use crate::actions::Scope;
        let mut order = Vec::new();
        if self.mode == Mode::Grid {
            order.push(Scope::Folders);
        }
        order.push(Scope::Images);
        if !self.folder.is_none() {
            order.push(Scope::Metadata);
        }
        let i = order.iter().position(|&s| s == self.focus).unwrap_or(0);
        let next = match action {
            Action::FocusLeft => i.saturating_sub(1),
            Action::FocusRight => (i + 1).min(order.len() - 1),
            _ => (i + 1) % order.len(),
        };
        self.focus = order[next];
        match self.focus {
            Scope::Folders => {
                self.folder_panel = true;
                if self.tree.cursor().is_none() {
                    if self.folder.is_none() {
                        self.tree.set_cursor(0);
                    } else {
                        let dir = self.folder.dir.clone();
                        self.tree.point_at(&dir);
                    }
                }
                self.scroll_to_cursor = true;
            }
            Scope::Metadata => {
                self.metadata_panel = true;
                self.scroll_to_cursor = true;
            }
            Scope::Images | Scope::Histogram | Scope::Crop => {}
        }
    }

    /// Créer, renommer, supprimer un dossier depuis le panneau des dossiers : la boîte
    /// (nom à saisir, ou confirmation avec ce que contient le dossier). Les emplacements
    /// (volume, dossier personnel, racine) ne se renomment ni ne se suppriment.
    fn folder_edit_action(&mut self, ctx: &egui::Context, action: Action) {
        let Some(i) = self.tree.cursor() else { return };
        let row = self.tree.rows()[i].clone();
        let at = ctx.content_rect().center() - vec2(160.0, 120.0);
        if action != Action::FolderCreate && row.depth == 0 {
            self.notice = Some((t!("folders-place-root", name = row.name), true));
            return;
        }
        self.file_dialog = Some(match action {
            Action::FolderCreate => crate::files::FileDialog::NewFolder {
                parent: row.path,
                name: String::new(),
                at,
                fresh: true,
            },
            Action::FolderRename => crate::files::FileDialog::RenameFolder {
                name: row.name,
                path: row.path,
                at,
                fresh: true,
            },
            _ => crate::files::FileDialog::DeleteFolder {
                contents: tri::files::dir_contents(&row.path),
                path: row.path,
                at,
            },
        });
    }

    /// Boîte « Copier / déplacer vers… » pour le dossier `path` (panneau des dossiers :
    /// clavier, clic droit, glisser-déposer) ; `dest` : destination déjà remplie (dossier
    /// où on l'a lâché). Les emplacements (volume, dossier personnel, racine) ne bougent
    /// pas.
    fn transfer_folder(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        copy: bool,
        dest: Option<&Path>,
    ) {
        if let Some(row) = self.tree.rows().iter().find(|r| r.path == path)
            && row.depth == 0
        {
            self.notice = Some((t!("folders-place-root", name = row.name.clone()), true));
            return;
        }
        let mode = if copy {
            tri::files::TransferMode::Copy
        } else {
            tri::files::TransferMode::Move
        };
        let at = ctx.content_rect().center() - vec2(210.0, 160.0);
        self.file_dialog = Some(crate::files::FileDialog::transfer(
            crate::files::Source::Folder(path),
            mode,
            dest,
            self.data_dir.as_deref(),
            at,
        ));
    }

    /// Ajoute le dossier pointé dans le panneau des dossiers aux favoris, ou l'en retire.
    /// Boîte « Nom du favori » pour le dossier pointé (s'il est favori), nom actuel
    /// proposé.
    fn name_favorite(&mut self, ctx: &egui::Context) {
        let Some(i) = self.tree.cursor() else { return };
        let row = self.tree.rows()[i].clone();
        if !row.favorite {
            self.notice = Some((t!("favorite-not-favorite", name = row.name), true));
            return;
        }
        let name = self
            .tree
            .favorite_name(&row.path)
            .unwrap_or_default()
            .to_string();
        self.file_dialog = Some(crate::files::FileDialog::NameFavorite {
            path: row.path,
            name,
            at: ctx.content_rect().center() - vec2(160.0, 120.0),
            fresh: true,
        });
    }

    fn toggle_favorite(&mut self) {
        let Some(i) = self.tree.cursor() else { return };
        let Some(data) = self.data_dir.clone() else {
            return;
        };
        let row = self.tree.rows()[i].clone();
        let on = !row.favorite;
        match tri::places::set_favorite(&data, &row.path, on) {
            Ok(list) => {
                self.tree.set_favorites(list);
                // Le curseur reste sur la ligne du dossier la plus proche (Tree::cursor).
                self.scroll_to_cursor = true;
                let name = row.name;
                let text = if on {
                    t!("folders-favorite-added", name = name)
                } else {
                    t!("folders-favorite-removed", name = name)
                };
                self.notice = Some((text, false));
            }
            Err(e) => {
                let error = e.to_string();
                self.notice = Some((t!("folders-favorite-error", error = error), true));
            }
        }
    }

    /// Après la création, le renommage ou la suppression d'un dossier (ou leur
    /// annulation) : arborescence relue ; le dossier ouvert suit son renommage, ou se
    /// ferme s'il a disparu.
    fn after_folder_change(&mut self, renamed: Option<(PathBuf, PathBuf)>, point: Option<PathBuf>) {
        // Les favoris suivent un dossier renommé (ou un dossier qui les contient).
        if let (Some((from, to)), Some(data)) = (&renamed, &self.data_dir) {
            match tri::places::rename_favorites(data, from, to) {
                Ok(list) => {
                    self.tree.set_favorites(list);
                    self.tree
                        .set_favorite_names(tri::places::favorite_names(data));
                }
                Err(e) => {
                    let error = e.to_string();
                    self.notice = Some((t!("folders-favorite-error", error = error), true));
                }
            }
        }
        self.tree.reload();
        if !self.folder.is_none() && !self.folder.dir.exists() {
            let moved = renamed.and_then(|(from, to)| {
                let rest = self.folder.dir.strip_prefix(&from).ok()?;
                Some(to.join(rest))
            });
            match moved.filter(|p| p.is_dir()) {
                Some(dir) => self.open_folder(&dir),
                None => self.close_folder(),
            }
        }
        if let Some(path) = point {
            if let Some(parent) = path.parent() {
                self.tree.expand(parent);
            }
            self.tree.point_at(&path);
            self.scroll_to_cursor = true;
        }
    }

    /// Éléments sélectionnés dans le panneau des métadonnées (rangs, histogramme compris
    /// en tête) : de l'ancre à la ligne pointée, ou la ligne pointée seule.
    fn meta_range(&self) -> (usize, usize) {
        let anchor = self.meta_anchor.unwrap_or(self.meta_cursor);
        (anchor.min(self.meta_cursor), anchor.max(self.meta_cursor))
    }

    /// Décharger ou éjecter le support amovible pointé dans le panneau des dossiers : les
    /// mêmes boîtes que les icônes de la ligne (confirmation), près du centre.
    fn folder_drive_action(&mut self, ctx: &egui::Context, action: Action) {
        let Some(i) = self.tree.cursor() else { return };
        let row = self.tree.rows()[i].clone();
        let removable = row.kind == Some(PlaceKind::Volume) && row.ejectable;
        // `o` sur un dossier ordinaire : « décharger ici », depuis un des supports
        // amovibles (idée de l'utilisateur, 18/10/2026).
        if action == Action::FolderOffload && !removable {
            if self.offload.is_some() || self.ejecting.is_some() {
                return;
            }
            let sources: Vec<(PathBuf, String)> = self
                .tree
                .rows()
                .into_iter()
                .filter(|r| r.kind == Some(PlaceKind::Volume) && r.ejectable)
                .map(|r| (r.path, r.name))
                .collect();
            let at = ctx.content_rect().center() - vec2(160.0, 120.0);
            match OffloadDialog::here(
                &row.path,
                sources,
                at,
                self.settings.offload.clone(),
                self.settings.data_dir(),
                ctx,
            ) {
                Some(dialog) => self.offload = Some(dialog),
                None => self.notice = Some((t!("folders-no-removable"), true)),
            }
            return;
        }
        if !removable {
            self.notice = Some((t!("folders-not-removable", name = row.name), true));
            return;
        }
        let at = ctx.content_rect().center() - vec2(160.0, 120.0);
        if action == Action::FolderEject {
            self.eject_dialog = Some((row.path, at));
        } else if self.offload.is_none() && self.ejecting.is_none() {
            self.offload = Some(OffloadDialog::new(
                row.path,
                at,
                self.settings.offload.clone(),
                self.settings.data_dir(),
                ctx,
            ));
        }
    }

    /// Déplace le curseur de l'arborescence, et le fait voir.
    fn tree_cursor(&mut self, f: impl FnOnce(&mut Tree)) {
        f(&mut self.tree);
        self.scroll_to_cursor = true;
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
        // Versions peut-être changées : relues.
        self.versions.clear();
        self.version_shown = None;
        if applied.iter().any(|a| a.files_changed) {
            self.after_folder_change(None, None);
        }
        if let Some(last) = applied.last() {
            let show = last
                .images
                .iter()
                .find(|p| p.parent() == Some(&self.folder.dir))
                .cloned();
            if self.folder.is_none() {
                // Le dossier ouvert a disparu (dossier renommé ou créé, défait) : fermé.
            } else if applied.iter().any(|a| a.files_changed) {
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

    /// Rapport d'export (`Ctrl+Maj+E`) : voir [`crate::export::report_dialog`].
    fn export_report_dialog(&mut self, ctx: &egui::Context) {
        if !self.export_report {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        match crate::export::report_dialog(ctx, &self.export_log, now) {
            Some(crate::export::ReportAction::Reveal(file)) => {
                if let Err(e) = tri::files::reveal(&file) {
                    self.file_error = Some(e.to_string());
                }
            }
            Some(crate::export::ReportAction::Clear) => {
                self.export_log.clear();
                if let Some(data) = &self.data_dir
                    && let Err(e) = tri::export_log::clear(data)
                {
                    self.file_error = Some(e.to_string());
                }
            }
            Some(crate::export::ReportAction::Close)
                if ctx.cumulative_pass_nr() > self.export_report_since =>
            {
                self.export_report = false;
            }
            Some(crate::export::ReportAction::Close) | None => {}
        }
    }

    /// Fenêtre de regroupement (`Ctrl+G`, comme Capture One) : regrouper ou non, le seuil
    /// (la grille derrière suit en direct), l'écart de temps maximal ; le seuil est retenu
    /// pour le dossier à la fermeture.
    fn group_window(&mut self, ctx: &egui::Context) {
        if !self.group_dialog {
            return;
        }
        let at = ctx.content_rect().center() - vec2(200.0, 160.0);
        let default_gap = self.settings.similarity.max_gap_s;
        let (mut close, mut reset) = (false, false);
        let (mut strictness, mut gap, mut stacking) =
            (self.group_strictness, self.group_gap, self.stacking);
        let threshold = t!(
            "groups-threshold-value",
            threshold = format!("{:.2}", self.group_threshold()),
            centre = format!("{:.2}", self.group_centre)
        );
        let summary = t!(
            "groups-summary",
            images = self.files.len(),
            stacks = self.stacks.count(),
            groups = self.stacks.groups()
        );
        let progress = self.progress[5];
        let modal = modal_at("group_dialog", at)
            .title(t!("groups-title"))
            .show(ctx, |ui| {
                // Le clavier sur la case dès l'ouverture (retour du 19/10/2026 : il
                // fallait deux Tab) : Espace coche, Tab passe au seuil.
                let enabled = ui.checkbox(&mut stacking, t!("groups-enabled"));
                if std::mem::take(&mut self.group_dialog_fresh) {
                    enabled.request_focus();
                }
                ui.add_space(4.0);
                ui.label(t!("groups-threshold"));
                // Double-clic : valeur par défaut, sur tous les curseurs (retour du
                // 21/10/2026, comme Lightroom).
                if ui
                    .add(
                        egui::Slider::new(&mut strictness, -1.0..=1.0)
                            .step_by(0.05)
                            .show_value(false),
                    )
                    .double_clicked_slider(ui)
                {
                    strictness = 0.0;
                }
                ui.weak(threshold);
                ui.weak(t!("groups-threshold-tip"));
                ui.add_space(4.0);
                ui.label(t!("groups-gap"));
                if ui
                    .add(
                        egui::Slider::new(&mut gap, 0.0..=600.0)
                            .step_by(5.0)
                            .suffix(" s"),
                    )
                    .double_clicked_slider(ui)
                {
                    gap = default_gap;
                }
                ui.weak(t!("groups-gap-tip"));
                ui.add_space(6.0);
                ui.label(summary);
                if progress.total > 0 && progress.done < progress.total {
                    ui.weak(t!(
                        "groups-computing",
                        done = progress.done,
                        total = progress.total
                    ));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    reset = ui
                        .add_enabled(strictness != 0.0, egui::Button::new(t!("groups-default")))
                        .clicked();
                    close = ui.button(t!("close")).clicked();
                    ui.weak(t!("or-escape"));
                });
            });
        if reset {
            strictness = 0.0;
        }
        if (strictness, gap, stacking) != (self.group_strictness, self.group_gap, self.stacking) {
            self.group_strictness = strictness;
            self.group_gap = gap;
            self.stacking = stacking;
            self.stacks_dirty = true;
        }
        if close || modal.should_close() {
            self.group_dialog = false;
            self.remember_group_strictness();
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

    /// Largeurs des libellés et des raccourcis de l'aide (les plus longs), mesurées une
    /// fois : dès la première image (`update`), ce qui prépare aussi dans la texture des
    /// polices les caractères de l'aide, sinon dessinés à sa première ouverture.
    fn help_widths(&mut self, ctx: &egui::Context) -> (f32, f32) {
        if let Some(widths) = self.help_widths {
            return widths;
        }
        let style = ctx.global_style();
        let body = egui::TextStyle::Body.resolve(&style);
        let mono = egui::TextStyle::Monospace.resolve(&style);
        let rows = help_rows(&self.keymap, "");
        let widths = ctx.fonts_mut(|fonts| {
            let mut width = |text: String, font: egui::FontId| {
                fonts.layout_no_wrap(text, font, Color32::WHITE).size().x
            };
            rows.iter().fold((0.0f32, 0.0f32), |(l, k), (a, keys)| {
                (
                    l.max(width(a.label(), body.clone())),
                    k.max(width(keys.clone(), mono.clone())),
                )
            })
        });
        self.help_widths = Some(widths);
        widths
    }

    /// Clavier au panneau cliqué (retours du 22/10/2026 : selon l'endroit, un clic
    /// changeait le panneau actif ou non) : un clic dans le panneau des dossiers ou des
    /// métadonnées lui donne le clavier, un clic dans la zone des images le leur rend. Les
    /// clics sur une boîte ouverte par-dessus ne comptent pas.
    fn click_focus(
        &mut self,
        ctx: &egui::Context,
        folders: Option<Rect>,
        metadata: Option<Rect>,
        center: Rect,
    ) {
        use crate::actions::Scope;
        let Some(pos) = ctx.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        }) else {
            return;
        };
        if ctx
            .layer_id_at(pos)
            .is_some_and(|layer| layer.order != egui::Order::Background)
        {
            return;
        }
        if folders.is_some_and(|r| r.contains(pos)) {
            self.focus = Scope::Folders;
        } else if metadata.is_some_and(|r| r.contains(pos)) {
            self.focus = Scope::Metadata;
        } else if center.contains(pos) && matches!(self.focus, Scope::Folders | Scope::Metadata) {
            self.focus = Scope::Images;
        }
    }

    /// Fond du plein écran, de la comparaison et de la loupe : celui choisi (`Ctrl+L`),
    /// sinon celui du thème.
    fn view_background(&self) -> Color32 {
        self.background.map_or(
            self.gui.theme.view_background(&self.gui.colors),
            Color32::from,
        )
    }

    /// Fond du thème pour ce qu'on regarde : grille, ou plein écran et comparaison.
    fn theme_background(&self, visuals: &egui::Visuals) -> Color32 {
        match self.mode {
            Mode::Grid => self
                .gui
                .theme
                .center(&self.gui.colors)
                .unwrap_or(visuals.panel_fill),
            _ => self.gui.theme.view_background(&self.gui.colors),
        }
    }

    /// Liste des actions, de leurs identifiants et de leurs touches.
    fn help_window(&mut self, ctx: &egui::Context) {
        let mut open = self.help;
        let mut check_update = false;
        let mut report = false;
        let opening = self.help_search.is_none();
        let mut query = self.help_search.take().unwrap_or_default();
        // Défilement au clavier (retour du 18/10/2026) : flèches haut / bas, pages, et
        // Ctrl+U / Ctrl+D ; pris avant le champ de recherche, à qui elles ne servent pas.
        let max_height = ctx.content_rect().height() * 0.75;
        let line = self.gui.dialogs.help_scroll_line;
        let scroll = ctx.input_mut(|i| {
            let mut delta = 0.0;
            for (modifiers, key, step) in [
                (egui::Modifiers::NONE, egui::Key::ArrowDown, line),
                (egui::Modifiers::NONE, egui::Key::ArrowUp, -line),
                (egui::Modifiers::NONE, egui::Key::PageDown, max_height * 0.9),
                (egui::Modifiers::NONE, egui::Key::PageUp, -max_height * 0.9),
                (egui::Modifiers::CTRL, egui::Key::D, max_height * 0.5),
                (egui::Modifiers::CTRL, egui::Key::U, -max_height * 0.5),
            ] {
                while i.consume_key(modifiers, key) {
                    delta += step;
                }
            }
            delta
        });
        // Largeur fixée d'avance d'après les libellés (issue n° 11) : calculée d'après la
        // fenêtre de l'image précédente, la colonne poussait la fenêtre un peu plus à chaque
        // image, qui s'ouvrait en grandissant pendant plusieurs images.
        let (label_width, keys_width) = self.help_widths(ctx);
        if self.help_lines.as_ref().is_none_or(|l| l.query != query) {
            self.help_lines = Some(HelpLines::new(&self.keymap, &query));
        }
        let lines = self.help_lines.take().unwrap_or_default();
        let style = ctx.global_style();
        let gap = 32.0;
        let width = (label_width + keys_width + gap)
            .max(self.gui.dialogs.help_width)
            .min(ctx.content_rect().width() * 0.9);
        // Colonne des libellés : le reste, moins l'espace entre colonnes et l'ascenseur.
        let label_column = (width
            - keys_width
            - 2.0 * style.spacing.item_spacing.x
            - style.spacing.scroll.bar_width
            - style.spacing.scroll.bar_outer_margin)
            .max(width / 2.0);
        egui::Window::new(t!("help-title"))
            .open(&mut open)
            .min_width(width)
            .max_width(width)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                // Le contenu à cette largeur : la zone défilante (qui ne rétrécit pas) prenait
                // sinon toute la place disponible, au-delà de la barre de titre.
                ui.set_width(width);
                // Version (en évidence et copiable, issue n° 7 : on la recopie dans un
                // signalement), recherche d'une mise à jour (sur demande seulement) et
                // formulaire de signalement prérempli.
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("tri {}", self.build.label())).strong());
                    let (rect, copy) = crate::marks::clickable(ui, 14.0, &t!("help-copy-version"));
                    let back = ui.visuals().panel_fill;
                    crate::marks::copy_icon(ui.painter(), rect, ui.visuals().text_color(), back);
                    if copy.clicked() {
                        ctx.copy_text(tri::update::report_line(&self.build));
                        self.notice = Some((t!("help-version-copied"), false));
                    }
                    ui.add_space(8.0);
                    if ui
                        .small_button(Action::CheckUpdate.label())
                        .on_hover_text(t!("update-tip"))
                        .clicked()
                    {
                        check_update = true;
                    }
                    let keys = self.keymap.keys_for(Action::ReportIssue).join(", ");
                    if ui
                        .small_button(t!("help-report"))
                        .on_hover_text(format!("{}\n{keys}", t!("help-report-tip")))
                        .clicked()
                    {
                        report = true;
                    }
                });
                ui.add_space(6.0);
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
                // Défilante : la liste des actions dépasse la hauteur d'un petit écran.
                // Toute la largeur de la boîte : l'ascenseur au bord, les bandes du tableau
                // jusqu'à lui (retour du 18/10/2026 : la boîte élargie gardait une zone de
                // texte étroite).
                // Toute sa hauteur dès la première image quand la liste défile (issue n° 11).
                if lines.lines.is_empty() {
                    ui.weak(t!("help-no-match"));
                }
                // Seulement les lignes visibles (une vingtaine sur ~180), toutes de même
                // hauteur, titres de thème compris ; bandes une ligne sur deux.
                let row_height = ui.spacing().interact_size.y;
                let spacing = ui.spacing().item_spacing;
                let _output = ScrollArea::vertical()
                    .id_salt("help_rows")
                    .max_height(max_height)
                    .min_scrolled_height(max_height)
                    .auto_shrink([false, true])
                    .show_rows(ui, row_height, lines.lines.len(), |ui, range| {
                        if scroll != 0.0 {
                            ui.scroll_with_delta(vec2(0.0, -scroll));
                        }
                        let full = ui.available_width();
                        for i in range {
                            let row = Rect::from_min_size(ui.cursor().min, vec2(full, row_height));
                            if i % 2 == 1 {
                                ui.painter().rect_filled(
                                    row.expand2(vec2(0.0, spacing.y / 2.0)),
                                    0.0,
                                    ui.visuals().faint_bg_color,
                                );
                            }
                            ui.allocate_ui_with_layout(
                                vec2(full, row_height),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_size(vec2(full, row_height));
                                    match &lines.lines[i] {
                                        // Rangées par thème, avec un titre (recherche comprise).
                                        HelpLine::Group(name) => {
                                            ui.label(egui::RichText::new(name).strong());
                                        }
                                        HelpLine::Action { label, id, keys } => {
                                            // Identifiant technique au survol seulement (utile
                                            // pour la section `[keys]` de la configuration).
                                            ui.allocate_ui_with_layout(
                                                vec2(label_column, row_height),
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    ui.set_min_width(label_column);
                                                    ui.label(label.as_str()).on_hover_text(*id);
                                                },
                                            );
                                            // Chasse fixe : « l » et « I » restent distincts.
                                            ui.label(
                                                egui::RichText::new(keys.as_str())
                                                    .monospace()
                                                    .strong(),
                                            );
                                        }
                                    }
                                },
                            );
                        }
                    });
                #[cfg(test)]
                {
                    self.help_offset = _output.state.clamped_offset().y;
                }
                ui.add_space(6.0);
                ui.weak(t!("help-close-hint"));
            });
        self.help_lines = Some(lines);
        self.help_search = Some(query);
        self.help = open;
        if check_update {
            self.check_update(ctx);
        }
        if report {
            self.perform(ctx, Action::ReportIssue);
        }
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
        // Recadrage : Tab change de préréglage ; egui, lui, y déplacerait le focus vers un
        // champ ou un bouton de la barre (rendu en fin d'image, sauf depuis un champ).
        let crop_tab = self.crop.is_some()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if self.engine.is_none() && self.error.is_none() {
            trace(|| "première image".into());
            self.start_engine(&ctx);
            trace(|| "moteur démarré".into());
        }
        self.drain_events();
        // Piles à jour avant les touches et le dessin : leur vue ne doit jamais retarder
        // sur la liste (filtre changé, image renommée…).
        // Retour à la grille depuis une image cachée d'une pile (mode pile) : sa couverture.
        let hidden = self.mode == Mode::Grid
            && !self.files.is_empty()
            && !self.stacks.shown.is_empty()
            && self.stacks.position(self.selected).is_none();
        if hidden || self.stacks_dirty || self.stacks.shown.iter().any(|&i| i >= self.files.len()) {
            self.regroup();
        }
        self.watch_volumes(&ctx);
        self.poll_loading(&ctx);
        if self.engine_backlog {
            ctx.request_repaint();
        }
        self.watch_folder(&ctx);
        self.poll_eject();
        self.run_script(&ctx);
        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.mouse_active = true;
        }
        self.handle_keys(&ctx);
        // Les champs de recherche dessinés dans cette image disent s'ils ont le clavier.
        self.search_focused = None;
        self.update_loads(&ctx);

        self.focus_mode_hint(&ctx);
        // Barre du haut : icône et nom, à gauche (comme darktable).
        if self.gui.title_bar && !self.focus_mode {
            egui::Panel::top("title").show(ui, |ui| self.title_bar(ui));
        }
        let status_click = if self.focus_mode {
            None
        } else {
            egui::Panel::bottom("status")
                .show(ui, |ui| self.status_bar(ui))
                .inner
        };
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
        // Rectangles des panneaux latéraux : un clic dedans leur donne le clavier.
        let mut metadata_rect = None;
        let mut folders_rect = None;
        if self.metadata_panel && !self.focus_mode && self.error.is_none() && !self.folder.is_none()
        {
            let panel = egui::Panel::right("metadata")
                .resizable(true)
                .default_size(self.gui.metadata.width)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| self.metadata_panel(ui));
                    if self.focus == crate::actions::Scope::Metadata {
                        panel_focus_frame(
                            ui,
                            Color32::from(self.gui.dialogs.title_color),
                            self.gui.dialogs.focus_frame_width,
                        );
                    }
                });
            metadata_rect = Some(panel.response.rect);
        }
        if self.folder_panel && !self.focus_mode && self.mode == Mode::Grid {
            let panel = egui::Panel::left("folders")
                .resizable(true)
                .default_size(self.gui.folders.width)
                .show(ui, |ui| self.folder_panel(ui));
            folders_rect = Some(panel.response.rect);
        }
        // Recadrage : sa barre, à la place du bandeau de notation.
        if let Some(crop) = &self.crop {
            // Profils qui s'appliquent à l'image recadrée (pas « JPEG tel quel » sur un
            // rendu de RAW, retour du 19/10/2026).
            let flat = crop.source == tri::versions::Source::Embedded
                || tri::media::kind_of(&crop.image) == Some(MediaKind::Flat);
            let profiles: Vec<(String, String)> = self
                .settings
                .export
                .applicable(flat)
                .into_iter()
                // Une version recadrée n'est jamais « telle quelle ».
                .filter(|p| !p.is_as_is())
                .map(|p| (p.id.clone(), crate::export::profile_name(p)))
                .collect();
            let presets: Vec<(String, String)> = self
                .settings
                .versions
                .presets
                .0
                .iter()
                .map(|p| (p.id.clone(), preset_name(p)))
                .collect();
            let mut action = None;
            egui::Panel::bottom("crop").show(ui, |ui| {
                if let Some(crop) = &mut self.crop {
                    action = crop.bar(ui, &profiles, &presets);
                }
            });
            match action {
                Some(crate::crop::BarAction::Save) => self.save_crop(),
                Some(crate::crop::BarAction::Cancel) => self.crop = None,
                Some(crate::crop::BarAction::Preset(i)) => self.apply_preset(i),
                Some(crate::crop::BarAction::SavePreset) => self.save_preset(),
                None => {}
            }
        } else if self.rating_bar
            && !self.focus_mode
            && !self.files.is_empty()
            && self.error.is_none()
        {
            egui::Panel::bottom("rating").show(ui, |ui| self.rating_bar(ui));
        }
        if self.filter_bar && !self.focus_mode && self.mode == Mode::Grid && self.error.is_none() {
            egui::Panel::top("filters").show(ui, |ui| self.filter_bar(ui));
        }
        // Fond central : celui choisi (Ctrl+L), sinon gris neutre avec ce thème (grille et
        // plein écran).
        let mut central = egui::CentralPanel::default();
        if let Some(fill) = self
            .background
            .map(Color32::from)
            .or(self.gui.theme.center(&self.gui.colors))
        {
            central = central.frame(egui::Frame::central_panel(ui.style()).fill(fill));
        }
        let center = central.show(ui, |ui| {
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
        self.click_focus(&ctx, folders_rect, metadata_rect, center.response.rect);
        if self.help {
            self.help_window(&ctx);
        } else {
            self.help_search = None;
            // Préparée d'avance (lignes, largeurs, caractères des polices), sans attendre
            // F1 : l'ouverture n'a plus rien à calculer.
            self.help_widths(&ctx);
            if self.help_lines.is_none() {
                self.help_lines = Some(HelpLines::new(&self.keymap, ""));
            }
        }
        self.settings_window(&ctx);
        self.history_dialog(&ctx);
        self.export_report_dialog(&ctx);
        self.group_window(&ctx);
        self.rating_dialog(&ctx);
        self.file_dialog(&ctx);
        self.poll_export();
        self.eject_dialog(&ctx);
        self.update_dialog(&ctx);
        self.histogram_dialog(&ctx);
        self.offload_dialog(&ctx);
        // Clavier en cours d'usage : pointeur masqué (dernier mot sur l'icône de l'image).
        if !self.mouse_active {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        if crop_tab {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.save_display();
        self.handle_screenshot(&ctx);
    }
}

/// Actions de `TRI_GUI_ACTIONS` (identifiants séparés par des virgules, ex.
/// `core.nav.next,core.app.help`) ; les identifiants inconnus sont signalés et ignorés.
/// Étape du script de démarrage (`TRI_GUI_ACTIONS`).
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Ouvrir la fenêtre de réglages sur une section (`settings:keys`, `settings:gui.zoom`).
    Settings(String),
    /// Taper dans la recherche de la boîte ouverte, réglages ou aide (`search:vim`).
    Search(String),
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
                None if id.starts_with("settings:") => {
                    Some(Step::Settings(id["settings:".len()..].to_string()))
                }
                None if id.starts_with("search:") => {
                    Some(Step::Search(id["search:".len()..].to_string()))
                }
                None => Action::from_id(id).map(Step::Do),
            };
            if step.is_none() {
                eprintln!("TRI_GUI_ACTIONS : étape inconnue « {id} »");
            }
            step
        })
        .collect()
}

/// Action qui vise l'image courante de la grille (ou en part) : curseur masqué, elle le
/// fait seulement réapparaître.
fn uses_cursor(action: Action) -> bool {
    matches!(
        action,
        Action::NavPrev
            | Action::NavNext
            | Action::NavUp
            | Action::NavDown
            | Action::NavFirst
            | Action::NavLast
            | Action::NavPageUp
            | Action::NavPageDown
            | Action::Open
            | Action::Stars(_)
            | Action::Color(_)
            | Action::Flag(_)
            | Action::FileRename
            | Action::FileDelete
            | Action::FileReveal
            | Action::FileEdit
            | Action::FileCopy
            | Action::FileMove
            | Action::FileExport
            | Action::FileDeleteStack
            | Action::FileCopyStack
            | Action::Send(_)
            | Action::SelectToggle
            | Action::SelectVisual
            | Action::Compare
            | Action::StackToggle
            | Action::StackChoose
    )
}

/// Icône de l'application (`gui/assets`, voir `scripts/make-icon.py`) : fenêtre, Dock,
/// barre des tâches, barre du haut.
pub fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/icon-256.png"))
        .expect("icône embarquée lisible")
        .to_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// Champ de recherche par nom de la grille.
fn grid_search_id() -> egui::Id {
    egui::Id::unique("grid_search")
}

/// Champ de recherche du panneau des dossiers.
fn folder_search_id() -> egui::Id {
    egui::Id::unique("folder_search")
}

/// Position d'un niveau dans `TriApp::progress`, `None` pour le 1:1 (non suivi).
fn tracked(level: Level) -> Option<usize> {
    match level {
        Level::Thumbnail => Some(0),
        Level::Embedded => Some(1),
        Level::Standard => Some(2),
        Level::Exposure => Some(3),
        Level::Sharpness => Some(4),
        Level::Similarity => Some(5),
        Level::EmbeddedFull | Level::Full | Level::Developed => None,
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
            Level::Similarity => s.similarity = avail,
            Level::Developed => s.developed = avail,
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

/// Nom du fichier sans son extension (grille : le format est écrit à droite).
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(|| file_name(path), |n| n.to_string_lossy().into_owned())
}

/// Format écrit sous une vignette : « RAW » pour tous les RAW, sinon le nom usuel du
/// format d'après l'extension (JPEG, TIFF, HEIF, PNG…).
fn format_label(path: &Path, kind: MediaKind) -> String {
    if kind == MediaKind::Raw {
        return "RAW".into();
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    match ext.as_str() {
        "JPG" | "JPE" => "JPEG".into(),
        "TIF" => "TIFF".into(),
        "HEIC" => "HEIF".into(),
        _ => ext,
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

/// Double-clic sur un curseur (valeur par défaut, comme Lightroom) : le curseur d'egui ne
/// détecte que le glisser (`Sense::drag`), jamais de double-clic ; on le lit donc
/// dans les entrées, pointeur sur le curseur.
trait SliderDoubleClick {
    fn double_clicked_slider(&self, ui: &Ui) -> bool;
}

impl SliderDoubleClick for egui::Response {
    fn double_clicked_slider(&self, ui: &Ui) -> bool {
        self.hovered()
            && ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
            })
    }
}

/// Curseur de sévérité (-1 à 1, centre 0) de largeur `width`, avec un repère au
/// centre ; un double-clic y revient. Rend la réponse du curseur et la nouvelle valeur. Flou et
/// piles (même logique, idée de l'utilisateur du 20/10/2026) : à droite plus strict.
fn strictness_slider(ui: &mut Ui, width: f32, value: f32, tip: String) -> (egui::Response, f32) {
    ui.spacing_mut().slider_width = width;
    let mut t = value;
    let slider = ui
        .add(egui::Slider::new(&mut t, -1.0..=1.0).show_value(false))
        .on_hover_text(tip);
    let rail = slider.rect.shrink2(vec2(slider.rect.height() / 2.0, 0.0));
    let x = rail.center().x;
    ui.painter().line_segment(
        [
            egui::pos2(x, rail.top() + 1.0),
            egui::pos2(x, rail.top() + rail.height() * 0.25),
        ],
        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
    );
    if slider.double_clicked_slider(ui) {
        t = 0.0;
    }
    (slider, t)
}

/// Niveaux parcourus à l'ouverture d'un dossier : 1 (le dossier seul), ou avec ses
/// sous-dossiers (`[gui.folders] subfolders_depth` niveaux de plus).
fn folder_depth(subfolders: bool, gui: &GuiSettings) -> usize {
    if subfolders {
        1 + gui.folders.subfolders_depth
    } else {
        1
    }
}

/// Recherche d'une mise à jour ([`TriApp::check_update`]).
enum UpdateCheck {
    /// Requête en cours, dans un fil à part.
    Running(std::thread::JoinHandle<Result<UpdateStatus, String>>),
    /// Résultat : dernière version, version plus récente, état d'un build personnel, ou
    /// message d'erreur.
    Done(Result<UpdateStatus, String>),
}

/// Ce qu'on glisse vers un dossier du panneau des dossiers (demandé le 20/10/2026) ; le
/// lâcher ouvre la boîte « Copier / déplacer vers… », remplie, qui confirme.
#[derive(Clone, Debug)]
enum Dragged {
    /// Un dossier du panneau.
    Folder(PathBuf),
    /// Des images de la grille (la sélection, si l'image glissée en fait partie).
    Images(Vec<PathBuf>),
}

/// Vrai si le glisser-déposer copie au lieu de déplacer : Option (⌥) sur macOS comme le
/// Finder (Ctrl + clic y est le clic droit), Ctrl ailleurs comme Nautilus et
/// l'Explorateur (demandé le 20/10/2026).
fn drag_copies(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        if cfg!(target_os = "macos") {
            i.modifiers.alt
        } else {
            i.modifiers.ctrl
        }
    })
}

/// Pendant un glisser-déposer : ce qu'on glisse, et copier ou déplacer, sous le
/// pointeur.
fn drag_preview(ctx: &egui::Context) {
    let Some(dragged) = egui::DragAndDrop::payload::<Dragged>(ctx) else {
        return;
    };
    let Some(pos) = ctx.pointer_latest_pos() else {
        return;
    };
    let text = match &*dragged {
        Dragged::Folder(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        Dragged::Images(paths) => match paths.as_slice() {
            [one] => one
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => t!("drag-images", n = paths.len()),
        },
    };
    let verb = if drag_copies(ctx) {
        t!("drag-copy")
    } else if cfg!(target_os = "macos") {
        t!("drag-move")
    } else {
        t!("drag-move-ctrl")
    };
    egui::Area::new(egui::Id::unique("drag_preview"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos + vec2(14.0, 10.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(text);
                ui.weak(verb);
            });
        });
}

/// Liseré de la couleur d'accent autour d'un panneau qui a le clavier, épais de `width`
/// (`[gui.dialogs] focus_frame_width`, 1 point : 2 paraissaient lourds, retour du
/// 22/10/2026).
fn panel_focus_frame(ui: &Ui, color: Color32, width: f32) {
    ui.painter().rect_stroke(
        ui.clip_rect().shrink(1.0),
        0.0,
        egui::Stroke::new(width, color),
        egui::StrokeKind::Inside,
    );
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

/// Fond suivant (`Ctrl+L`) : celui du thème (`None`), puis ceux de `list`, en sautant une
/// couleur égale au fond du thème `theme` ou déjà vue (demandé le 22/10/2026 : pas deux
/// fois la même) ; après le dernier, retour au thème.
fn next_background(
    current: Option<crate::settings::Color>,
    theme: Color32,
    list: &[crate::settings::Color],
) -> Option<crate::settings::Color> {
    let mut options = vec![None];
    for &color in list {
        if Color32::from(color) != theme && !options.contains(&Some(color)) {
            options.push(Some(color));
        }
    }
    let at = options.iter().position(|o| *o == current).unwrap_or(0);
    options[(at + 1) % options.len()]
}

/// Nom traduit d'une clé de tri.
fn sort_name(key: tri::sort::SortKey) -> String {
    match key {
        tri::sort::SortKey::Name => t!("sort-name"),
        tri::sort::SortKey::Captured => t!("sort-captured"),
        tri::sort::SortKey::Modified => t!("sort-modified"),
    }
}

/// Une ligne de l'aide : titre de thème, ou action et ses raccourcis.
enum HelpLine {
    /// Titre d'un thème.
    Group(String),
    /// Action : libellé, identifiant (au survol), raccourcis.
    Action {
        label: String,
        id: &'static str,
        keys: String,
    },
}

/// Lignes de l'aide pour une recherche, préparées une fois (issue n° 11).
#[derive(Default)]
struct HelpLines {
    /// Recherche pour laquelle elles ont été préparées.
    query: String,
    /// Titres de thème et actions, dans l'ordre d'affichage.
    lines: Vec<HelpLine>,
}

impl HelpLines {
    fn new(keymap: &Keymap, query: &str) -> HelpLines {
        let mut lines = Vec::new();
        let mut group = None;
        for (action, keys) in help_rows(keymap, query) {
            if group != Some(action.group()) {
                group = Some(action.group());
                lines.push(HelpLine::Group(action.group().label()));
            }
            lines.push(HelpLine::Action {
                label: action.label(),
                id: action.id(),
                keys,
            });
        }
        HelpLines {
            query: query.to_string(),
            lines,
        }
    }
}

/// Lignes de l'aide : (action, raccourcis), rangées par thème. Sans recherche, toutes les
/// actions ; avec une recherche, celles qui y correspondent (nom, raccourci, identifiant,
/// thème, mots-clés cachés), les exactes seulement s'il y en a (`tri::fuzzy::relevant`),
/// le thème le mieux trouvé d'abord et, dans un thème, la meilleure d'abord (22/10/2026 :
/// les thèmes restent visibles pendant la recherche).
fn help_rows(keymap: &Keymap, query: &str) -> Vec<(Action, String)> {
    let found: Vec<((Action, String), tri::fuzzy::Match)> = Action::ALL
        .iter()
        .filter(|a| a.available())
        .filter_map(|&action| {
            let keys = keymap.keys_for(action).join(" / ");
            let fields = [
                action.label(),
                keys.clone(),
                action.id().to_string(),
                action.group().label(),
                crate::actions::keywords(action.id()),
            ];
            let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
            let found = tri::fuzzy::find(query, &fields)?;
            Some(((action, keys), found))
        })
        .collect();
    let mut rows = tri::fuzzy::relevant(found);
    let rank = |a: Action| Group::ALL.iter().position(|g| *g == a.group());
    if query.trim().is_empty() {
        rows.sort_by_key(|((a, _), _)| rank(*a)); // stable : ordre d'origine dans un thème
    } else {
        // Thèmes par meilleure correspondance ; dans un thème, la meilleure, et à égalité
        // le nom le plus court (le plus proche de la requête), d'abord.
        let best = |g: Group| {
            rows.iter()
                .filter(|((a, _), _)| a.group() == g)
                .map(|(_, m)| *m)
                .max()
        };
        let order: Vec<(Group, Option<tri::fuzzy::Match>)> =
            Group::ALL.iter().map(|&g| (g, best(g))).collect();
        let group_best = |a: Action| {
            order
                .iter()
                .find(|(g, _)| *g == a.group())
                .and_then(|o| o.1)
        };
        rows.sort_by_key(|((a, _), m)| {
            (
                std::cmp::Reverse(group_best(*a)),
                rank(*a),
                std::cmp::Reverse(*m),
                a.label().len(),
            )
        });
    }
    rows.into_iter().map(|(row, _)| row).collect()
}

/// Nom affiché d'un préréglage de recadrage : le sien, sinon le nom traduit des
/// préréglages fournis, sinon l'identifiant.
fn preset_name(p: &tri::versions::Preset) -> String {
    if !p.name.trim().is_empty() {
        return p.name.clone();
    }
    match p.id.as_str() {
        "story" => t!("preset-story"),
        "post" => t!("preset-post"),
        id => id.to_string(),
    }
}

/// Geste sur les versions dans le bandeau.
enum VersionAction {
    /// Montrer une version, ou l'original (`None`).
    Show(Option<usize>),
    /// Rouvrir son cadre.
    Edit(usize),
    /// La supprimer.
    Delete(usize),
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
                let pixel = image::Rgb([((40 * i) % 256) as u8, 100, 160]);
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
        // Ni du dossier ouvert (un test l'active pour lui seul).
        settings.watch.interval_ms = 0;
        // Séquences (`f °`, `gg`) : délai large, sinon une machine chargée (tests en
        // parallèle) le dépasse entre deux touches et le test échoue au hasard.
        settings.gui.key_sequence_timeout_ms = 60_000;
        // `y` seul (copier) agit dès l'image suivante, sans attendre un éventuel `y w`.
        settings.gui.key_ambiguous_timeout_ms = 0;
        // Les deux jeux de raccourcis : les tests tapent l'un et l'autre.
        settings.gui.profile = crate::actions::Profile::Both;
        // Pas de darktable dans les tests (installé ou non sur la machine) : un test le
        // simule là où il le faut.
        settings.develop.darktable = String::new();
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
        // Pas de piles : les images de test, des aplats de couleurs proches, seraient
        // groupées (le test des piles les active).
        settings.similarity.background = false;
        let mut harness = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_eframe(move |_cc| TriApp::new(Some(photos), settings, None));
        // Notes lues en fond (issue n° 15) : les tests partent, comme l'utilisateur après
        // un instant, d'un dossier dont les notes sont connues.
        run_until(&mut harness, |app| app.loading.is_none());
        (harness, dir)
    }

    fn press(h: &mut Harness<'_, TriApp>, key: Key, modifiers: Modifiers) {
        h.key_press_modifiers(modifiers, key);
        h.run_steps(3);
    }

    #[test]
    fn thumbnail_info_and_size_from_the_keyboard_and_the_bottom_bar() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        // Fenêtre étroite (1000 points) et panneaux ouverts : pas la place pour les
        // contrôles de la grille, qui recouvriraient les notes du bandeau.
        assert!(h.query_by_label(&Action::ToggleGridInfo.label()).is_none());
        press(&mut h, Key::D, Modifiers::SHIFT);
        press(&mut h, Key::M, Modifiers::SHIFT);
        assert_eq!(
            h.query_all_by_role(egui::accesskit::Role::Slider).count(),
            1
        );
        let default = h.state().gui.grid.cell;
        assert!(h.state().grid_info);
        // Maj+T masque les infos des vignettes, le bouton du bas les remet.
        press(&mut h, Key::T, Modifiers::SHIFT);
        assert!(!h.state().grid_info);
        h.get_by_label(&Action::ToggleGridInfo.label()).click();
        h.run_steps(3);
        assert!(h.state().grid_info);
        // + / - : un palier de plus, de moins (comme le zoom en plein écran).
        h.event(UiEvent::Text("+".into()));
        h.run_steps(3);
        assert_eq!(h.state().grid_cell, default * 1.25);
        h.event(UiEvent::Text("-".into()));
        h.run_steps(3);
        assert_eq!(h.state().grid_cell, default);
        // * (ou le bouton) : 3 images, toutes à l'écran, donc plus grandes.
        h.get_by_label("Toutes les vignettes à l'écran").click();
        h.run_steps(3);
        let fitted = h.state().grid_cell;
        assert!(fitted > default, "{fitted}");
        h.event(UiEvent::Text("-".into()));
        h.run_steps(3);
        h.event(UiEvent::Text("*".into()));
        h.run_steps(3);
        assert_eq!(h.state().grid_cell, fitted);
    }

    #[test]
    fn clicking_the_versions_badge_of_a_thumbnail_starts_cropping() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let image = dir.path().join("photos").join("img1.jpg");
        let versions = tri::versions::Versions {
            list: vec![tri::versions::Version {
                name: "Story".into(),
                crop: [0.0, 0.0, 0.5, 1.0].into(),
                ratio: tri::versions::Ratio::Free,
                source: tri::versions::Source::Embedded,
                profile: None,
            }],
            unknown: Vec::new(),
        };
        tri::versions::write(&image, &versions, tri::xmp::Convention::Darktable).unwrap();
        h.state_mut().versions.clear();
        h.run_steps(3);
        h.get_by_label(&t!("badge-versions", n = 1)).click();
        h.run_steps(3);
        let app = h.state();
        assert_eq!(app.files[app.selected].path, image, "l'image du badge");
        assert_eq!(app.mode, Mode::View);
        assert!(app.crop.is_some(), "recadrage commencé");
    }

    #[test]
    fn checking_for_updates_shows_the_newer_release() {
        // Liste des versions servie par un fichier local (curl lit file://) : pas de réseau
        // dans les tests.
        let (mut h, dir) = harness();
        h.run_steps(3);
        let list = dir.path().join("releases.json");
        std::fs::write(
            &list,
            r#"[{"tag_name": "v99.0.0", "html_url": "https://example.org/r", "body": "Nouveautés", "draft": false}]"#,
        )
        .unwrap();
        let url = format!("file://{}", list.display().to_string().replace('\\', "/"));
        let url = if url.starts_with("file:///") {
            url
        } else {
            url.replacen("file://", "file:///", 1)
        };
        h.state_mut().settings.update.url = url;
        h.state_mut().build.channel = tri::update::Channel::Release;
        press(&mut h, Key::F1, Modifiers::NONE);
        // L'aide se met en page sur plusieurs images (elle grandit, centrée) : cliquer
        // une fois sa place prise.
        h.run_steps(30);
        h.get_by_role_and_label(egui::accesskit::Role::Button, &Action::CheckUpdate.label())
            .click();
        h.run_steps(2);
        assert!(h.state().update.is_some(), "recherche lancée");
        assert!(!h.state().help, "l'aide laisse la place à la boîte");
        run_until(&mut h, |app| {
            matches!(app.update, Some(UpdateCheck::Done(_)))
        });
        h.run_steps(2);
        let current = env!("CARGO_PKG_VERSION");
        let text = t!("update-available", version = "99.0.0", current = current);
        assert!(h.query_by_label(&text).is_some(), "{text}");
        assert!(h.query_by_label("Nouveautés").is_some());
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().update.is_none());
        // Plus de version récente : le message « dernière version ».
        std::fs::write(&list, "[]").unwrap();
        let ctx = h.ctx.clone();
        h.state_mut().perform(&ctx, Action::CheckUpdate);
        run_until(&mut h, |app| {
            matches!(app.update, Some(UpdateCheck::Done(_)))
        });
        h.run_steps(2);
        assert!(
            h.query_by_label(&t!("update-latest", version = current))
                .is_some()
        );
    }

    #[test]
    fn a_personal_build_says_so_and_counts_the_commits_to_pull() {
        // Build personnel : pas de version publiée proposée, mais les commits de main à
        // récupérer (réponse de l'API « compare » servie par un fichier local).
        let (mut h, dir) = harness();
        h.run_steps(3);
        std::fs::write(dir.path().join("c0ffee"), r#"{"ahead_by": 2}"#).unwrap();
        let url = format!(
            "file://{}",
            dir.path().display().to_string().replace('\\', "/")
        );
        let url = if url.starts_with("file:///") {
            url
        } else {
            url.replacen("file://", "file:///", 1)
        };
        h.state_mut().settings.update.compare_url = format!("{url}/{{commit}}");
        h.state_mut().build = tri::update::Build {
            version: "1.0.0".into(),
            commit: Some("c0ffee".into()),
            channel: tri::update::Channel::Personal,
        };
        let ctx = h.ctx.clone();
        h.state_mut().perform(&ctx, Action::CheckUpdate);
        run_until(&mut h, |app| {
            matches!(app.update, Some(UpdateCheck::Done(_)))
        });
        h.run_steps(2);
        let build = h.state().build.clone();
        for line in tri::update::build_text(&build, Some(2)) {
            assert!(h.query_by_label(&line).is_some(), "{line}");
        }
    }

    #[test]
    fn the_thumbnail_caption_names_the_format_instead_of_the_extension() {
        assert_eq!(file_stem(Path::new("/a/DSCF0001.RAF")), "DSCF0001");
        assert_eq!(format_label(Path::new("a.RAF"), MediaKind::Raw), "RAW");
        assert_eq!(format_label(Path::new("a.jpg"), MediaKind::Flat), "JPEG");
        assert_eq!(format_label(Path::new("a.tif"), MediaKind::Flat), "TIFF");
        assert_eq!(format_label(Path::new("a.heic"), MediaKind::Flat), "HEIF");
        assert_eq!(format_label(Path::new("a.png"), MediaKind::Flat), "PNG");
    }

    #[test]
    fn a_failed_thumbnail_shows_a_drawn_warning() {
        // Vignette impossible à calculer : avertissement dessiné, pas de glyphe « ⚠ »
        // (absent de la police : case vide en usage, panique dans les tests ; vu sous
        // Windows, CI du 19/10/2026).
        let (mut h, _dir) = harness();
        run_until(&mut h, |app| {
            app.status.iter().all(|s| s.thumbnail == Avail::Ready)
        });
        let app = h.state_mut();
        app.textures.clear();
        for s in &mut app.status {
            s.thumbnail = Avail::Failed("fichier illisible".into());
        }
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
        // Déplacer l'image courante vers un dossier (Ctrl+X, couper).
        let dest = dir.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        press(&mut h, Key::X, Modifiers::CTRL);
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
        wait_for_label(&mut h, "État au lancement");
        h.get_by_label("État au lancement").click();
        h.run_steps(3);
        assert_eq!(h.state().history.position(), 0);
        assert!(photos.join("img0.jpg").exists() && !photos.join("quai.jpg").exists());
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0, "revenue");
        assert_eq!(h.state().files.len(), 3, "dossier relu");
        // Le pas le plus récent : tout refait.
        let label = h.state().history.entries()[1].label.clone();
        wait_for_label(&mut h, &label);
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
        // Recliquer l'étoile posée : zéro, comme le filtre des étoiles (plus de contrôle
        // « ⊘ ») ; au clavier, 1 deux fois reste à 1 (la touche 0 remet à zéro).
        h.get_by_label("1 étoile").click();
        h.run_steps(3);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 1);
        h.get_by_label("1 étoile").click();
        h.run_steps(3);
        assert_eq!(tri::xmp::read(&image).unwrap().stars, 0);
        press(&mut h, Key::Num1, Modifiers::NONE);
        press(&mut h, Key::Num1, Modifiers::NONE);
        run_until(&mut h, |_| tri::xmp::read(&image).unwrap().stars == 1);
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
        // Le drapeau, lui, ne s'efface pas en le recliquant : un contrôle à part, comme
        // Maj+U ; les étoiles recliquées, si.
        h.get_by_label("Rejeter").click();
        h.run_steps(3);
        assert_eq!(
            tri::xmp::read(&image).unwrap().flag,
            Flag::Rejected,
            "inchangé"
        );
        h.get_by_label("3 étoiles").click();
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
    fn airdrop_is_a_plugin_action_with_a_configurable_key() {
        let airdrop = Action::from_id("plugin.airdrop.send").expect("action du plugin");
        assert_eq!(airdrop.available(), cfg!(target_os = "macos"));
        // Dans l'aide sur macOS seulement ; sans touche par défaut, réglable dans [keys].
        let rows = help_rows(&Keymap::default(), "airdrop");
        assert_eq!(
            rows.iter().any(|(a, _)| *a == airdrop),
            cfg!(target_os = "macos")
        );
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("plugin.airdrop.send".to_string(), vec!["F6".to_string()]);
        let (km, warnings) = Keymap::default().with_overrides(&keys);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(km.keys_for(airdrop).len(), 1);
        // Lancée depuis un fil de test (pas le fil principal) : refus expliqué, rien
        // d'ouvert.
        let (mut h, _dir) = harness();
        h.run_steps(3);
        h.state_mut().keymap = km;
        press(&mut h, Key::F6, Modifiers::NONE);
        let error = h.state().file_error.clone().unwrap_or_default();
        assert!(error.starts_with("AirDrop"), "{error}");
    }

    #[test]
    fn overlays_cycle_through_grids_and_user_files() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let overlays = dir.path().join("data/overlays");
        std::fs::create_dir_all(&overlays).unwrap();
        std::fs::write(
            overlays.join("insta.toml"),
            "name = \"Insta\"\n[[shape]]\nkind = \"frame\"\nratio = \"4:5\"\n",
        )
        .unwrap();
        std::fs::write(
            overlays.join("cassé.toml"),
            "[[shape]]\nkind = \"cercle\"\n",
        )
        .unwrap();
        press(&mut h, Key::Enter, Modifiers::NONE);
        press(&mut h, Key::O, Modifiers::NONE);
        assert_eq!(h.state().overlay, 1);
        // Un fichier mal écrit est signalé (les autres restent utilisables).
        let notice = h.state().notice.clone().unwrap();
        assert!(notice.1 && notice.0.contains("cassé"), "{notice:?}");
        // Quatre grilles intégrées, puis le fichier de l'utilisateur, puis aucune.
        assert_eq!(h.state().overlays.len(), 5);
        for _ in 0..4 {
            press(&mut h, Key::O, Modifiers::NONE);
        }
        assert_eq!(h.state().overlays[h.state().overlay - 1].name(), "Insta");
        press(&mut h, Key::O, Modifiers::NONE);
        assert_eq!(h.state().overlay, 0, "après la dernière : aucune");
        // Maj+O : quatre orientations, puis retour au départ.
        press(&mut h, Key::O, Modifiers::SHIFT);
        assert_eq!(h.state().overlay_flip, tri::overlay::Flip::X);
        assert!(h.state().notice.as_ref().unwrap().0.contains("2 / 4"));
        for _ in 0..3 {
            press(&mut h, Key::O, Modifiers::SHIFT);
        }
        assert_eq!(h.state().overlay_flip, tri::overlay::Flip::None);
    }

    #[test]
    fn the_help_scrolls_with_the_keyboard() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.state().help);
        for _ in 0..5 {
            press(&mut h, Key::ArrowDown, Modifiers::NONE);
        }
        let after = h.state().help_offset;
        assert!(after > 0.0, "flèche bas : défile ({after})");
        press(&mut h, Key::PageDown, Modifiers::NONE);
        assert!(h.state().help_offset > after, "page suivante");
        press(&mut h, Key::U, Modifiers::CTRL);
        press(&mut h, Key::PageUp, Modifiers::NONE);
        press(&mut h, Key::PageUp, Modifiers::NONE);
        assert_eq!(h.state().help_offset, 0.0, "revenu en haut");
        assert!(h.state().help, "toujours ouverte");
    }

    #[test]
    fn the_grid_cursor_hides_for_an_overview() {
        let (mut h, dir) = harness();
        h.run_steps(5);
        let image = dir.path().join("photos/img0.jpg");
        // Échap sans rien à fermer : plus d'image en surbrillance.
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().cursor_hidden);
        // Une note ne vise pas une image qu'on ne voit pas : le curseur revient seulement.
        press(&mut h, Key::Num3, Modifiers::NONE);
        assert!(!h.state().cursor_hidden);
        assert_eq!(h.state().folder.notes.notes(&image).stars, 0, "rien noté");
        // Une flèche : le curseur revient là où il était, sans bouger.
        press(&mut h, Key::Escape, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert!(!h.state().cursor_hidden);
        assert_eq!(h.state().selected, 0);
        // Les filtres, les panneaux… restent utilisables curseur masqué.
        press(&mut h, Key::Escape, Modifiers::NONE);
        press(&mut h, Key::N, Modifiers::SHIFT);
        assert!(
            h.state().cursor_hidden,
            "Maj+N : bandeau, curseur toujours masqué"
        );
        // Clic dans le vide, sous les vignettes : masqué ; clic sur une image : revenu.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        let rects = h.state().cell_rects.clone();
        let rect = rects[0].1;
        let click = |h: &mut Harness<'_, TriApp>, pos: egui::Pos2| {
            h.event(UiEvent::PointerMoved(pos));
            for pressed in [true, false] {
                h.event(UiEvent::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
            h.run_steps(3);
        };
        // Place vide : la colonne de la deuxième image, la ligne de la troisième.
        let last = rects.iter().map(|(_, r)| *r).fold(rect, |a, r| a.union(r));
        let empty = egui::pos2(
            last.max.x - rect.width() / 2.0,
            last.max.y - rect.height() / 2.0,
        );
        assert!(rects.iter().all(|(_, r)| !r.contains(empty)));
        click(&mut h, empty);
        assert!(h.state().cursor_hidden, "clic dans le vide");
        click(&mut h, rect.center());
        assert!(!h.state().cursor_hidden, "clic sur une image");
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
    fn peaking_shows_fitted_and_at_one_to_one_follows_strictness_and_rotates_colours() {
        let (mut h, _dir) = harness_with(200, 150);
        h.run_steps(3);
        press(&mut h, Key::P, Modifiers::NONE);
        assert!(h.state().peaking);
        press(&mut h, Key::Enter, Modifiers::NONE);
        // En vue ajustée aussi (retour du 21/10/2026), sur l'image affichée.
        run_until(&mut h, |s| s.peaking_texture.is_some());
        let threshold = h.state().settings.sharpness.threshold;
        let ((path, _, t), _) = h.state().peaking_texture.clone().unwrap();
        assert!(path.ends_with("img0.jpg"));
        assert_eq!(t, threshold, "au centre : le seuil des réglages");
        press(&mut h, Key::Z, Modifiers::NONE);
        run_until(&mut h, |s| {
            s.peaking_texture
                .as_ref()
                .is_some_and(|((_, l, _), _)| *l == Level::Full)
        });
        let (_, texture) = h.state().peaking_texture.clone().unwrap();
        assert_eq!(
            texture.size(),
            [200, 150],
            "le 1:1 d'une image plate, pixel pour pixel"
        );
        // Ctrl+J : plus souple, la carte est refaite à un seuil plus bas.
        press(&mut h, Key::J, Modifiers::CTRL);
        assert!(h.state().blur_strictness < 0.0);
        run_until(&mut h, |s| {
            s.peaking_texture
                .as_ref()
                .is_some_and(|((_, _, t), _)| *t < threshold)
        });
        press(&mut h, Key::K, Modifiers::CTRL);
        assert_eq!(h.state().blur_strictness, 0.0, "retour juste au centre");
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
        // f b puis f i (inverser) : seulement la floue ; f i encore : sans elle.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::NONE);
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::I, Modifiers::NONE);
        assert_eq!(h.state().files.len(), 1);
        assert_eq!(h.state().files[0].path, c);
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::I, Modifiers::NONE);
        assert_eq!(h.state().files.len(), 3);
        // Curseur de sévérité tout à gauche : plus souple, une photo limite (0,45 × les
        // autres) n'est plus floue et revient dans la grille ; au centre, de nouveau floue.
        let scores: Vec<PathBuf> = h.state().scores.keys().cloned().collect();
        for path in scores {
            let value = if path == c { 4.5 } else { 10.0 };
            h.state_mut().scores.insert(path, value);
        }
        h.state_mut().set_blur_strictness(0.0);
        assert!(h.state().is_blurry(&c));
        h.state_mut().set_blur_strictness(-1.0);
        assert!(!h.state().is_blurry(&c));
        assert_eq!(h.state().files.len(), 4);
        h.state_mut().set_blur_strictness(0.0);
        assert!(h.state().is_blurry(&c));
        assert_eq!(h.state().files.len(), 3);
        // Tout à gauche, même une photo très floue est gardée : toutes visibles (retour
        // du 21/10/2026) ; juste avant le bout, elle reste floue.
        h.state_mut().scores.insert(c.clone(), 0.5);
        h.state_mut().set_blur_strictness(-0.9);
        assert!(h.state().is_blurry(&c));
        h.state_mut().set_blur_strictness(-1.0);
        assert!(!h.state().is_blurry(&c));
        assert_eq!(h.state().files.len(), 4);
        h.state_mut().set_blur_strictness(0.0);
        // En plein écran, le curseur ne refiltre pas la grille (l'image regardée ne doit
        // pas disparaître) : au retour seulement.
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
        h.state_mut().set_blur_strictness(-1.0);
        h.run_steps(2);
        assert_eq!(h.state().files.len(), 3, "pas pendant le plein écran");
        press(&mut h, Key::Escape, Modifiers::NONE);
        run_until(&mut h, |app| app.files.len() == 4);
    }

    #[test]
    fn double_clicking_a_slider_resets_it() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        // Curseur du flou, montré avec un filtre sur la netteté.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::B, Modifiers::NONE);
        h.state_mut().blur_strictness = 0.7;
        h.run_steps(2);
        // Deux vrais clics de souris au quart gauche (le curseur d'egui ne voit que le
        // glisser ; un clic seul y amène la valeur, loin du centre).
        // Celui de la barre des filtres, en haut (celui de la taille des vignettes est en
        // bas).
        let rect = h
            .query_all_by_role(egui::accesskit::Role::Slider)
            .map(|n| n.rect())
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("curseur du flou");
        let at = pos2(rect.left() + rect.width() * 0.25, rect.center().y);
        let click = |h: &mut Harness<'_, TriApp>| {
            for pressed in [true, false] {
                h.input_mut().events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
                h.run_steps(1);
            }
        };
        // Le banc d'essai avance de 0,5 s par image : délai du double-clic élargi.
        h.ctx
            .options_mut(|o| o.input_options.max_double_click_delay = 5.0);
        h.input_mut().events.push(egui::Event::PointerMoved(at));
        h.run_steps(1);
        click(&mut h);
        assert!(
            h.state().blur_strictness < -0.2,
            "un clic : la valeur sous le pointeur"
        );
        click(&mut h);
        h.run_steps(1);
        assert_eq!(h.state().blur_strictness, 0.0, "double-clic : le centre");
    }

    #[test]
    fn the_full_screen_view_gives_the_resolution_of_what_is_shown() {
        let (mut h, _dir) = harness_images(vec![("a.png", checkerboard(4, 0))]);
        press(&mut h, Key::Enter, Modifiers::NONE);
        let file = h.state().files[0].clone();
        let ctx = h.ctx.clone();
        let mut size = None;
        for _ in 0..500 {
            size = h.state_mut().shown_resolution(&ctx, &file, None);
            if size.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.run_steps(1);
        }
        assert_eq!(size, Some((240, 160)));
        // Une version : son cadre ; son préréglage d'export le réduit au besoin.
        let mut version = tri::versions::Version {
            name: "Moitié".into(),
            crop: [0.0, 0.0, 0.5, 1.0].into(),
            ratio: tri::versions::Ratio::Free,
            source: tri::versions::Source::Embedded,
            profile: None,
        };
        let shown = |h: &mut Harness<'_, TriApp>, v: &tri::versions::Version| {
            h.state_mut().shown_resolution(&ctx, &file, Some(v))
        };
        assert_eq!(shown(&mut h, &version), Some((120, 160)));
        let mut small = tri::export::Profile::defaults()[0].clone();
        small.id = "petit".into();
        small.max_side = 80;
        h.state_mut().settings.export.profiles.0.push(small);
        version.profile = Some("petit".into());
        assert_eq!(shown(&mut h, &version), Some((60, 80)));
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
    fn move_with_ctrl_x_typing_tab_and_enter_then_copy_from_recents() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        let dest = dir.path().join("Sélection");
        std::fs::create_dir(&dest).unwrap();
        // Ctrl+X : déplacer l'image courante ; chemin tapé puis complété par Tab.
        press(&mut h, Key::X, Modifiers::CTRL);
        assert!(h.state().file_dialog.is_some());
        assert!(h.ctx.egui_wants_keyboard_input(), "le champ a le focus");
        let sep = std::path::MAIN_SEPARATOR;
        h.event(UiEvent::Text(format!("{}{sep}Sél", dir.path().display())));
        h.run_steps(2);
        press(&mut h, Key::Tab, Modifiers::NONE);
        assert!(h.query_by_label_contains("fichier (sidecars").is_some());
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
        // x (comme dans vim) : la boîte de suppression.
        press(&mut h, Key::X, Modifiers::NONE);
        assert!(h.state().file_dialog.is_some(), "x supprime");
        press(&mut h, Key::Escape, Modifiers::NONE);
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
    fn deleting_one_image_of_a_pair_can_leave_the_other() {
        // Une paire (même nom, deux formats) : cochée, les deux partent ; décochée,
        // seulement l'image choisie (supprimer les JPEG d'un dossier RAW+JPEG).
        let (mut h, dir) = harness_files(&["a.jpg", "a.png", "b.jpg", "b.png"], 32, 24);
        h.run_steps(3);
        let photos = dir.path().join("photos");
        assert_eq!(selected_name(&h), "a.jpg");
        let label = t!("files-with-pairs");
        press(&mut h, Key::Delete, Modifiers::NONE);
        h.get_by_label(&label).click();
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(!photos.join("a.jpg").exists());
        assert!(photos.join("a.png").exists(), "l'autre de la paire reste");
        // Case laissée cochée : la paire part entière.
        h.state_mut().select(1);
        h.run_steps(2);
        assert_eq!(selected_name(&h), "b.jpg");
        press(&mut h, Key::Delete, Modifiers::NONE);
        assert!(h.query_by_label(&label).is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(!photos.join("b.jpg").exists() && !photos.join("b.png").exists());
        // Image seule : pas de case.
        h.state_mut().select(0);
        h.run_steps(2);
        assert_eq!(selected_name(&h), "a.png");
        press(&mut h, Key::Delete, Modifiers::NONE);
        assert!(h.query_by_label(&label).is_none());
        press(&mut h, Key::Escape, Modifiers::NONE);
    }

    #[test]
    fn moving_one_image_of_a_pair_can_leave_the_other() {
        let (mut h, dir) = harness_files(&["a.jpg", "a.png"], 32, 24);
        h.run_steps(3);
        let photos = dir.path().join("photos");
        let dest = dir.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        press(&mut h, Key::X, Modifiers::CTRL);
        h.event(UiEvent::Text(dest.display().to_string()));
        h.run_steps(2);
        h.get_by_label(&t!("files-with-pairs")).click();
        h.run_steps(2);
        assert!(
            h.query_by_label_contains("1 fichier").is_some(),
            "plan sans la paire"
        );
        h.get_by_label("Valider").click();
        run_until(&mut h, |app| app.file_dialog.is_none());
        assert!(dest.join("a.jpg").exists() && !photos.join("a.jpg").exists());
        assert!(photos.join("a.png").exists() && !dest.join("a.png").exists());
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
        // Deux interrupteurs : JPEG éteint, les RAW seuls ; rallumé, tout ; RAW éteint,
        // les JPEG seuls ; le dernier allumé ne s'éteint pas.
        h.get_by_label("JPEG").click();
        h.run_steps(3);
        assert_eq!(
            h.state().filter,
            KindFilter::RawOnly,
            "clic : même action que le clavier"
        );
        h.get_by_label("JPEG").click();
        h.run_steps(3);
        assert_eq!(h.state().filter, KindFilter::All);
        h.get_by_label("RAW").click();
        h.run_steps(3);
        assert_eq!(h.state().filter, KindFilter::FlatOnly);
        h.get_by_label("JPEG").click();
        h.run_steps(3);
        assert_eq!(
            h.state().filter,
            KindFilter::FlatOnly,
            "un des deux reste allumé"
        );
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
    fn f_s_and_f_r_change_the_order_and_keep_the_selection() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let photos = dir.path().join("photos");
        // img2 la plus ancienne, puis img0, puis img1.
        let now = std::time::SystemTime::now();
        for (name, hours) in [("img2.jpg", 3), ("img0.jpg", 2), ("img1.jpg", 1)] {
            std::fs::File::options()
                .write(true)
                .open(photos.join(name))
                .unwrap()
                .set_modified(now - Duration::from_secs(hours * 3600))
                .unwrap();
        }
        let order = |h: &Harness<'_, TriApp>| -> Vec<String> {
            h.state().files.iter().map(|f| file_name(&f.path)).collect()
        };
        assert_eq!(order(&h), ["img0.jpg", "img1.jpg", "img2.jpg"]);
        h.state_mut().select(1);
        // f r : décroissant, img1 toujours sélectionnée.
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::R, Modifiers::NONE);
        assert_eq!(order(&h), ["img2.jpg", "img1.jpg", "img0.jpg"]);
        assert_eq!(
            file_name(&h.state().files[h.state().selected].path),
            "img1.jpg"
        );
        // f s deux fois : date de modification (décroissant : la plus récente d'abord).
        for _ in 0..2 {
            press(&mut h, Key::F, Modifiers::NONE);
            press(&mut h, Key::S, Modifiers::NONE);
        }
        assert_eq!(h.state().order.key, tri::sort::SortKey::Modified);
        assert_eq!(order(&h), ["img1.jpg", "img0.jpg", "img2.jpg"]);
        assert_eq!(
            file_name(&h.state().files[h.state().selected].path),
            "img1.jpg"
        );
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::R, Modifiers::NONE);
        assert_eq!(order(&h), ["img2.jpg", "img0.jpg", "img1.jpg"]);
        // Retenu d'un lancement à l'autre.
        assert_eq!(h.state().display().sort, Some(h.state().order));
    }

    #[test]
    fn backgrounds_rotate_without_repeating_the_theme() {
        use crate::settings::Color as C;
        let theme = Color32::from_rgb(12, 12, 12);
        let list = [
            C([48, 48, 48]),
            C([12, 12, 12]),
            C([0, 0, 0]),
            C([48, 48, 48]),
        ];
        let mut seen = vec![None];
        let mut current = None;
        for _ in 0..3 {
            current = next_background(current, theme, &list);
            seen.push(current);
        }
        // Le gris du thème et le doublon sautés : thème, gris foncé, noir, puis le thème.
        assert_eq!(
            seen,
            [None, Some(C([48, 48, 48])), Some(C([0, 0, 0])), None]
        );
        // Un fond choisi qui n'est plus proposé repart du début.
        assert_eq!(
            next_background(Some(C([1, 2, 3])), theme, &list),
            Some(C([48, 48, 48]))
        );
        assert_eq!(next_background(None, theme, &[]), None);
    }

    #[test]
    fn help_search_filters_and_typing_is_not_a_shortcut() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.query_by_label("Ligne du dessous").is_some());
        // Rangée par thème sans recherche.
        assert!(h.query_by_label("Navigation").is_some());
        // Plus bas, hors de la vue (seules les lignes visibles sont dessinées) : dans la
        // liste préparée.
        let groups: Vec<&str> = h
            .state()
            .help_lines
            .iter()
            .flat_map(|l| &l.lines)
            .filter_map(|l| match l {
                HelpLine::Group(name) => Some(name.as_str()),
                HelpLine::Action { .. } => None,
            })
            .collect();
        assert!(groups.contains(&"Notation"), "{groups:?}");
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
    fn panels_take_the_keyboard_like_tmux() {
        use crate::actions::Scope;
        let (mut h, dir) = harness();
        h.run_steps(3);
        // Arborescence de test : le dossier temporaire et deux séances.
        for d in ["séance-1", "séance-2"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
            image::RgbImage::from_pixel(8, 8, image::Rgb([10, 10, 10]))
                .save(dir.path().join(d).join("a.jpg"))
                .unwrap();
        }
        let place = tri::places::Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: tri::places::PlaceKind::Home,
        };
        h.state_mut().tree = Tree::with_places(vec![place], false);
        h.state_mut().tree.expand(dir.path());
        // Ctrl+B puis h : le clavier aux dossiers (le panneau s'affiche au besoin).
        h.state_mut().folder_panel = false;
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Folders);
        assert!(h.state().folder_panel, "affiché");
        assert!(
            h.query_by_label("clavier : dossiers (Échap : images)")
                .is_some()
        );
        // j descend (pas l'image suivante), Entrée ouvre et rend le clavier aux images.
        let selected = h.state().selected;
        let first = dir.path().join("séance-1");
        h.state_mut().tree.point_at(&first);
        press(&mut h, Key::J, Modifiers::NONE);
        assert_eq!(
            h.state().tree.cursor_path(),
            Some(dir.path().join("séance-2"))
        );
        press(&mut h, Key::K, Modifiers::NONE);
        assert_eq!(
            h.state().selected,
            selected,
            "j et k ne bougent pas la grille"
        );
        assert_eq!(
            h.state().tree.cursor_path(),
            Some(dir.path().join("séance-1"))
        );
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().folder.dir, dir.path().join("séance-1"));
        assert_eq!(h.state().focus, Scope::Images);
        // Un clic sur un dossier : ouvert, pointé, et le clavier au panneau, comme un clic
        // dans les métadonnées (retour du 22/10/2026).
        h.get_by_label("séance-2").click();
        h.run_steps(3);
        assert_eq!(h.state().folder.dir, dir.path().join("séance-2"));
        assert_eq!(h.state().focus, Scope::Folders);
        assert_eq!(
            h.state().tree.cursor_path(),
            Some(dir.path().join("séance-2"))
        );
        // Un clic sur une image rend le clavier aux images ; sur une ligne des métadonnées,
        // il le donne à leur panneau ; sur un dossier, de nouveau aux dossiers.
        run_until(&mut h, |app| app.cell_rects.iter().any(|(i, _)| *i == 0));
        let cell = h
            .state()
            .cell_rects
            .iter()
            .find(|(i, _)| *i == 0)
            .unwrap()
            .1;
        h.hover_at(cell.center());
        for pressed in [true, false] {
            h.event(UiEvent::PointerButton {
                pos: cell.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.run_steps(3);
        assert_eq!(h.state().focus, Scope::Images, "clic sur une image");
        wait_for_label(&mut h, "Nom");
        h.get_by_label("Nom").click();
        h.run_steps(3);
        assert_eq!(
            h.state().focus,
            Scope::Metadata,
            "clic dans les métadonnées"
        );
        // Le panneau des métadonnées se remplit en fond et peut décaler la mise en page :
        // sous la charge (CI Windows), le clic tombait parfois à côté. Recliqué tant que le
        // clavier n'est pas passé aux dossiers.
        let deadline = Instant::now() + Duration::from_secs(60);
        while h.state().focus != Scope::Folders {
            assert!(Instant::now() < deadline, "clic sur un dossier jamais reçu");
            h.get_by_label("séance-1").click();
            h.run_steps(3);
            std::thread::sleep(Duration::from_millis(10));
        }
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Images);
        // Ctrl+B puis l : les métadonnées.
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::L, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Metadata);
        // Jusqu'à la dernière ligne (netteté comprise), puis l'histogramme en tête : Entrée
        // l'ouvre en grand.
        run_until(&mut h, |app| {
            app.meta_has_histogram && !app.meta_rows.is_empty()
        });
        for _ in 0..40 {
            press(&mut h, Key::J, Modifiers::NONE);
        }
        let last = h.state().meta_rows.len();
        assert_eq!(
            h.state().meta_cursor,
            last,
            "dernière ligne (après l'histogramme)"
        );
        // Maj+k deux fois : trois lignes sélectionnées.
        press(&mut h, Key::K, Modifiers::SHIFT);
        press(&mut h, Key::K, Modifiers::SHIFT);
        assert_eq!(h.state().meta_range(), (last - 2, last));
        press(&mut h, Key::K, Modifiers::NONE);
        assert_eq!(
            h.state().meta_anchor,
            None,
            "sans Maj : la sélection s'efface"
        );
        for _ in 0..40 {
            press(&mut h, Key::K, Modifiers::NONE);
        }
        assert_eq!(h.state().meta_cursor, 0);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().histogram_large, "Entrée sur l'histogramme");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().histogram_large);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Images);
        assert_eq!(
            h.state().mode,
            Mode::Grid,
            "Échap n'a fait que rendre le clavier"
        );
        // o sur un dossier ordinaire : « décharger ici », mais aucune carte branchée.
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        press(&mut h, Key::O, Modifiers::NONE);
        assert!(h.state().offload.is_none());
        // Pas de carte dans l'arborescence de test : expliqué.
        assert!(
            h.state()
                .notice
                .as_ref()
                .unwrap()
                .0
                .contains("aucun support amovible")
        );
        // Une « carte » (dossier temporaire déclaré volume) : décharger ici l'ouvre.
        let card = dir.path().join("carte");
        std::fs::create_dir(&card).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([90, 90, 90]))
            .save(card.join("DSC1.jpg"))
            .unwrap();
        let places = vec![
            tri::places::Place {
                name: "Test".into(),
                path: dir.path().to_path_buf(),
                kind: tri::places::PlaceKind::Home,
            },
            tri::places::Place {
                name: "CARTE".into(),
                path: card.clone(),
                kind: tri::places::PlaceKind::Volume,
            },
        ];
        h.state_mut().tree = Tree::with_places(places, false);
        h.state_mut().tree.expand(dir.path());
        h.state_mut().tree.point_at(&dir.path().join("séance-2"));
        press(&mut h, Key::O, Modifiers::NONE);
        let dialog = h.state().offload.as_ref().expect("décharger ici");
        assert_eq!(dialog.source(), card.as_path());
        assert_eq!(
            dialog.root_here(),
            Some(dir.path().join("séance-2").display().to_string().as_str())
        );
    }

    /// Robustesse (plantage du 19/10/2026 sur macOS, origine inconnue) : des images
    /// retirées du dossier pendant chaque mode (grille, plein écran zoomé, comparaison,
    /// piles) ne doivent jamais faire planter le dessin.
    #[test]
    fn images_removed_elsewhere_in_every_mode_do_not_crash() {
        let names = ["a.jpg", "b.jpg", "c.jpg", "d.jpg", "e.jpg"];
        for mode in 0..4 {
            let (mut h, dir) = harness_files(&names, 64, 48);
            h.run_steps(3);
            let photos = dir.path().join("photos");
            h.state_mut().stacking = mode == 3;
            h.state_mut().stacks_dirty = true;
            press(&mut h, Key::End, Modifiers::NONE);
            match mode {
                1 => {
                    press(&mut h, Key::Enter, Modifiers::NONE);
                    press(&mut h, Key::Z, Modifiers::NONE);
                }
                2 => {
                    press(&mut h, Key::A, Modifiers::CTRL);
                    press(&mut h, Key::C, Modifiers::NONE);
                }
                _ => {}
            }
            h.run_steps(2);
            // Les trois dernières disparaissent (déplacées par un autre logiciel).
            for name in &names[2..] {
                std::fs::remove_file(photos.join(name)).unwrap();
            }
            h.state_mut().reload_folder(None);
            h.run_steps(5);
            assert_eq!(h.state().files.len(), 2, "mode {mode}");
            assert!(h.state().selected < 2, "mode {mode}");
            // Puis toutes : dossier vide.
            for name in &names[..2] {
                std::fs::remove_file(photos.join(name)).unwrap();
            }
            h.state_mut().reload_folder(None);
            h.run_steps(5);
            assert!(h.state().files.is_empty(), "mode {mode}");
            for key in [Key::ArrowRight, Key::Enter, Key::Escape, Key::Num3] {
                press(&mut h, key, Modifiers::NONE);
            }
        }
    }

    #[test]
    fn slash_searches_the_grid_by_name() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        // `/` en grille : le champ de recherche prend le clavier.
        h.event(UiEvent::Text("/".into()));
        h.run_steps(3);
        assert!(
            h.ctx.memory(|m| m.has_focus(grid_search_id())),
            "champ actif"
        );
        h.event(UiEvent::Text("img1".into()));
        h.run_steps(3);
        assert_eq!(names(&h), ["img1.jpg"], "filtre en direct");
        assert!(h.state().filter_summary().contains("img1"));
        // Entrée : le filtre reste, le clavier revient à la grille.
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(!h.ctx.memory(|m| m.has_focus(grid_search_id())));
        assert_eq!(names(&h).len(), 1);
        // Ctrl+F y retourne ; Échap efface le filtre.
        press(&mut h, Key::F, Modifiers::CTRL);
        assert!(h.ctx.memory(|m| m.has_focus(grid_search_id())));
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(names(&h).len(), 3, "Échap : filtre effacé");
        assert!(h.state().name_query.is_empty());
        // En plein écran, `/` reste « ajusté » (pas de recherche).
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
        h.event(UiEvent::Text("/".into()));
        h.run_steps(3);
        assert_eq!(h.state().mode, Mode::View);
        assert!(!h.ctx.memory(|m| m.has_focus(grid_search_id())));
    }

    #[test]
    fn slash_searches_folders_from_the_pointed_one() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        for d in ["séances/2026/2026-10-17_Dehors", "séances/2025/Plage"] {
            std::fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let place = tri::places::Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: tri::places::PlaceKind::Home,
        };
        h.state_mut().tree = Tree::with_places(vec![place], false);
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        h.state_mut().tree.point_at(dir.path());
        h.event(UiEvent::Text("/".into()));
        h.run_steps(3);
        assert!(
            h.ctx.memory(|m| m.has_focus(folder_search_id())),
            "champ des dossiers"
        );
        h.event(UiEvent::Text("dehors".into()));
        run_until(&mut h, |app| {
            app.folder_search.as_ref().is_some_and(|s| s.is_done())
                && !app.folder_results.is_empty()
        });
        h.run_steps(2);
        let target = dir.path().join("séances/2026/2026-10-17_Dehors");
        assert_eq!(h.state().folder_results[0].0, target);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(
            h.state().folder.dir,
            target,
            "Entrée ouvre le dossier trouvé"
        );
        assert!(h.state().folder_search.is_none(), "recherche refermée");
        // Échap referme une recherche en cours, l'arborescence revient.
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        press(&mut h, Key::F, Modifiers::CTRL);
        h.event(UiEvent::Text("plage".into()));
        h.run_steps(3);
        assert!(h.state().folder_search.is_some());
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().folder_search.is_none() && h.state().folder_query.is_empty());
    }

    #[test]
    fn changes_made_elsewhere_show_up_by_themselves() {
        let (mut h, dir) = harness();
        h.state_mut().settings.watch.interval_ms = 20;
        h.run_steps(3);
        let photos = dir.path().join("photos");
        let wait = |h: &mut Harness<'static, TriApp>, done: &dyn Fn(&TriApp) -> bool| {
            for _ in 0..250 {
                h.run_steps(1);
                if done(h.state()) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        };
        // Une image copiée dans le dossier (déchargement, export…) : relu.
        image::RgbImage::from_pixel(64, 48, image::Rgb([9, 9, 9]))
            .save(photos.join("img3.jpg"))
            .unwrap();
        assert!(
            wait(&mut h, &|app| app.files.len() == 4),
            "image ajoutée vue"
        );
        assert!(
            h.state()
                .notice
                .as_ref()
                .unwrap()
                .0
                .contains("dossier relu")
        );
        // Notes posées par un autre logiciel (darktable) : relues.
        let img0 = photos.join("img0.jpg");
        let three = tri::xmp::Notes {
            stars: 3,
            ..Default::default()
        };
        tri::xmp::write_sidecar(&photos.join("img0.jpg.xmp"), &three).unwrap();
        assert!(
            wait(&mut h, &|app| app.folder.notes.notes(&img0).stars == 3),
            "notes relues"
        );
        // L'avis peut arriver une image plus tard (CI Windows).
        assert!(
            wait(&mut h, &|app| app
                .notice
                .as_ref()
                .is_some_and(|n| n.0.contains("modifiées ailleurs"))),
            "avis des notes modifiées ailleurs"
        );
    }

    #[test]
    fn favorites_are_toggled_from_the_panel_and_follow_renames() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let seance = dir.path().join("séance");
        std::fs::create_dir(&seance).unwrap();
        let place = tri::places::Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: tri::places::PlaceKind::Home,
        };
        h.state_mut().tree = Tree::with_places(vec![place], false);
        h.state_mut().tree.expand(dir.path());
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        h.state_mut().tree.point_at(&seance);
        // m : favori, dans sa section après le dossier personnel ; le curseur reste dans
        // l'arborescence.
        press(&mut h, Key::M, Modifiers::NONE);
        let data = dir.path().join("data");
        assert_eq!(
            tri::places::favorites(&data),
            std::slice::from_ref(&seance),
            "écrit"
        );
        let rows = h.state().tree.rows();
        let favorite = |rows: &[tri::places::Row]| {
            rows.iter()
                .position(|r| r.kind == Some(PlaceKind::Favorite))
        };
        let f = favorite(&rows).expect("favori listé");
        assert_eq!(rows[f].path, seance);
        assert_eq!(rows[f].section, tri::places::Section::Favorites);
        let cursor = h.state().tree.cursor().unwrap();
        assert!(
            cursor < f && rows[cursor].path == seance,
            "sous le dossier personnel"
        );
        h.run_steps(2);
        assert!(h.query_by_label_contains("ajouté aux favoris").is_some());
        // Renommé depuis le panneau : le favori suit.
        press(&mut h, Key::R, Modifiers::NONE);
        h.event(UiEvent::Text("séance du jour".into()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        let renamed = dir.path().join("séance du jour");
        assert_eq!(
            tri::places::favorites(&data),
            std::slice::from_ref(&renamed)
        );
        let rows = h.state().tree.rows();
        let f = favorite(&rows).expect("favori listé");
        assert_eq!(rows[f].path, renamed);
        // n sur la ligne du favori : nommé (22/10/2026), dans sa section seulement.
        h.state_mut().tree.set_cursor(f);
        press(&mut h, Key::N, Modifiers::NONE);
        h.event(UiEvent::Text("Mon favori".into()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(
            tri::places::favorite_names(&data)[&renamed],
            "Mon favori",
            "écrit"
        );
        let rows = h.state().tree.rows();
        let f = favorite(&rows).expect("favori listé");
        assert_eq!(rows[f].name, "Mon favori");
        assert!(
            rows.iter()
                .any(|r| r.path == renamed && r.name == "séance du jour"),
            "le vrai dossier garde son nom"
        );
        // Déplier le favori ne déplie pas le vrai dossier.
        h.state_mut().tree.set_cursor(f);
        press(&mut h, Key::L, Modifiers::NONE);
        assert!(h.state().tree.rows()[f].expanded);
        assert!(!h.state().tree.is_expanded(&renamed));
        // m sur la ligne du favori : retiré.
        h.state_mut().tree.set_cursor(f);
        press(&mut h, Key::M, Modifiers::NONE);
        assert!(tri::places::favorites(&data).is_empty());
        assert_eq!(favorite(&h.state().tree.rows()), None);
    }

    #[test]
    fn folders_are_created_renamed_deleted_and_undone_from_the_panel() {
        use crate::actions::Scope;
        let (mut h, dir) = harness();
        h.run_steps(3);
        let base = dir.path().join("base");
        std::fs::create_dir(&base).unwrap();
        let place = tri::places::Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: tri::places::PlaceKind::Home,
        };
        h.state_mut().tree = Tree::with_places(vec![place], false);
        h.state_mut().tree.expand(dir.path());
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Folders);
        h.state_mut().tree.point_at(&base);
        // a : nouveau dossier dans « base ».
        press(&mut h, Key::A, Modifiers::NONE);
        h.event(UiEvent::Text("Séance".into()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        let made = base.join("Séance");
        assert!(made.is_dir());
        assert_eq!(h.state().tree.cursor_path(), Some(made.clone()), "pointé");
        // r : renommer ; le nom actuel est sélectionné, on tape le nouveau.
        press(&mut h, Key::R, Modifiers::NONE);
        h.event(UiEvent::Text("Séance du jour".into()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        let renamed = base.join("Séance du jour");
        assert!(renamed.is_dir() && !made.exists());
        // Annuler deux fois : renommage puis création défaits.
        press(&mut h, Key::U, Modifiers::NONE);
        assert!(made.is_dir() && !renamed.exists());
        press(&mut h, Key::U, Modifiers::NONE);
        assert!(!made.exists());
        // x : supprimer (confirmé ; corbeille désactivée dans les tests).
        h.state_mut().tree.point_at(&base);
        press(&mut h, Key::X, Modifiers::NONE);
        assert!(h.query_by_label_contains("Dossier vide").is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(!base.exists());
        // À la souris : clic droit sur un dossier, « Nouveau dossier… ».
        let other = dir.path().join("autre");
        std::fs::create_dir(&other).unwrap();
        h.state_mut().tree.reload();
        h.run_steps(2);
        h.get_by_label("autre").click_secondary();
        h.run_steps(2);
        h.get_by_label("Nouveau dossier…").click();
        h.run_steps(2);
        assert!(
            matches!(
                &h.state().file_dialog,
                Some(crate::files::FileDialog::NewFolder { parent, .. }) if *parent == other
            ),
            "boîte du nouveau dossier, dans le dossier visé"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        // Un emplacement ne se supprime pas.
        h.state_mut().tree.set_cursor(0);
        press(&mut h, Key::X, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        assert!(h.state().notice.as_ref().unwrap().0.contains("emplacement"));
    }

    #[test]
    fn a_folder_is_moved_from_the_panel_and_the_menu_adds_favorites() {
        use crate::actions::Scope;
        let (mut h, dir) = harness();
        h.run_steps(3);
        let seance = dir.path().join("séance");
        let archive = dir.path().join("archive");
        std::fs::create_dir_all(seance.join("JPEG")).unwrap();
        std::fs::create_dir(&archive).unwrap();
        std::fs::write(seance.join("JPEG/A.JPG"), b"x").unwrap();
        let place = tri::places::Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: tri::places::PlaceKind::Home,
        };
        h.state_mut().tree = Tree::with_places(vec![place], false);
        h.state_mut().tree.expand(dir.path());
        press(&mut h, Key::B, Modifiers::CTRL);
        press(&mut h, Key::H, Modifiers::NONE);
        assert_eq!(h.state().focus, Scope::Folders);
        h.state_mut().tree.point_at(&seance);
        // Ctrl+X : déplacer le dossier (pas les images) ; destination tapée, Entrée.
        press(&mut h, Key::X, Modifiers::CTRL);
        h.run_steps(2);
        assert!(
            h.query_by_label_contains("Dossier « séance » vers")
                .is_some()
        );
        h.event(UiEvent::Text(archive.display().to_string()));
        h.run_steps(2);
        press(&mut h, Key::Enter, Modifiers::NONE);
        let moved = archive.join("séance");
        for _ in 0..200 {
            if h.state().file_dialog.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.run_steps(1);
        }
        assert!(h.state().file_dialog.is_none(), "déplacement terminé");
        assert!(moved.join("JPEG/A.JPG").exists() && !seance.exists());
        assert_eq!(h.state().tree.cursor_path(), Some(moved.clone()), "pointé");
        // Annulé : le dossier revient.
        press(&mut h, Key::U, Modifiers::NONE);
        assert!(seance.join("JPEG/A.JPG").exists() && !moved.exists());
        // Clic droit, « Ajouter aux favoris » : ajouté (ouvrait la boîte de suppression).
        h.state_mut().tree.reload();
        h.run_steps(2);
        h.get_by_label("séance").click_secondary();
        h.run_steps(2);
        h.get_by_label("Ajouter aux favoris").click();
        h.run_steps(2);
        assert!(
            h.state().file_dialog.is_none(),
            "pas de boîte de suppression"
        );
        let data = dir.path().join("data");
        assert_eq!(tri::places::favorites(&data), std::slice::from_ref(&seance));
        // Clic droit, « Déplacer vers… » : la boîte du dossier visé.
        h.get_by_label("archive").click_secondary();
        h.run_steps(2);
        h.get_by_label("Déplacer vers…").click();
        h.run_steps(2);
        assert!(
            h.query_by_label_contains("Dossier « archive » vers")
                .is_some()
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
    }

    #[test]
    fn the_grouping_dialog_gives_the_keyboard_to_its_checkbox() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert!(!h.state().stacking);
        press(&mut h, Key::G, Modifiers::CTRL);
        assert!(h.state().group_dialog);
        // Espace coche aussitôt (plus de Tab pour atteindre la case).
        press(&mut h, Key::Space, Modifiers::NONE);
        assert!(h.state().stacking, "Ctrl+G puis Espace : piles actives");
        assert!(
            h.state().selection.is_empty(),
            "Espace n'a pas sélectionné l'image"
        );
    }

    #[test]
    fn focus_mode_hides_everything_and_gives_it_back() {
        let icon = window_icon();
        assert_eq!(
            (icon.width, icon.height),
            (256, 256),
            "icône embarquée lisible"
        );
        let (mut h, _dir) = harness();
        h.run_steps(3);
        assert!(h.state().gui.title_bar, "barre du haut");
        let before = (
            h.state().folder_panel,
            h.state().metadata_panel,
            h.state().filter_bar,
        );
        press(&mut h, Key::F11, Modifiers::NONE);
        assert!(h.state().focus_mode && h.state().focus_mode_since.is_some());
        assert!(
            h.query_by_label_contains("Dossiers").is_none(),
            "panneaux masqués"
        );
        assert!(
            h.query_by_label_contains("Mode concentration").is_some(),
            "indication"
        );
        // Ctrl+Maj+F aussi ; les panneaux reviennent tels quels.
        press(&mut h, Key::F, Modifiers::CTRL | Modifiers::SHIFT);
        assert!(!h.state().focus_mode);
        let after = (
            h.state().folder_panel,
            h.state().metadata_panel,
            h.state().filter_bar,
        );
        assert_eq!(after, before);
    }

    #[test]
    fn the_grid_starts_without_stacks_unless_asked() {
        assert!(!crate::settings::GridSettings::default().stacks);
        let (h, _dir) = harness();
        assert!(!h.state().stacking, "grille non groupée par défaut");
        // Jamais les données de l'utilisateur : dossier temporaire, rien de rouvert.
        let dir = tempfile::tempdir().unwrap();
        let mut settings = AppSettings::default();
        settings.data.dir = Some(dir.path().join("data"));
        settings.cache.dir = Some(dir.path().join("cache"));
        settings.gui.reopen_last = false;
        settings.gui.grid.stacks = true;
        let app = TriApp::new(None, settings, None);
        assert!(app.stacking, "[gui.grid] stacks = true");
    }

    #[test]
    fn dw_and_yw_act_on_the_whole_stack() {
        let ramp = |tint: u8| {
            image::RgbImage::from_fn(64, 48, |x, _| image::Rgb([(x * tint as u32 / 63) as u8; 3]))
        };
        let images = vec![
            ("a1.jpg", ramp(255)),
            ("a2.jpg", ramp(250)),
            ("a3.jpg", ramp(245)),
        ];
        let (mut h, _dir) = harness_images(images);
        h.state_mut().stacking = true;
        let files = h.state().files.clone();
        let preview = tri::preview::Settings::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        for f in &files {
            let sig = tri::preview::compute_signature(f, &preview, &cancel).unwrap();
            h.state_mut().signatures.insert(f.path.clone(), sig);
        }
        h.state_mut().stacks_dirty = true;
        // Délai de la touche ambiguë large : `y` attend le `w`.
        h.state_mut().sequencer = crate::actions::Sequencer::new(Duration::from_secs(60))
            .with_ambiguous(Duration::from_secs(60));
        h.run_steps(3);
        assert_eq!(h.state().stacks.count(), 1);
        // d w : corbeille pour les 3 images de la pile, cachées comprises.
        press(&mut h, Key::D, Modifiers::NONE);
        press(&mut h, Key::W, Modifiers::NONE);
        assert!(
            h.query_by_label_contains("3 fichiers").is_some(),
            "suppression de la pile"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        // y w : copie de la pile.
        press(&mut h, Key::Y, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none(), "y attend la suite");
        press(&mut h, Key::W, Modifiers::NONE);
        assert!(h.query_by_label_contains("3 images vers").is_some());
        press(&mut h, Key::Escape, Modifiers::NONE);
        // Piles désactivées : expliqué, rien d'autre.
        h.state_mut().stacking = false;
        press(&mut h, Key::D, Modifiers::NONE);
        press(&mut h, Key::W, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        assert!(
            h.state()
                .notice
                .as_ref()
                .unwrap()
                .0
                .contains("Piles désactivées")
        );
    }

    #[test]
    fn similar_images_stack_and_unstack() {
        // Deux rafales de 3 images : des dégradés horizontaux, puis verticaux, de teintes
        // différentes ; entre elles, rien de commun.
        let ramp = |vertical: bool, tint: [u8; 3]| {
            image::RgbImage::from_fn(64, 48, |x, y| {
                let v = if vertical { y * 255 / 47 } else { x * 255 / 63 } as u16;
                image::Rgb(tint.map(|c| (v * c as u16 / 255) as u8))
            })
        };
        let images = vec![
            ("a1.jpg", ramp(false, [255, 255, 255])),
            ("a2.jpg", ramp(false, [250, 250, 250])),
            ("a3.jpg", ramp(false, [245, 245, 245])),
            ("b1.jpg", ramp(true, [255, 80, 20])),
            ("b2.jpg", ramp(true, [250, 80, 20])),
            ("b3.jpg", ramp(true, [245, 80, 20])),
        ];
        let (mut h, _dir) = harness_images(images);
        h.state_mut().stacking = true;
        h.state_mut().settings.similarity.background = true;
        // Empreintes calculées directement (le moteur du banc de test ne les fait pas).
        let files = h.state().files.clone();
        let preview = tri::preview::Settings::default();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        for f in &files {
            let sig = tri::preview::compute_signature(f, &preview, &cancel).unwrap();
            h.state_mut().signatures.insert(f.path.clone(), sig);
        }
        h.state_mut().stacks_dirty = true;
        h.run_steps(3);
        assert_eq!(h.state().stacks.shown.len(), 2, "deux piles repliées");
        assert_eq!(h.state().stacks.count(), 2);
        // La flèche droite saute les images cachées : de la première pile à la seconde.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert!(selected_name(&h).starts_with('b'), "{}", selected_name(&h));
        // s déplie la pile ; Maj+S déplie / replie tout.
        press(&mut h, Key::S, Modifiers::NONE);
        assert_eq!(h.state().stacks.shown.len(), 4);
        press(&mut h, Key::S, Modifiers::SHIFT);
        assert_eq!(h.state().stacks.shown.len(), 2, "tout replié");
        press(&mut h, Key::S, Modifiers::SHIFT);
        assert_eq!(h.state().stacks.shown.len(), 6, "tout déplié");
        press(&mut h, Key::S, Modifiers::SHIFT);
        h.state_mut().expanded.clear();
        h.state_mut().stacks_dirty = true;
        h.run_steps(2);
        // Plein écran, mode pile (Ctrl+S) : → dans la pile, arrêt au bout ; ↓ la pile
        // suivante ; Entrée choisit la préférée, qui devient la couverture.
        let pos = |name: &str| files.iter().position(|f| f.path.ends_with(name)).unwrap();
        h.state_mut().select(pos("a1.jpg"));
        h.state_mut().chosen.clear();
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
        assert!(h.state().stacking, "piles actives : en plein écran aussi");
        let start = h.state().selected;
        let range = h.state().stacks.range_of(start).cloned().unwrap();
        for _ in 0..5 {
            press(&mut h, Key::ArrowRight, Modifiers::NONE);
        }
        assert_eq!(h.state().selected, range.end - 1, "arrêt en fin de pile");
        assert!(h.query_by_label_contains("pile : 3 / 3").is_some());
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(
            h.state().stacks.cover_of(start),
            Some(range.end - 1),
            "préférée"
        );
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        assert!(selected_name(&h).starts_with('b'), "pile suivante");
        press(&mut h, Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.state().selected, range.end - 1, "retour : sa couverture");
        // Ctrl+S : plus de piles ; → passe à la photo suivante, toutes visibles.
        press(&mut h, Key::S, Modifiers::CTRL);
        assert!(!h.state().stacking);
        h.run_steps(2);
        let before = h.state().selected;
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, before + 1, "de photo en photo");
        press(&mut h, Key::S, Modifiers::CTRL);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::Grid);
        // Ctrl+G : la fenêtre ; seuil nul : plus de piles.
        press(&mut h, Key::G, Modifiers::CTRL);
        assert!(h.state().group_dialog);
        h.state_mut().settings.similarity.auto = false;
        h.state_mut().settings.similarity.threshold = 0.0;
        h.state_mut().stacks_dirty = true;
        h.run_steps(2);
        assert_eq!(
            h.state().stacks.count(),
            0,
            "seuil nul : chaque image seule"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().group_dialog);
    }

    #[test]
    fn zoom_steps_pinch_and_loupe() {
        let (mut h, _dir) = harness_with(4000, 3000);
        h.run_steps(3);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.last_shown.1.is_some());
        // + : palier au-dessus de l'ajusté ; * : 100 % ; - : palier en dessous ; / : ajusté.
        h.event(UiEvent::Text("+".into()));
        h.run_steps(3);
        assert!(h.state().zoom.is_some(), "zoomé");
        let first = h.state().zoom_scale;
        assert!(
            h.state().gui.zoom.steps.contains(&first),
            "un palier : {first}"
        );
        h.event(UiEvent::Text("*".into()));
        h.run_steps(3);
        assert_eq!(h.state().zoom_scale, 1.0);
        // Dans la barre d'état (et le bouton 1:1).
        assert!(
            h.query_all_by_label_contains("100 %").next().is_some(),
            "affiché"
        );
        h.event(UiEvent::Text("-".into()));
        h.run_steps(3);
        assert!(h.state().zoom_scale < 1.0 || h.state().zoom.is_none());
        h.event(UiEvent::Text("/".into()));
        h.run_steps(3);
        assert!(h.state().zoom.is_none(), "ajusté");
        // - depuis l'ajusté : sous l'ajusté, l'image plus petite que l'écran.
        h.event(UiEvent::Text("-".into()));
        h.run_steps(3);
        assert!(h.state().zoom.is_some(), "dézoomé");
        assert!(h.state().zoom_scale < h.state().fit_scale, "sous l'ajusté");
        h.event(UiEvent::Text("/".into()));
        h.run_steps(3);
        // Pincer au-dessus de l'image : zoom continu.
        let center = h.ctx.content_rect().center();
        h.hover_at(center);
        h.event(UiEvent::Zoom(3.0));
        h.run_steps(3);
        assert!(h.state().zoom.is_some(), "pincé : zoomé");
        let pinched = h.state().zoom_scale;
        assert!(
            pinched > h.state().fit_scale,
            "au-delà de l'ajusté : {pinched}"
        );
        h.event(UiEvent::Zoom(1000.0));
        h.run_steps(3);
        assert_eq!(h.state().zoom_scale, h.state().gui.zoom.max, "plafonné");
        // Loupe : `t` maintenue (défaut), en vue ajustée ; Maj ne la montre plus.
        press(&mut h, Key::Z, Modifiers::NONE);
        if h.state().zoom.is_some() {
            press(&mut h, Key::Z, Modifiers::NONE);
        }
        h.state_mut().gui.zoom.loupe_delay_ms = 0;
        h.event(UiEvent::ModifiersChanged(Modifiers::SHIFT));
        h.hover_at(center + vec2(1.0, 0.0));
        h.run_steps(3);
        assert!(!h.state().loupe_on, "Maj : plus de loupe");
        h.event(UiEvent::ModifiersChanged(Modifiers::NONE));
        let key = |pressed| UiEvent::Key {
            key: Key::T,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        h.event(key(true));
        h.run_steps(3);
        assert!(h.state().loupe_on, "loupe");
        h.event(key(false));
        h.run_steps(3);
        assert!(!h.state().loupe_on, "lâchée");
    }

    #[test]
    fn help_shows_labels_not_ids() {
        let (mut h, _dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::F1, Modifiers::NONE);
        assert!(h.query_by_label("Ligne du dessous").is_some());
        // Image suivante, et déplier dans le panneau des dossiers.
        assert!(h.query_all_by_label("Flèche droite / l").count() >= 1);
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
    // Ligne de l'appel dans le message d'échec : savoir ce qui n'est jamais arrivé.
    #[track_caller]
    fn run_until(h: &mut Harness<'_, TriApp>, done: impl Fn(&TriApp) -> bool) {
        // Large : les machines d'intégration continue sont plus lentes (20 s ne suffisaient
        // pas au zoom sur une image de 4000 × 3000) ; rapide quand tout va bien.
        let deadline = Instant::now() + Duration::from_secs(60);
        while !done(h.state()) {
            assert!(Instant::now() < deadline, "délai dépassé");
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Attend qu'un élément de libellé `label` soit affiché (une fenêtre egui peut demander
    /// une image de plus pour apparaître, plus sur une machine d'intégration continue lente).
    #[track_caller]
    fn wait_for_label(h: &mut Harness<'_, TriApp>, label: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while h.query_by_label(label).is_none() {
            assert!(Instant::now() < deadline, "« {label} » jamais affiché");
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
        // Clic sur le badge : l'image choisie, le grand histogramme ouvert.
        assert!(!h.state().histogram_large);
        let percent = tri::i18n::decimal((over * 1000.0).round() / 10.0, 1);
        h.get_by_label(&t!("badge-over", percent = percent.clone()))
            .click();
        h.run_steps(3);
        assert!(h.state().histogram_large);
        assert_eq!(h.state().selected, white);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().histogram_large);
        // f w : bien exposées, l'image blanche part (et les aplats à 0 sur un canal,
        // bouchés) ; f i : seulement elles ; l'entonnoir : tout.
        let n = h.state().files.len();
        let hist = h.state().gui.histogram;
        let badly: Vec<PathBuf> = h
            .state()
            .files
            .iter()
            .filter(|f| {
                let (o, u) = h.state().clip_summary[&f.path];
                o > hist.badge_over || u > hist.badge_under
            })
            .map(|f| f.path.clone())
            .collect();
        assert!(badly.contains(&white_path));
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::W, Modifiers::NONE);
        assert!(h.state().well_exposed);
        assert_eq!(h.state().files.len(), n - badly.len());
        // Plus de refiltrage ensuite (issue n° 10 : refiltrée sans fin, la grille revenait
        // sans cesse à la sélection et ne défilait plus).
        let generation = h.state().engine.as_ref().unwrap().generation;
        for _ in 0..30 {
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(h.state().engine.as_ref().unwrap().generation, generation);
        assert!(!h.state().files.iter().any(|f| badly.contains(&f.path)));
        press(&mut h, Key::F, Modifiers::NONE);
        press(&mut h, Key::I, Modifiers::NONE);
        let shown: Vec<_> = h.state().files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(shown, badly, "inversé : les mal exposées");
        h.get_by_label("Effacer").click();
        h.run_steps(3);
        assert!(!h.state().well_exposed && !h.state().filter_inverted);
        assert_eq!(h.state().files.len(), n);
        let white = h
            .state()
            .files
            .iter()
            .position(|f| f.path == white_path)
            .unwrap();
        // Plein écran sur l'image blanche : histogramme et zones.
        h.state_mut().select(white);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| app.exposures.contains(&white_path));
        // Badge de l'écrêtage dans le bandeau aussi (issue n° 9) : grand histogramme.
        h.run_steps(2);
        assert!(!h.state().histogram_large);
        h.get_by_label(&t!("badge-over", percent = percent)).click();
        h.run_steps(3);
        assert!(
            h.state().histogram_large,
            "clic sur le badge du plein écran"
        );
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(h.state().mode, Mode::View);
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
        press(&mut h, Key::W, Modifiers::NONE);
        assert!(h.state().clipping, "w : zones");
        // Masque calculé en fond : deux images ne suffisaient pas toujours (CI Windows).
        run_until(&mut h, |app| app.mask_textures.contains(&white_path));
        // Maj+C : grand histogramme ouvert sur le canal rouge, image d'analyse ; puis
        // vert, bleu, tous ; Échap ferme.
        press(&mut h, Key::C, Modifiers::SHIFT);
        assert!(
            h.state().histogram_large,
            "Maj+C ouvre le grand histogramme"
        );
        assert_eq!(h.state().histogram_channel, Some(0));
        // L'image d'analyse de tous les canaux peut encore être là (CI macOS) : attendre
        // celle du rouge.
        run_until(&mut h, |app| {
            app.analysis_texture
                .as_ref()
                .is_some_and(|t| t.0.1 == Some(0))
        });
        assert_eq!(
            h.state().analysis_texture.as_ref().unwrap().0,
            (white_path.clone(), Some(0))
        );
        for expected in [Some(1), Some(2), None] {
            press(&mut h, Key::C, Modifiers::SHIFT);
            assert_eq!(h.state().histogram_channel, expected);
        }
        // r, g, b, a : un canal, ou tous ; b ne change pas la source, g ne commence pas gg.
        let source = h.state().source;
        for (key, expected) in [
            (Key::G, Some(1)),
            (Key::B, Some(2)),
            (Key::R, Some(0)),
            (Key::A, None),
        ] {
            press(&mut h, key, Modifiers::NONE);
            assert_eq!(h.state().histogram_channel, expected, "{key:?}");
        }
        assert_eq!(h.state().source, source);
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
        // La première de la fenêtre : `[gui] auto_advance` (la recherche floue en trouve
        // d'autres, dont le texte contient ces lettres dans l'ordre) ; pas la case
        // « Sous-dossiers » du panneau des dossiers, derrière.
        let panel = h.state().gui.folders.width;
        let rect = h
            .query_all_by_role(egui::accesskit::Role::CheckBox)
            .map(|n| n.rect())
            .find(|r| r.left() > panel)
            .expect("une case");
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
        // Calculs de fond finis : leurs barres de progression, en apparaissant ou en
        // disparaissant, déplaceraient les icônes entre deux clics.
        run_until(&mut h, |app| app.progress.iter().all(|p| p.done >= p.total));
        h.run_steps(2);
        // Un clic, recommencé si l'icône a bougé entre-temps (suite de tests chargée : un
        // calcul de fond qui finit change la barre d'état), jusqu'à ce que `state` change.
        let click = |h: &mut Harness<'_, TriApp>, what: &str, state: fn(&TriApp) -> bool| {
            let before = state(h.state());
            for _ in 0..5 {
                h.get_by_label_contains(what).click();
                h.run_steps(2);
                if state(h.state()) != before {
                    break;
                }
            }
        };
        assert!(h.state().folder_panel && h.state().metadata_panel);
        click(&mut h, "panneau des dossiers", |a| a.folder_panel);
        assert!(!h.state().folder_panel);
        click(&mut h, "panneau des métadonnées", |a| a.metadata_panel);
        assert!(!h.state().metadata_panel);
        click(&mut h, "barre de filtres", |a| a.filter_bar);
        assert!(!h.state().filter_bar);
        click(&mut h, "bandeau de notation", |a| a.rating_bar);
        assert!(!h.state().rating_bar);
        click(&mut h, "panneau des dossiers", |a| a.folder_panel);
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
        // Recliqué tant que la séance n'est pas ouverte (CI Linux : le clic, à côté du bilan
        // qui venait de s'afficher, se perdait parfois).
        while h.state().folder.dir == before {
            assert!(
                Instant::now() < deadline,
                "séance jamais ouverte à la demande"
            );
            h.get_by_label_contains("Ouvrir la séance").click();
            h.run_steps(3);
        }
        assert_eq!(names(&h), ["IMG0.jpg"]);
        assert!(h.state().offloaded.is_none());
    }

    #[test]
    fn the_large_histogram_opens_and_closes() {
        let (mut h, _dir) = harness();
        press(&mut h, Key::E, Modifiers::SHIFT);
        assert!(h.state().histogram_large, "Maj+E l'ouvre");
        // L'échelle est une icône dans l'histogramme (19/10/2026), plus un bouton texte
        // à côté de « Fermer ».
        assert!(h.query_by_label("Échelle linéaire").is_none());
        // p : photo d'analyse affichée / masquée (masquée par défaut).
        assert!(!h.state().histogram_photo);
        press(&mut h, Key::P, Modifiers::NONE);
        assert!(h.state().histogram_photo, "p la montre");
        press(&mut h, Key::P, Modifiers::NONE);
        assert!(!h.state().histogram_photo, "p la masque");
        press(&mut h, Key::I, Modifiers::SHIFT);
        assert_eq!(
            h.state().histogram_scale,
            HistogramScale::Linear,
            "Maj+I dans la boîte"
        );
        // Les déplacements passent sous la boîte : l'histogramme suit l'image courante.
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 1, "→ : image suivante");
        assert!(h.state().histogram_large, "boîte toujours ouverte");
        press(&mut h, Key::H, Modifiers::NONE);
        assert_eq!(h.state().selected, 0, "h aussi");
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
            "rendu boîtier par défaut"
        );
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(h.state().source, Source::Raw);
        press(&mut h, Key::B, Modifiers::NONE);
        assert_eq!(
            h.state().source,
            Source::Embedded,
            "sans darktable : deux sources"
        );
        // darktable là, mais rien de développé : b ne lance jamais de rendu (10 s pour
        // rien) et reste boîtier ↔ RAW.
        h.state_mut().darktable = true;
        for expected in [Source::Raw, Source::Embedded] {
            press(&mut h, Key::B, Modifiers::NONE);
            assert_eq!(h.state().source, expected);
        }
        assert!(
            h.state()
                .requested
                .iter()
                .all(|(_, l)| *l != Level::Developed)
        );
        // Développé (fait plus tôt, ou retrouvé en cache) : il entre dans la rotation.
        let key = {
            let app = h.state();
            let engine = app.engine.as_ref().unwrap();
            let key = tri::preview::key(&app.files[0], Level::Developed, &engine.preview).unwrap();
            engine.cache.put_bytes(&key, b"jpeg").unwrap();
            key
        };
        set(&mut h.state_mut().status, 0, Level::Developed, Avail::Ready);
        for expected in [Source::Raw, Source::Developed, Source::Embedded] {
            press(&mut h, Key::B, Modifiers::NONE);
            assert_eq!(h.state().source, expected);
        }
        // Développement modifié dans darktable (autre clé : le cache ne l'a plus) : oublié,
        // b ne le montre plus (retour du 19/10/2026 : l'ancien recadrage restait).
        h.state()
            .engine
            .as_ref()
            .unwrap()
            .cache
            .remove(&key)
            .unwrap();
        for expected in [Source::Raw, Source::Embedded] {
            press(&mut h, Key::B, Modifiers::NONE);
            assert_eq!(h.state().source, expected);
        }
        assert_eq!(h.state().status[0].developed, Avail::Pending);
    }

    #[test]
    fn ctrl_e_offers_the_export_profiles_and_remembers_the_choice() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        press(&mut h, Key::E, Modifiers::CTRL);
        assert!(matches!(
            h.state().file_dialog,
            Some(crate::files::FileDialog::Export(_))
        ));
        // Sans darktable (tests) : rendu de base par LibRaw, dit dans la boîte.
        h.get_by_label_contains("darktable absent");
        // Des JPEG : « JPEG tel quel » proposé d'abord (liste déroulante).
        let Some(crate::files::FileDialog::Export(dialog)) = &h.state().file_dialog else {
            panic!("boîte d'export attendue");
        };
        assert_eq!(dialog.choice(), 1);
        // ↓ ↓ (sans ouvrir la liste) : profil Web ; Entrée : export en fond, puis bilan.
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        let Some(crate::files::FileDialog::Export(dialog)) = &h.state().file_dialog else {
            panic!("boîte d'export attendue");
        };
        assert_eq!(dialog.choice(), 2, "Web");
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        run_until(&mut h, |app| {
            app.export_job.is_none() && app.notice.is_some()
        });
        let notice = h.state().notice.clone().unwrap();
        assert!(notice.0.contains("1 image exportée"), "{notice:?}");
        assert!(dir.path().join("photos/export/img0.jpg").is_file());
        // Rapport d'export : gardé dans le dossier de données, bouton à côté du bilan.
        let data = h
            .state()
            .data_dir
            .clone()
            .expect("dossier de données des tests");
        assert!(data.starts_with(dir.path()), "jamais les vraies données");
        let log = tri::export_log::read(&data);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].written.len(), 1);
        assert_eq!(h.state().export_log, log);
        // Clic recommencé si le bouton a bougé (barre d'état : calculs de fond qui finissent).
        run_until(&mut h, |app| app.progress.iter().all(|p| p.done >= p.total));
        // Image où l'export a fini : barre d'état dessinée avant le bilan ; une de plus.
        wait_for_label(&mut h, "Rapport d'export");
        for _ in 0..5 {
            h.get_by_label("Rapport d'export").click();
            h.run_steps(2);
            if h.state().export_report {
                break;
            }
        }
        assert!(h.state().export_report);
        wait_for_label(&mut h, "Effacer la liste");
        h.get_by_label("Effacer la liste").click();
        h.run_steps(2);
        assert!(h.state().export_log.is_empty());
        assert!(tri::export_log::read(&data).is_empty(), "effacé du disque");
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(!h.state().export_report);
        press(&mut h, Key::E, Modifiers::CTRL | Modifiers::SHIFT);
        assert!(h.state().export_report, "Ctrl+Maj+E");
        press(&mut h, Key::E, Modifiers::CTRL | Modifiers::SHIFT);
        assert!(!h.state().export_report);
        // Rouverte : le profil choisi la dernière fois.
        press(&mut h, Key::E, Modifiers::CTRL);
        let Some(crate::files::FileDialog::Export(dialog)) = &h.state().file_dialog else {
            panic!("boîte d'export attendue");
        };
        assert_eq!(dialog.choice(), 2);
        // En grille, sans darktable : rendu LibRaw.
        assert_eq!(dialog.render(), crate::export::Render::LibRaw);
        // JPEG, rendu local : la case « sans recompression » est proposée, cochée.
        h.get_by_label("Sans recompression");
        assert!(dialog.passthrough());
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().file_dialog.is_none());
        // On exporte ce qu'on regarde : plein écran sur le JPEG boîtier, puis le rendu RAW.
        press(&mut h, Key::Enter, Modifiers::NONE);
        for (key, expected) in [
            (None, crate::export::Render::Embedded),
            (Some(Key::B), crate::export::Render::LibRaw),
        ] {
            if let Some(key) = key {
                press(&mut h, key, Modifiers::NONE);
            }
            press(&mut h, Key::E, Modifiers::CTRL);
            let Some(crate::files::FileDialog::Export(dialog)) = &h.state().file_dialog else {
                panic!("boîte d'export attendue");
            };
            assert_eq!(dialog.render(), expected);
            // Images du test : des JPEG, « tel quel » toujours proposé (un rendu de RAW ne
            // le propose pas : `what_is_exported_is_what_is_looked_at`).
            assert!(dialog.lists_as_is());
            press(&mut h, Key::Escape, Modifiers::NONE);
        }
    }

    #[test]
    fn r_crops_a_version_saved_in_the_sidecar_and_undone() {
        let (mut h, dir) = harness();
        h.run_steps(3);
        let image = h.state().files[0].path.clone();
        // r depuis la grille : plein écran et recadrage.
        press(&mut h, Key::R, Modifiers::NONE);
        h.run_steps(3);
        assert_eq!(h.state().mode, Mode::View);
        assert!(h.state().crop.is_some());
        // Image affichée : le cadre connaît ses proportions (suite de tests chargée).
        run_until(&mut h, |app| {
            app.crop.as_ref().is_some_and(crate::crop::Crop::ready)
        });
        // Rapport de l'image : « Recadrage », pas « Original », réservé à l'image non
        // recadrée dans la liste des versions (retour du 23/10/2026).
        assert_eq!(h.state().crop.as_ref().unwrap().name, "Recadrage");
        h.state_mut().crop.as_mut().unwrap().name = "original".into();
        assert!(h.state().crop.as_ref().unwrap().name_taken());
        h.state_mut().crop.as_mut().unwrap().name = "Recadrage".into();
        // 5 : 3:2 ; flèches : le cadre bouge, l'image reste la même.
        press(&mut h, Key::Num5, Modifiers::NONE);
        let before = h.state().crop.as_ref().unwrap().rect;
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.state().selected, 0, "pas d'image suivante");
        let after = h.state().crop.as_ref().unwrap().rect;
        assert!(after.y > before.y, "déplacé : {before:?} → {after:?}");
        // Entrée : enregistrée dans le sidecar, recadrage fermé.
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().crop.is_none());
        let saved = tri::versions::read(&image).unwrap();
        assert_eq!(saved.list.len(), 1);
        assert_eq!(saved.list[0].name, "3:2");
        assert_eq!(saved.list[0].ratio, tri::versions::Ratio::Fixed(3, 2));
        assert!(dir.path().join("photos/img0.jpg.xmp").is_file());
        // Montrée tout de suite, dans la liste des versions du bandeau (issue n° 5).
        assert_eq!(h.state().version_shown, Some((image.clone(), 0)));
        h.run_steps(2);
        h.get_by_role(egui::accesskit::Role::ComboBox).click();
        h.run_steps(2);
        h.get_by_label("Original").click();
        h.run_steps(2);
        assert_eq!(h.state().version_shown, None);
        h.get_by_role(egui::accesskit::Role::ComboBox).click();
        h.run_steps(2);
        h.get_by_label("3:2").click();
        h.run_steps(2);
        assert_eq!(
            h.state().version_shown,
            Some((image.clone(), 0)),
            "remontrée"
        );
        // Ctrl+E sur la version montrée : elle seule, recadrée, nommée d'après elle.
        press(&mut h, Key::E, Modifiers::CTRL);
        press(&mut h, Key::Enter, Modifiers::NONE);
        run_until(&mut h, |app| {
            app.export_job.is_none() && app.notice.is_some()
        });
        let out = dir.path().join("photos/export/img0-3_2.jpg");
        assert!(out.is_file(), "{:?}", h.state().notice);
        let (w, h2) = image::image_dimensions(&out).unwrap();
        assert!(
            (f64::from(w) / f64::from(h2) - 1.5).abs() < 0.05,
            "3:2 : {w} × {h2}"
        );
        // n : version suivante (ici la seule), puis l'original.
        press(&mut h, Key::N, Modifiers::NONE);
        assert_eq!(h.state().version_shown, None);
        press(&mut h, Key::N, Modifiers::NONE);
        assert_eq!(h.state().version_shown, Some((image.clone(), 0)));
        // Supprimer (dans la liste, annulable), puis Ctrl+Z la remet.
        h.get_by_role(egui::accesskit::Role::ComboBox).click();
        h.run_steps(2);
        h.get_by_label("Supprimer").click();
        h.run_steps(2);
        assert!(tri::versions::read(&image).unwrap().list.is_empty());
        press(&mut h, Key::Z, Modifiers::CTRL);
        assert_eq!(tri::versions::read(&image).unwrap().list.len(), 1);
        // Et Ctrl+Z encore : la création défaite.
        press(&mut h, Key::Z, Modifiers::CTRL);
        assert!(tri::versions::read(&image).unwrap().list.is_empty());
        // Échap abandonne sans rien écrire.
        press(&mut h, Key::R, Modifiers::NONE);
        press(&mut h, Key::Escape, Modifiers::NONE);
        assert!(h.state().crop.is_none());
        assert_eq!(
            h.state().mode,
            Mode::View,
            "Échap ferme le recadrage seulement"
        );
        assert!(tri::versions::read(&image).unwrap().list.is_empty());
    }

    #[test]
    fn presets_set_ratio_name_and_profile_and_new_ones_are_saved() {
        let (mut h, dir) = harness();
        let config = dir.path().join("config.toml");
        h.state_mut().config_path = Some(config.clone());
        h.run_steps(3);
        let image = h.state().files[0].path.clone();
        press(&mut h, Key::R, Modifiers::NONE);
        run_until(&mut h, |app| {
            app.crop.as_ref().is_some_and(crate::crop::Crop::ready)
        });
        // Tab : premier préréglage, « Story » (9:16, Web) ; Tab puis Maj+Tab y revient. Pas
        // de champ pris par Tab (déplacement du focus d'egui).
        press(&mut h, Key::Tab, Modifiers::NONE);
        assert!(
            !h.ctx.egui_wants_keyboard_input(),
            "Tab ne va pas dans le champ du nom"
        );
        let first = h.state().crop.as_ref().unwrap().preset.clone();
        press(&mut h, Key::Tab, Modifiers::NONE);
        assert_ne!(h.state().crop.as_ref().unwrap().preset, first);
        press(&mut h, Key::Tab, Modifiers::SHIFT);
        assert_eq!(h.state().crop.as_ref().unwrap().preset, first);
        // p : la carte de netteté, pas un préréglage.
        let peaking = h.state().peaking;
        press(&mut h, Key::P, Modifiers::NONE);
        assert_ne!(h.state().peaking, peaking);
        assert_eq!(h.state().crop.as_ref().unwrap().preset, first);
        let crop = h.state().crop.as_ref().unwrap();
        assert_eq!(crop.ratio, tri::versions::Ratio::Fixed(9, 16));
        assert_eq!(crop.name, "Story");
        assert_eq!(crop.profile.as_deref(), Some("web"));
        // Enregistrer comme préréglage : écrit dans le fichier, identifiant libre.
        h.get_by_label("Enregistrer comme préréglage de recadrage")
            .click();
        h.run_steps(2);
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("[versions.presets.story-2]"), "{text}");
        assert!(
            text.contains("name = \"Story (2)\""),
            "nom pas en double : {text}"
        );
        assert!(text.contains("ratio = \"9:16\""), "{text}");
        assert_eq!(
            h.state().settings.versions.presets.0.len(),
            3,
            "appliqué à chaud"
        );
        // Entrée : la version prend le nom et le profil du préréglage.
        press(&mut h, Key::Enter, Modifiers::NONE);
        let saved = tri::versions::read(&image).unwrap();
        assert_eq!(saved.list[0].name, "Story");
        assert_eq!(saved.list[0].profile.as_deref(), Some("web"));
        // Issue n° 14 : pas deux versions du même nom. Le préréglage propose « Story (2) » ;
        // un nom tapé déjà pris (casse comprise) n'est pas enregistré, le recadrage reste.
        press(&mut h, Key::R, Modifiers::NONE);
        run_until(&mut h, |app| {
            app.crop.as_ref().is_some_and(crate::crop::Crop::ready)
        });
        press(&mut h, Key::Tab, Modifiers::NONE);
        assert_eq!(h.state().crop.as_ref().unwrap().name, "Story (2)");
        h.state_mut().crop.as_mut().unwrap().name = "story ".into();
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().crop.is_some(), "refusée, recadrage gardé");
        assert!(h.state().notice.as_ref().is_some_and(|(_, error)| *error));
        assert_eq!(tri::versions::read(&image).unwrap().list.len(), 1);
        h.state_mut().crop.as_mut().unwrap().name = "Story bis".into();
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().crop.is_none());
        let names: Vec<String> = tri::versions::read(&image)
            .unwrap()
            .list
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(names, ["Story", "Story bis"]);
        // Reprendre une version : elle garde son nom, mais pas celui de l'autre.
        h.state_mut().edit_version(&image, 1);
        assert!(h.state().crop.as_ref().unwrap().name_ok());
        h.state_mut().crop.as_mut().unwrap().name = "Story".into();
        assert!(h.state().crop.as_ref().unwrap().name_taken());
    }

    #[test]
    fn export_profiles_follow_the_arrows_with_a_multiple_selection() {
        // Assez d'images pour que la grille défile (comme un vrai dossier de 472 images).
        let names: Vec<String> = (0..80).map(|i| format!("img{i:02}.jpg")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let (mut h, _dir) = harness_files(&names, 32, 24);
        h.run_steps(3);
        // Deux images sélectionnées (Espace), puis Ctrl+E.
        press(&mut h, Key::Space, Modifiers::NONE);
        press(&mut h, Key::ArrowRight, Modifiers::NONE);
        press(&mut h, Key::Space, Modifiers::NONE);
        assert_eq!(h.state().selection.visible(&h.state().files), 2);
        press(&mut h, Key::E, Modifiers::CTRL);
        let choice = |h: &Harness<'_, TriApp>| match &h.state().file_dialog {
            Some(crate::files::FileDialog::Export(d)) => d.choice(),
            _ => panic!("boîte d'export attendue"),
        };
        let first = choice(&h);
        press(&mut h, Key::ArrowDown, Modifiers::NONE);
        assert_eq!(choice(&h), first + 1, "flèche bas : profil suivant");
        press(&mut h, Key::ArrowUp, Modifiers::NONE);
        assert_eq!(choice(&h), first);
        assert_eq!(
            h.state().selection.visible(&h.state().files),
            2,
            "sélection gardée"
        );
        // Un clic dans la boîte (ici sur son titre, au-dessus de la grille) ne vide pas la
        // sélection.
        h.get_by_label_contains("Exporter 2 images").click();
        h.run_steps(2);
        assert_eq!(
            h.state().selection.visible(&h.state().files),
            2,
            "clic dans la boîte"
        );
    }

    #[test]
    fn an_image_without_development_says_so_and_falls_back() {
        // darktable « présent » (simulé) mais un sidecar sans historique : le rendu
        // échoue proprement, l'image reste visible (rendu RAW ou boîtier) avec la raison.
        let (mut h, _dir) = harness();
        h.run_steps(3);
        h.state_mut().darktable = true;
        press(&mut h, Key::Enter, Modifiers::NONE);
        // Maj+B : développer l'image affichée (à la demande seulement).
        press(&mut h, Key::B, Modifiers::SHIFT);
        assert_eq!(h.state().source, Source::Developed);
        run_until(&mut h, |app| {
            matches!(app.status[0].developed, Avail::Failed(_))
        });
        // Une image reste affichée (le temps de la charger : suite de tests chargée).
        run_until(&mut h, |app| app.last_shown.1.is_some());
        // La raison est donnée (affichée en surimpression à côté de la source).
        let Avail::Failed(reason) = &h.state().status[0].developed else {
            unreachable!()
        };
        assert!(reason.contains("darktable"), "{reason}");
        // Sans développement, l'image se montre dans la source d'avant, et elle zoome
        // (retour du 20/10/2026 : après un Maj+B, plus aucune image ne zoomait).
        let file = h.state().files[0].clone();
        assert_eq!(h.state().source_for(&file), Source::Embedded);
        press(&mut h, Key::Z, Modifiers::NONE);
        let level = view::zoom_level(file.kind, Source::Embedded);
        run_until(&mut h, |app| app.big.contains(&(file.path.clone(), level)));
        assert_eq!(h.state().source, Source::Developed, "le choix reste");
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
        assert_eq!(
            parse_actions("settings:keys"),
            [Step::Settings("keys".into())]
        );
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
        // Toutes les actions disponibles sur ce système (AirDrop : macOS seulement).
        let available = Action::ALL.iter().filter(|a| a.available()).count();
        assert_eq!(all.len(), available);
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
        // Rangées par thème pendant la recherche aussi : chaque thème d'un seul tenant.
        let found = help_rows(&keymap, "zoom");
        let mut seen: Vec<Group> = Vec::new();
        for (a, _) in &found {
            if seen.last() != Some(&a.group()) {
                assert!(!seen.contains(&a.group()), "{:?} coupé en deux", a.group());
                seen.push(a.group());
            }
        }
        // Mots-clés cachés : « vim » trouve le mode visuel, et seulement des actions qui
        // le contiennent tel quel (plus de lettres éparpillées).
        crate::i18n::ensure();
        let vim = help_rows(&keymap, "vim");
        assert!(
            vim.iter().any(|(a, _)| *a == Action::SelectVisual),
            "{vim:?}"
        );
        assert!(vim.len() < 10, "{vim:?}");
        // Le titre d'un thème trouve tout le thème.
        let group = Action::Stars(3).group();
        let by_group = help_rows(&keymap, &group.label());
        let count = Action::ALL
            .iter()
            .filter(|a| a.available() && a.group() == group)
            .count();
        assert!(by_group.iter().filter(|(a, _)| a.group() == group).count() == count);
    }
}
