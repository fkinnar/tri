//! Réglages de l'interface : le fichier de configuration entier, sections du moteur
//! comprises, plus la section `[gui]`.
//!
//! Même fichier que le moteur (voir `tri::settings`) ; toutes les valeurs par défaut sont
//! dans `doc/config.example.toml`, qu'un test compare au code.

use std::collections::BTreeMap;
use std::time::Duration;

use eframe::egui::{self, Color32};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use tri::scheduler::RawPreviews;
use tri::settings::{
    CacheSection, EngineSettings, PreviewSection, RenderSection, SchedulerSection, XmpSection,
};

use crate::actions::{Keymap, Profile};

/// Le fichier de configuration entier, vu par l'interface. Toute section ou clé inconnue
/// est signalée.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    /// `[preview]` (moteur).
    pub preview: PreviewSection,
    /// `[render]` (moteur).
    pub render: RenderSection,
    /// `[cache]` (moteur).
    pub cache: CacheSection,
    /// `[scheduler]` (moteur).
    pub scheduler: SchedulerSection,
    /// `[xmp]` (moteur).
    pub xmp: XmpSection,
    /// `[places]` (moteur).
    pub places: tri::places::PlaceSettings,
    /// `[watch]` (moteur) : surveillance du dossier ouvert.
    pub watch: tri::watch::WatchSettings,
    /// `[develop]` (moteur) : rendu par darktable.
    pub develop: tri::develop::DevelopSettings,
    /// `[export]` (moteur) : profils d'export, destination.
    pub export: tri::export::ExportSettings,
    /// `[versions]` (moteur) : préréglages de recadrage.
    pub versions: tri::versions::VersionSettings,
    /// `[exposure]` (moteur).
    pub exposure: tri::exposure::ExposureSettings,
    /// `[offload]` (moteur).
    pub offload: tri::offload::OffloadSettings,
    /// `[data]` (moteur).
    pub data: tri::settings::DataSection,
    /// `[files]` (moteur).
    pub files: tri::files::FileSettings,
    /// `[sharpness]` (moteur).
    pub sharpness: tri::sharpness::SharpnessSettings,
    /// `[similarity]` (moteur) : regroupement des images semblables.
    pub similarity: tri::similarity::SimilaritySettings,
    /// `[i18n]` (moteur) : langue des textes.
    pub i18n: tri::i18n::I18nSettings,
    /// `[update]` (moteur) : recherche d'une nouvelle version.
    pub update: tri::update::UpdateSettings,
    /// `[gui]`.
    pub gui: GuiSettings,
    /// `[keys]` : raccourcis par identifiant d'action (`"core.flag.accept" = ["Maj+P"]`),
    /// qui remplacent ceux des profils pour les actions citées.
    pub keys: BTreeMap<String, Vec<String>>,
}

impl AppSettings {
    /// Sections du moteur.
    pub fn engine(&self) -> EngineSettings {
        EngineSettings {
            preview: self.preview,
            render: self.render,
            cache: self.cache.clone(),
            scheduler: self.scheduler,
            xmp: self.xmp,
            places: self.places.clone(),
            watch: self.watch,
            develop: self.develop.clone(),
            export: self.export.clone(),
            versions: self.versions.clone(),
            exposure: self.exposure,
            offload: self.offload.clone(),
            data: self.data.clone(),
            files: self.files.clone(),
            sharpness: self.sharpness,
            similarity: self.similarity,
            i18n: self.i18n,
            update: self.update.clone(),
        }
    }

    /// Table des raccourcis : profils de `[gui] keys`, puis section `[keys]` ; avec les
    /// problèmes à signaler (touche inconnue, conflit...).
    pub fn keymap(&self) -> (Keymap, Vec<String>) {
        let (keymap, mut warnings) = match self.gui.keymap().with_leader(&self.gui.leader) {
            Ok(keymap) => (keymap, Vec::new()),
            Err(e) => {
                let default = GuiSettings::default().leader;
                let warning = t!("keys-error", id = "gui.leader", error = e);
                let keymap = self.gui.keymap().with_leader(&default).unwrap_or_default();
                (keymap, vec![warning])
            }
        };
        let (keymap, more) = keymap.with_overrides(&self.keys);
        warnings.extend(more);
        // Touche de loupe aussi liée à une action : les deux se gêneraient.
        if let LoupeKey::Key(key) = self.gui.zoom.loupe_key
            && keymap.binds(key)
        {
            let name = crate::actions::key_label(key);
            warnings.push(t!("loupe-key-taken", key = name));
        }
        (keymap, warnings)
    }
}

/// Section `[gui]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GuiSettings {
    /// Thème de l'interface.
    pub theme: Theme,
    /// Jeu de raccourcis : classique (flèches), vim (hjkl), ou les deux.
    pub profile: Profile,
    /// Leader du changement de panneau, comme le préfixe de tmux : puis `h` / `l` (ou les
    /// flèches) pour le panneau de gauche / de droite, `o` pour le suivant.
    pub leader: String,
    /// Délai maximal entre deux touches d'une séquence (`g g`), en ms, comme le
    /// `timeoutlen` de vim.
    pub key_sequence_timeout_ms: u64,
    /// Touche qui est un raccourci et le début d'un autre (`y` et `y w`) : délai d'attente
    /// de la suite (ms) avant d'agir seule.
    pub key_ambiguous_timeout_ms: u64,
    /// Taille initiale de la fenêtre (points).
    pub window: [f32; 2],
    /// Rendus RAW : à la demande (image affichée et voisines, en mode RAW) ou tout le
    /// dossier en fond.
    pub raw_previews: RawPreviews,
    /// Temps maximal passé à charger des vignettes, et à intégrer les résultats du moteur,
    /// par image affichée (ms) : au-delà, on reprend à l'image suivante pour ne pas figer
    /// l'interface.
    pub load_budget_ms: u64,
    /// Largeur des barres de progression de la barre d'état (points).
    pub progress_width: f32,
    /// Masquer les barres de progression terminées (la barre d'état reste courte).
    pub progress_hide_done: bool,
    /// Délai avant d'afficher les barres de progression après un refiltrage (ms) : le
    /// cache répond souvent tout de suite, une barre montrée 1/10 s n'est pas lisible
    /// (retour du 21/10/2026).
    pub progress_delay_ms: u64,
    /// Côté des boutons à icône du bandeau de notation (avance automatique, image
    /// précédente / suivante), en points.
    pub bar_icon_size: f32,
    /// Barre de filtres affichée au démarrage (Maj+F l'affiche ou la masque).
    pub filter_bar: bool,
    /// Sans dossier en argument, rouvrir le dernier dossier ouvert.
    pub reopen_last: bool,
    /// Ordre des images au premier lancement (`f s` le change, retenu ensuite) : par nom,
    /// date de prise de vue ou de modification.
    pub sort: tri::sort::SortKey,
    /// Ordre décroissant (`f r`).
    pub sort_descending: bool,
    /// Garder d'un lancement à l'autre ce qu'on affiche ou masque d'une touche (collimateur,
    /// carte de netteté, zones, histogramme, panneaux, barres, avance automatique) ; les
    /// valeurs de la configuration ne servent qu'au premier lancement.
    pub remember_display: bool,
    /// Bandeau de notation à la souris affiché au démarrage (Maj+N l'affiche ou le masque).
    pub rating_bar: bool,
    /// Barre du haut : icône et nom de l'application à gauche (comme darktable).
    pub title_bar: bool,
    /// Barre du haut : taille du nom (points ; l'icône un peu plus grande).
    pub title_bar_size: f32,
    /// Mode concentration : durée de l'indication « F11 pour en sortir » (ms).
    pub focus_mode_hint_ms: u64,
    /// Après une note au clavier ou au bandeau, passer à l'image suivante (Maj+A l'active
    /// ou le désactive).
    pub auto_advance: bool,
    /// Annuler : nombre de pas gardés dans l'historique de la session (les plus anciens
    /// sont oubliés).
    pub history: usize,
    /// `[gui.grid]`.
    pub grid: GridSettings,
    /// `[gui.view]`.
    pub view: ViewSettings,
    /// `[gui.zoom]`.
    pub zoom: ZoomSettings,
    /// `[gui.compare]`.
    pub compare: CompareSettings,
    /// `[gui.dialogs]`.
    pub dialogs: DialogSettings,
    /// `[gui.colors]`.
    pub colors: Colors,
    /// `[gui.folders]`.
    pub folders: FolderSettings,
    /// `[gui.histogram]`.
    pub histogram: HistogramSettings,
    /// `[gui.metadata]`.
    pub metadata: MetadataSettings,
    /// `[gui.sharpness]`.
    pub sharpness: SharpnessDisplay,
    /// `[gui.crop]`.
    pub crop: CropSettings,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            // Fond sombre et neutre : la norme pour juger des photos.
            theme: Theme::Dark,
            profile: Profile::Classic,
            leader: "Ctrl+b".into(),
            key_sequence_timeout_ms: 1000,
            key_ambiguous_timeout_ms: 300,
            window: [1400.0, 900.0],
            // En mode JPEG boîtier, la machine reste libre.
            raw_previews: RawPreviews::OnDemand,
            load_budget_ms: 8,
            progress_width: 160.0,
            progress_hide_done: true,
            progress_delay_ms: 400,
            bar_icon_size: 24.0,
            filter_bar: true,
            reopen_last: true,
            sort: tri::sort::SortKey::Name,
            sort_descending: false,
            remember_display: true,
            rating_bar: true,
            title_bar: true,
            title_bar_size: 26.0,
            focus_mode_hint_ms: 3000,
            auto_advance: false,
            history: 200,
            grid: GridSettings::default(),
            view: ViewSettings::default(),
            zoom: ZoomSettings::default(),
            compare: CompareSettings::default(),
            dialogs: DialogSettings::default(),
            colors: Colors::default(),
            folders: FolderSettings::default(),
            histogram: HistogramSettings::default(),
            metadata: MetadataSettings::default(),
            sharpness: SharpnessDisplay::default(),
            crop: CropSettings::default(),
        }
    }
}

impl GuiSettings {
    /// Table des raccourcis des profils choisis.
    pub fn keymap(&self) -> Keymap {
        Keymap::from_profiles(&[self.profile])
    }

    /// Budget de chargement des vignettes par image affichée.
    pub fn load_budget(&self) -> Duration {
        Duration::from_millis(self.load_budget_ms)
    }

    /// Délai maximal entre deux touches d'une séquence.
    pub fn key_sequence_timeout(&self) -> Duration {
        Duration::from_millis(self.key_sequence_timeout_ms)
    }

    /// Séquenceur des raccourcis, avec ses deux délais.
    pub fn sequencer(&self) -> crate::actions::Sequencer {
        crate::actions::Sequencer::new(self.key_sequence_timeout())
            .with_ambiguous(Duration::from_millis(self.key_ambiguous_timeout_ms))
    }
}

/// Thème de l'interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Sombre.
    #[default]
    Dark,
    /// Clair.
    Light,
    /// Celui du système.
    System,
    /// Gris neutre (idée de l'utilisateur, 19/10/2026, façon Lightroom) : panneaux gris
    /// foncé, fond central gris neutre, qui ne fausse pas le jugement des images comme un
    /// fond noir (plus claires et plus contrastées qu'elles ne sont). Couleurs :
    /// `[gui.colors] neutral_*`.
    Neutral,
}

impl Theme {
    /// Applique le thème à `ctx` : le gris neutre retouche les fonds du thème sombre
    /// d'egui ; les autres thèmes les remettent d'origine (rien d'autre n'est touché).
    pub fn apply(self, ctx: &egui::Context, colors: &Colors) {
        ctx.set_theme(self);
        let dark = egui::Visuals::dark();
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            let v = &mut style.visuals;
            if self == Theme::Neutral {
                v.panel_fill = colors.neutral_panel.into();
                v.window_fill = colors.neutral_panel.into();
                v.extreme_bg_color = colors.neutral_field.into();
            } else {
                v.panel_fill = dark.panel_fill;
                v.window_fill = dark.window_fill;
                v.extreme_bg_color = dark.extreme_bg_color;
            }
        });
    }

    /// Fond central (grille, plein écran) : gris neutre pour ce thème, sinon celui du
    /// thème.
    pub fn center(self, colors: &Colors) -> Option<Color32> {
        (self == Theme::Neutral).then(|| colors.neutral_center.into())
    }

    /// Fond d'une cellule de la grille : `neutral_cell` pour ce thème, sinon le fond des
    /// champs du thème (comme avant).
    pub fn cell(self, colors: &Colors, visuals: &egui::Visuals) -> Color32 {
        if self == Theme::Neutral {
            colors.neutral_cell.into()
        } else {
            visuals.extreme_bg_color
        }
    }

    /// Couleur de sélection (fond de la cellule, cadre de l'image active en comparaison) :
    /// `selection` si elle est donnée, sinon `neutral_selection` pour ce thème, sinon
    /// celle du thème.
    pub fn selection(self, colors: &Colors, visuals: &egui::Visuals) -> Color32 {
        match (colors.selection, self) {
            (Some(c), _) => c.into(),
            (None, Theme::Neutral) => colors.neutral_selection.into(),
            (None, _) => visuals.selection.bg_fill,
        }
    }

    /// Nom d'une image sur une cellule sélectionnée : sombre sur le gris clair de ce
    /// thème (sans `selection` donnée), sinon `text`.
    pub fn selected_text(self, colors: &Colors, text: Color32) -> Color32 {
        if self == Theme::Neutral && colors.selection.is_none() {
            colors.neutral_cell_text.into()
        } else {
            text
        }
    }

    /// Nom d'une image sur une cellule non sélectionnée : `neutral_cell_text` pour ce
    /// thème, sinon `text` (la couleur de texte du thème).
    pub fn cell_text(self, colors: &Colors, text: Color32) -> Color32 {
        if self == Theme::Neutral {
            colors.neutral_cell_text.into()
        } else {
            text
        }
    }

    /// Fond du plein écran, de la comparaison et de la loupe : le gris central pour ce
    /// thème, sinon `view_background`.
    pub fn view_background(self, colors: &Colors) -> Color32 {
        self.center(colors).unwrap_or(colors.view_background.into())
    }

    /// Fonds des piles (un groupe sur deux, l'autre) : accordés au thème.
    pub fn stack_bands(self, colors: &Colors) -> (Color32, Color32) {
        if self == Theme::Neutral {
            (
                colors.neutral_stack_band.into(),
                colors.neutral_stack_band_alt.into(),
            )
        } else {
            (colors.stack_band.into(), colors.stack_band_alt.into())
        }
    }
}

impl From<Theme> for egui::ThemePreference {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
            Theme::System => egui::ThemePreference::System,
            Theme::Neutral => egui::ThemePreference::Dark,
        }
    }
}

/// Section `[gui.grid]` : dimensions en points.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridSettings {
    /// Côté d'une cellule au premier lancement et au double-clic sur le curseur de taille
    /// (la taille choisie est ensuite retenue, `state.rs`).
    pub cell: f32,
    /// Bornes du curseur de taille des vignettes.
    pub cell_min: f32,
    pub cell_max: f32,
    /// Espace entre les cellules.
    pub gap: f32,
    /// Infos sur les vignettes (nom, format, notes, badges) au démarrage (Maj+T).
    pub info: bool,
    /// Hauteur réservée au nom du fichier sous la vignette.
    pub label: f32,
    /// Marge intérieure de la cellule.
    pub padding: f32,
    /// Longueur maximale du nom affiché (caractères, « … » au-delà).
    pub name_chars: usize,
    /// Vignettes gardées en mémoire graphique au plus.
    pub textures: usize,
    /// Taille des notes (étoiles, couleurs, drapeau) sur les vignettes.
    pub marks: f32,
    /// Opacité de la vignette d'une photo rejetée (0 : invisible, 1 : normale).
    pub rejected_opacity: f32,
    /// Épaisseur du cadre de la vignette survolée par la souris.
    pub hover_frame: f32,
    /// Épaisseur du cadre des vignettes sélectionnées (sélection multiple).
    pub marked_frame: f32,
    /// Goutte sous les vignettes des photos jugées floues (score de netteté).
    pub blur_badge: bool,
    /// Largeur du curseur de similarité de la barre des filtres (points).
    pub stack_slider: f32,
    /// Largeur du curseur de sévérité du flou de la barre des filtres (points).
    pub blur_slider: f32,
    /// Largeur de chacun des curseurs « brûlé » / « bouché » de la barre des filtres.
    pub clip_slider: f32,
    /// Largeur du curseur de taille des vignettes, en bas de la grille (points).
    pub size_slider: f32,
    /// Piles affichées au démarrage (sinon chaque image seule ; Ctrl+S bascule). Les
    /// empreintes se calculent en fond quand même (`[similarity] background`) : la
    /// bascule est instantanée.
    pub stacks: bool,
    /// Champ de recherche par nom de la barre des filtres (`/`, Ctrl+F) : largeur (points).
    pub search_width: f32,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            cell: 200.0,
            cell_min: 60.0,
            cell_max: 600.0,
            gap: 8.0,
            info: true,
            label: 18.0,
            padding: 6.0,
            name_chars: 24,
            textures: 600,
            marks: 12.0,
            rejected_opacity: 0.35,
            hover_frame: 2.0,
            marked_frame: 3.0,
            blur_badge: true,
            stack_slider: 90.0,
            blur_slider: 90.0,
            clip_slider: 60.0,
            size_slider: 90.0,
            stacks: false,
            search_width: 200.0,
        }
    }
}

/// Section `[gui.dialogs]` : boîtes ouvertes près du clic (notation, éjection,
/// déchargement, renommage, corbeille). Elles se déplacent en glissant leur fond.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DialogSettings {
    /// Marge intérieure (points).
    pub margin: f32,
    /// Espace entre les éléments (points).
    pub spacing: f32,
    /// Marge intérieure des boutons, horizontale (points ; verticale : moitié).
    pub button_padding: f32,
    /// Largeur minimale (points).
    pub min_width: f32,
    /// Largeur maximale (points) : les textes longs passent à la ligne.
    pub max_width: f32,
    /// Taille du texte des titres (points).
    pub title_size: f32,
    /// Couleur des titres (accent).
    pub title_color: Color,
    /// Fond du panneau qui a le clavier : part de la couleur du texte mêlée au fond des
    /// panneaux (0 : aucune différence, 1 : couleur du texte).
    pub focus_panel_tint: f32,
    /// Côté des boutons à icône (actions sur le fichier de la boîte du clic droit).
    pub icon_size: f32,
    /// Largeur minimale de l'aide (F1), points : ses lignes ne débordent pas.
    pub help_width: f32,
    /// Défilement de l'aide par flèche haut / bas (points).
    pub help_scroll_line: f32,
}

impl Default for DialogSettings {
    fn default() -> Self {
        Self {
            margin: 18.0,
            spacing: 10.0,
            button_padding: 10.0,
            min_width: 340.0,
            max_width: 520.0,
            title_size: 17.0,
            title_color: Color([80, 190, 220]),
            focus_panel_tint: 0.06,
            icon_size: 34.0,
            help_width: 620.0,
            help_scroll_line: 40.0,
        }
    }
}

/// Section `[gui.crop]` : recadrage (versions, `r` en plein écran).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CropSettings {
    /// Pas du clavier (déplacer, agrandir), en proportion de l'image.
    pub step: f64,
    /// Couleur du voile autour du cadre.
    pub shade: Color,
    /// Opacité du voile (0 à 1).
    pub shade_opacity: f32,
    /// Épaisseur du cadre (points).
    pub line_width: f32,
    /// Côté des poignées (points).
    pub handle_size: f32,
    /// Distance de prise d'un bord ou d'un angle (points).
    pub handle_grab: f32,
}

impl Default for CropSettings {
    fn default() -> Self {
        Self {
            step: 0.01,
            shade: Color([40, 40, 40]),
            shade_opacity: 0.65,
            line_width: 1.5,
            handle_size: 9.0,
            handle_grab: 10.0,
        }
    }
}

/// Section `[gui.compare]` : comparaison côte à côte (`c`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompareSettings {
    /// Images comparées au plus : au-delà, les premières de la sélection. Chacune garde
    /// un aperçu, et en 1:1 son 1:1 (~100 Mo de mémoire graphique pour 26 Mpx).
    pub max: usize,
    /// Espace entre les images (points).
    pub gap: f32,
    /// Épaisseur du cadre de l'image active (celle que l'on note).
    pub frame: f32,
}

impl Default for CompareSettings {
    fn default() -> Self {
        Self {
            max: 6,
            gap: 4.0,
            frame: 3.0,
        }
    }
}

/// Touche à maintenir pour la loupe : un modificateur, ou une touche ordinaire (retour du
/// 19/10/2026 : avec Maj, chaque raccourci Maj+… faisait apparaître la loupe ; `t` par
/// défaut, libre dans les deux jeux de raccourcis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoupeKey {
    /// Maj.
    Shift,
    /// Ctrl.
    Ctrl,
    /// Alt (Option sur macOS).
    Alt,
    /// Pas de loupe.
    None,
    /// Une touche (nom comme dans `[keys]` : « t », « Espace », « F3 »).
    Key(egui::Key),
}

impl Default for LoupeKey {
    fn default() -> Self {
        LoupeKey::Key(egui::Key::T)
    }
}

impl<'de> Deserialize<'de> for LoupeKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = String::deserialize(d)?;
        Ok(match name.to_lowercase().as_str() {
            "shift" => LoupeKey::Shift,
            "ctrl" => LoupeKey::Ctrl,
            "alt" => LoupeKey::Alt,
            "none" => LoupeKey::None,
            _ => LoupeKey::Key(crate::actions::key_named(&name).ok_or_else(|| {
                serde::de::Error::custom(t!("loupe-key-unknown", key = name.as_str()))
            })?),
        })
    }
}

impl LoupeKey {
    /// Vrai si la touche est maintenue dans `input`.
    pub fn held(self, input: &egui::InputState) -> bool {
        match self {
            LoupeKey::Shift => input.modifiers.shift,
            LoupeKey::Ctrl => input.modifiers.ctrl,
            LoupeKey::Alt => input.modifiers.alt,
            LoupeKey::None => false,
            LoupeKey::Key(key) => input.key_down(key),
        }
    }
}

/// Section `[gui.zoom]` : zoom continu et loupe (décidé le 18/10/2026).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoomSettings {
    /// Paliers de `+` / `-` (1,0 = 100 % : un pixel de l'image par pixel de l'écran).
    pub steps: Vec<f32>,
    /// Zoom maximal (4,0 = 400 %).
    pub max: f32,
    /// Zoom minimal (0,1 = 10 %) : sous l'ajusté, l'image plus petite que l'écran.
    pub min: f32,
    /// Loupe : touche à maintenir (en vue ajustée, sous le pointeur).
    pub loupe_key: LoupeKey,
    /// Loupe : délai avant d'apparaître (ms), pour ne pas clignoter quand la touche sert à
    /// un raccourci (Maj+P).
    pub loupe_delay_ms: u64,
    /// Loupe : côté (points).
    pub loupe_size: f32,
    /// Loupe : grossissement (1,0 = 100 %).
    pub loupe_scale: f32,
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            steps: vec![0.125, 0.25, 0.5, 1.0, 2.0, 4.0],
            max: 4.0,
            min: 0.1,
            loupe_key: LoupeKey::default(),
            loupe_delay_ms: 150,
            loupe_size: 320.0,
            loupe_scale: 1.0,
        }
    }
}

/// Section `[gui.view]` : plein écran.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewSettings {
    /// Temps à passer sur une image avant de lancer son 100 % RAW (ms ; ~4,5 s de calcul,
    /// que LibRaw ne sait pas interrompre) : en enchaînant vite les flèches, les images
    /// seulement traversées ne lancent pas un calcul qui bloquerait ensuite celui de
    /// l'image regardée.
    pub full_raw_dwell_ms: u64,
    /// Grandes textures (aperçus, 1:1) gardées en mémoire : l'image affichée, ses voisines
    /// et quelques images récentes. Un 1:1 de 26 Mpx occupe ~100 Mo de mémoire graphique.
    pub textures: usize,
    /// Déplacement d'une image zoomée par appui sur une touche (Maj + flèches, Maj + hjkl),
    /// en fraction de la largeur (ou hauteur) de la vue.
    pub pan_step: f32,
    /// Taille des notes (étoiles, couleurs, drapeau) en plein écran.
    pub marks: f32,
    /// Collimateur de mise au point affiché au démarrage (`m` l'affiche ou le masque ;
    /// plein écran et comparaison).
    pub focus_point: bool,
    /// Forme du repère du collimateur.
    pub focus_marker: FocusMarker,
    /// Taille du repère du collimateur (points).
    pub focus_marker_size: f32,
    /// Épaisseur du trait du repère (points).
    pub focus_marker_width: f32,
    /// Aide-mémoire des touches en haut à droite du plein écran (F1 ouvre l'aide).
    pub key_hints: bool,
    /// Mode concentration : infos et notes dessinées sur l'image (ailleurs, la barre d'état
    /// et le bandeau les montrent) ; `Ctrl+I` les masque.
    pub image_info: bool,
    /// 1:1 au clavier ou au bouton centré sur le collimateur quand il est connu (sinon au
    /// milieu de l'image).
    pub zoom_on_focus_point: bool,
    /// Mode pile (plein écran) : au bout de la pile, ← / → repartent de l'autre bout
    /// (sinon on s'arrête).
    pub stack_wrap: bool,
    /// Mode pile (plein écran), avance automatique après une note : au bout de la pile,
    /// passer à la première photo de la pile suivante (sinon on reste ; prime sur
    /// `stack_wrap` pour l'avance automatique).
    pub stack_advance_next: bool,
    /// Superpositions (o) : épaisseur des traits (points).
    pub overlay_line_width: f32,
    /// Grilles intégrées (tiers, nombre d'or, diagonales) : opacité des lignes.
    pub overlay_opacity: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            full_raw_dwell_ms: 400,
            textures: 8,
            pan_step: 0.2,
            marks: 18.0,
            focus_point: true,
            focus_marker: FocusMarker::default(),
            focus_marker_size: 28.0,
            focus_marker_width: 2.0,
            key_hints: false,
            image_info: true,
            zoom_on_focus_point: true,
            stack_wrap: false,
            stack_advance_next: false,
            overlay_line_width: 1.0,
            overlay_opacity: 0.6,
        }
    }
}

/// Forme du repère du collimateur. Dans le fichier : `"square"`, `"cross"`, `"circle"` ou
/// `"heart"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusMarker {
    /// Carré, comme dans le viseur.
    #[default]
    Square,
    /// Croix.
    Cross,
    /// Cercle.
    Circle,
    /// Cœur (proposé par l'utilisateur, le 17/10/2026).
    Heart,
}

impl ViewSettings {
    /// Attente avant un 100 % RAW.
    pub fn full_raw_dwell(&self) -> Duration {
        Duration::from_millis(self.full_raw_dwell_ms)
    }
}

/// Section `[gui.folders]` : panneau des dossiers, à gauche de la grille.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FolderSettings {
    /// Panneau affiché au démarrage (Maj+D l'affiche ou le masque).
    pub visible: bool,
    /// Largeur initiale (points) ; le bord se tire à la souris.
    pub width: f32,
    /// Retrait par niveau de l'arborescence (points).
    pub indent: f32,
    /// Montrer les dossiers cachés (`.xxx`).
    pub hidden: bool,
    /// Recherche de dossiers (`/`) : niveaux de sous-dossiers parcourus au plus.
    pub search_depth: usize,
    /// Recherche de dossiers : nombre de résultats au plus.
    pub search_results: usize,
    /// Case « sous-dossiers » du panneau : niveaux de sous-dossiers dont les images
    /// s'affichent aussi dans la grille, au plus (en plus du dossier ouvert).
    pub subfolders_depth: usize,
}

impl Default for FolderSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: 240.0,
            indent: 14.0,
            hidden: false,
            search_depth: 4,
            search_results: 200,
            subfolders_depth: 3,
        }
    }
}

/// Section `[gui.sharpness]` : affichage de la carte de netteté (calcul : `[sharpness]`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SharpnessDisplay {
    /// Carte affichée au démarrage (`p` l'affiche ou la masque ; en 1:1 seulement).
    pub visible: bool,
    /// Couleurs proposées, dans l'ordre de la rotation (`Ctrl+P`) : selon l'image, l'une
    /// ressort mieux que l'autre (une couleur proche de celle du sujet se perd).
    pub colors: Vec<Color>,
    /// Opacité des contours (0 à 1).
    pub opacity: f32,
    /// Pas de la sévérité au clavier (Ctrl+J / Ctrl+K), sur une échelle de -1 à 1.
    pub step: f32,
}

impl Default for SharpnessDisplay {
    fn default() -> Self {
        Self {
            visible: false,
            colors: vec![
                Color([255, 40, 40]),
                Color([40, 255, 80]),
                Color([40, 180, 255]),
            ],
            opacity: 1.0,
            step: 0.1,
        }
    }
}

/// Section `[gui.metadata]` : panneau des métadonnées, à droite.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetadataSettings {
    /// Panneau affiché au démarrage (Maj+M l'affiche ou le masque).
    pub visible: bool,
    /// Largeur initiale (points) ; le bord se tire à la souris.
    pub width: f32,
    /// Histogramme de l'image sélectionnée en haut du panneau, en grille comme en plein
    /// écran (panneau masqué : en surimpression sur l'image en plein écran).
    pub histogram: bool,
    /// Hauteur de cet histogramme (points).
    pub histogram_height: f32,
}

impl Default for MetadataSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: 280.0,
            histogram: true,
            histogram_height: 120.0,
        }
    }
}

/// Échelle horizontale de l'histogramme. Dans le fichier : `"ev"` ou `"linear"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistogramScale {
    /// En IL sous la saturation (0 = blanc du capteur, -1, -2…) : la marge avant de
    /// brûler se lit directement, les ombres ne sont pas écrasées à gauche.
    #[default]
    Ev,
    /// Valeurs brutes, du noir au blanc.
    Linear,
}

impl HistogramScale {
    /// L'autre échelle.
    pub fn toggled(self) -> Self {
        match self {
            Self::Ev => Self::Linear,
            Self::Linear => Self::Ev,
        }
    }
}

/// Section `[gui.histogram]` : histogramme et alertes d'exposition.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistogramSettings {
    /// Histogramme affiché au démarrage, en haut du panneau des métadonnées (touche `i`).
    pub visible: bool,
    /// Échelle au démarrage (`Maj+I` bascule).
    pub scale: HistogramScale,
    /// Grand histogramme (`Maj+E`) : part de la largeur de la fenêtre (0,2 à 1).
    pub large: f32,
    /// IL montrés sous la saturation, en échelle IL : 0 (défaut) = plage utile de
    /// l'image (~13,9 IL pour un RAW 14 bits, ~11,7 pour un JPEG) ; positif : plafond.
    pub ev_shown: f32,
    /// Zones saturées / bouchées superposées à l'image au démarrage (touche `o`).
    pub clipping: bool,
    /// Opacité des zones superposées (0 à 1).
    pub clipping_opacity: f32,
    /// Zones saturées colorées par canal (`clip_red`, `clip_green`, `clip_blue`,
    /// mélangées si deux canaux, `clip_over` si les trois) ; sinon `clip_over` pour toute
    /// saturation.
    pub clipping_channels: bool,
    /// Badge « brûlée » sur une vignette au-delà de cette part de pixels saturés (canal
    /// le plus touché, 0 à 1).
    pub badge_over: f64,
    /// Badge « bouchée » au-delà de cette part de pixels bouchés.
    pub badge_under: f64,
    /// Curseurs « brûlé » / « bouché » de la barre des filtres : à fond à droite, seuils
    /// des badges divisés par cette valeur ; vers la gauche, multipliés (tout à gauche, ce
    /// côté n'exclut plus rien).
    pub strictness_span: f64,
    /// Pastilles saturés / bouchés de l'histogramme : allumées au-delà de cette part des
    /// pixels (canal le plus touché, 0 à 1), pas pour quelques pixels morts.
    pub indicator_min: f64,
    /// Nombre de classes de la courbe (sur la largeur de l'histogramme).
    pub classes: usize,
    /// Lissage de la courbe : écart type, en classes (0 : sans lissage).
    pub smoothing: f32,
    /// Intensité des surfaces de la courbe (0 à 1), mélangées en additif : rouge + vert =
    /// jaune, les trois = blanc.
    pub fill_strength: f32,
    /// Statistiques du grand histogramme : part de pixels laissée de chaque côté pour la
    /// plage des ombres aux hautes lumières (0,01 : centiles 1 % et 99 %).
    pub stats_tail: f32,
    /// Grand histogramme : diamètre des pastilles de choix du canal (points).
    pub channel_dot: f32,
    /// Grand histogramme : photo d'analyse (zones saturées / bouchées sur l'image en
    /// gris) affichée à l'ouverture, à droite de l'histogramme et à sa hauteur (`p`
    /// bascule).
    pub analysis: bool,
    /// Fond du bloc de l'image d'analyse : fond de la boîte éclairci de cette part vers
    /// le blanc (0 à 1).
    pub analysis_lighten: f32,
}

impl Default for HistogramSettings {
    fn default() -> Self {
        Self {
            visible: true,
            scale: HistogramScale::Ev,
            large: 0.7,
            ev_shown: 0.0,
            clipping: false,
            clipping_opacity: 0.7,
            clipping_channels: true,
            badge_over: 0.005,
            badge_under: 0.05,
            strictness_span: 10.0,
            indicator_min: 0.001,
            classes: 80,
            smoothing: 1.5,
            fill_strength: 0.55,
            stats_tail: 0.01,
            channel_dot: 11.0,
            analysis: false,
            analysis_lighten: 0.06,
        }
    }
}

/// Section `[gui.colors]` : couleurs `"#rrggbb"`. Sélection absente : couleur du thème.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// Fond de la cellule sélectionnée (au clic ou au clavier).
    pub selection: Option<Color>,
    /// Barres de progression de la barre d'état ; absent : gris du thème (discret, pas la
    /// couleur d'accent).
    pub progress: Option<Color>,
    /// Cadre de la cellule survolée par la souris (le survol ne sélectionne pas).
    pub hover: Color,
    /// Cadre des images sélectionnées (sélection multiple : Espace, v, Maj+clic…).
    pub marked: Color,
    /// Repère du collimateur de mise au point.
    pub focus: Color,
    /// Grilles intégrées des superpositions (tiers, nombre d'or, diagonales).
    pub overlay: Color,
    /// Goutte des filtres de netteté (barre de filtres).
    pub blur_drop: Color,
    /// Piles actives : fond des images d'un groupe sur deux (gris clair), photo isolée
    /// comprise (un groupe d'une image) ; les autres sur `stack_band_alt` (gris foncé).
    pub stack_band: Color,
    /// Fond de l'autre pile sur deux (voir `stack_band`).
    pub stack_band_alt: Color,
    /// Thème gris neutre : panneaux et boîtes.
    pub neutral_panel: Color,
    /// Thème gris neutre : fond central (grille, plein écran).
    pub neutral_center: Color,
    /// Thème gris neutre : cellules de la grille (un peu plus claires que le fond).
    pub neutral_cell: Color,
    /// Thème gris neutre : champs de saisie (recherche, réglages).
    pub neutral_field: Color,
    /// Thème gris neutre : cellule sélectionnée (gris clair, comme darktable), sauf si
    /// `selection` est donnée.
    pub neutral_selection: Color,
    /// Thème gris neutre : nom des images sur les cellules (sombre : lisible sur un gris
    /// moyen, contrairement au gris du thème sombre).
    pub neutral_cell_text: Color,
    /// Thème gris neutre : fonds des piles, un groupe sur deux et l'autre.
    pub neutral_stack_band: Color,
    /// Voir `neutral_stack_band`.
    pub neutral_stack_band_alt: Color,
    /// Piles repliées : feuilles derrière la couverture et pastille du nombre d'images.
    pub stack_mark: Color,
    /// Badge « RAW » des cellules ; absent : couleur d'accent du thème.
    pub badge: Option<Color>,
    /// Icône d'une vignette impossible à calculer.
    pub warning: Color,
    /// Messages d'erreur.
    pub error: Color,
    /// Fond du plein écran.
    pub view_background: Color,
    /// Fonds proposés tour à tour (`Ctrl+L`) pour la grille et le plein écran, après celui
    /// du thème ; une couleur égale à celle du thème ou déjà vue est sautée.
    pub backgrounds: Vec<Color>,
    /// Étoiles ; absent : couleur d'accent du thème, comme le reste de l'interface
    /// (retour du 21/10/2026, à la place du jaune).
    pub stars: Option<Color>,
    /// Pastille « acceptée ».
    pub accepted: Color,
    /// Pastille « rejetée ».
    pub rejected: Color,
    /// Couleurs de notation, telles qu'affichées.
    pub red: Color,
    /// Jaune.
    pub yellow: Color,
    /// Vert.
    pub green: Color,
    /// Bleu.
    pub blue: Color,
    /// Violet.
    pub purple: Color,
    /// Histogramme : canal rouge.
    pub hist_red: Color,
    /// Histogramme : canal vert.
    pub hist_green: Color,
    /// Histogramme : canal bleu.
    pub hist_blue: Color,
    /// Zones et badges : saturé dans les trois canaux (brûlé), ou dans un canal
    /// quelconque sans couleur par canal.
    pub clip_over: Color,
    /// Zones : saturé dans le seul canal rouge.
    pub clip_red: Color,
    /// Zones : saturé dans le seul canal vert.
    pub clip_green: Color,
    /// Zones : saturé dans le seul canal bleu.
    pub clip_blue: Color,
    /// Zones et badges : bouché.
    pub clip_under: Color,
    /// Image d'analyse du grand histogramme (en gris) : saturé.
    pub analysis_over: Color,
    /// Image d'analyse du grand histogramme : bouché.
    pub analysis_under: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            selection: None,
            progress: None,
            hover: Color([180, 180, 180]),
            marked: Color([255, 170, 40]),
            focus: Color([60, 255, 110]),
            overlay: Color([255, 255, 255]),
            blur_drop: Color([110, 170, 230]),
            stack_band: Color([56, 56, 56]),
            stack_band_alt: Color([24, 24, 24]),
            // Relevés sur une capture de darktable (thème gris, 19/10/2026).
            neutral_panel: Color([42, 42, 42]),
            neutral_center: Color([94, 94, 94]),
            neutral_cell: Color([118, 118, 118]),
            neutral_field: Color([62, 62, 62]),
            neutral_cell_text: Color([30, 30, 30]),
            neutral_selection: Color([179, 179, 179]),
            neutral_stack_band: Color([132, 132, 132]),
            neutral_stack_band_alt: Color([106, 106, 106]),
            stack_mark: Color([140, 140, 140]),
            badge: None,
            warning: Color([230, 160, 60]),
            error: Color([230, 90, 90]),
            view_background: Color([12, 12, 12]),
            backgrounds: vec![
                Color([48, 48, 48]),
                Color([0, 0, 0]),
                Color([255, 255, 255]),
            ],
            stars: None,
            accepted: Color([70, 170, 90]),
            rejected: Color([215, 70, 70]),
            red: Color([225, 65, 65]),
            yellow: Color([230, 200, 50]),
            green: Color([70, 180, 80]),
            blue: Color([65, 130, 225]),
            purple: Color([165, 85, 210]),
            hist_red: Color([235, 70, 70]),
            hist_green: Color([80, 200, 90]),
            hist_blue: Color([80, 130, 240]),
            clip_over: Color([255, 40, 255]),
            clip_red: Color([255, 50, 50]),
            clip_green: Color([50, 230, 50]),
            clip_blue: Color([50, 120, 255]),
            clip_under: Color([0, 210, 255]),
            analysis_over: Color([255, 220, 0]),
            analysis_under: Color([40, 110, 255]),
        }
    }
}

/// Couleur opaque, écrite `"#rrggbb"` dans le fichier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub [u8; 3]);

impl From<Color> for Color32 {
    fn from(Color([r, g, b]): Color) -> Self {
        Color32::from_rgb(r, g, b)
    }
}

impl std::str::FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let hex = s
            .strip_prefix('#')
            .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| t!("settings-bad-color", value = s))?;
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("hexadécimal vérifié");
        Ok(Color([byte(0), byte(2), byte(4)]))
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let [r, g, b] = self.0;
        serializer.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le fichier d'exemple documente exactement les valeurs par défaut du code.
    #[test]
    fn example_file_matches_the_defaults() {
        let text = include_str!("../../doc/config.example.toml");
        let parsed: AppSettings = tri::settings::parse(text).unwrap();
        assert_eq!(parsed, AppSettings::default());
    }

    #[test]
    fn empty_file_gives_defaults() {
        let parsed: AppSettings = tri::settings::parse("").unwrap();
        assert_eq!(parsed, AppSettings::default());
    }

    #[test]
    fn gui_section_overrides_and_engine_sections_pass_through() {
        let parsed: AppSettings = tri::settings::parse(
            r##"
            [preview]
            standard = 3840
            [gui]
            theme = "light"
            profile = "vim"
            raw_previews = "background"
            [gui.view]
            full_raw_dwell_ms = 250
            [gui.colors]
            selection = "#1E6F7F"
            "##,
        )
        .unwrap();
        assert_eq!(parsed.gui.theme, Theme::Light);
        assert_eq!(parsed.gui.profile, Profile::Vim);
        assert_eq!(parsed.gui.raw_previews, RawPreviews::Background);
        assert_eq!(parsed.gui.view.full_raw_dwell(), Duration::from_millis(250));
        assert_eq!(parsed.gui.colors.selection, Some(Color([0x1e, 0x6f, 0x7f])));
        assert_eq!(parsed.gui.grid, GridSettings::default());
        assert_eq!(
            parsed.engine().preview(Some(1000)).sizes.standard,
            3840,
            "section du moteur transmise"
        );
    }

    #[test]
    fn unknown_sections_and_keys_are_reported() {
        for (text, culprit) in [
            ("[previw]\nthumbnail = 300", "previw"),
            ("[gui]\nthme = \"dark\"", "thme"),
            ("[gui.grid]\ncells = 3", "cells"),
        ] {
            let err = tri::settings::parse::<AppSettings>(text).unwrap_err();
            assert!(err.contains(culprit), "{text} : {err}");
        }
    }

    #[test]
    fn bad_values_are_reported() {
        for (text, culprit) in [
            ("[gui]\ntheme = \"sepia\"", "sepia"),
            ("[gui]\nprofile = \"emacs\"", "emacs"),
            ("[gui.colors]\nbadge = \"bleu\"", "#rrggbb"),
            ("[gui.colors]\nbadge = \"#12345\"", "#rrggbb"),
            ("[gui.colors]\nbadge = \"#12345g\"", "#rrggbb"),
        ] {
            let err = tri::settings::parse::<AppSettings>(text).unwrap_err();
            assert!(err.contains(culprit), "{text} : {err}");
        }
    }

    #[test]
    fn keys_section_overrides_profile_keys() {
        let parsed: AppSettings = tri::settings::parse(
            r#"
            [keys]
            "core.flag.accept" = ["a"]
            "core.nope" = ["z"]
            "#,
        )
        .unwrap();
        let (keymap, warnings) = parsed.keymap();
        assert_eq!(
            keymap.action(egui::Key::A, egui::Modifiers::NONE),
            Some(crate::actions::Action::Flag(tri::xmp::Flag::Accepted))
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            AppSettings::default().keymap().1.is_empty(),
            "défauts sans conflit"
        );
    }

    #[test]
    fn the_neutral_theme_has_its_own_greys() {
        let c = Colors::default();
        assert_eq!(Theme::Dark.center(&c), None);
        assert_eq!(Theme::Neutral.center(&c), Some(c.neutral_center.into()));
        assert_eq!(
            Theme::Neutral.view_background(&c),
            Color32::from(c.neutral_center)
        );
        assert_eq!(
            Theme::Dark.view_background(&c),
            Color32::from(c.view_background)
        );
        assert_eq!(
            Theme::Neutral.stack_bands(&c),
            (c.neutral_stack_band.into(), c.neutral_stack_band_alt.into())
        );
        // Cellules et champs séparés ; noms sombres sur les cellules.
        let v = egui::Visuals::dark();
        assert_eq!(Theme::Neutral.cell(&c, &v), Color32::from(c.neutral_cell));
        assert_eq!(Theme::Dark.cell(&c, &v), v.extreme_bg_color);
        assert_eq!(
            Theme::Neutral.cell_text(&c, Color32::WHITE),
            Color32::from(c.neutral_cell_text)
        );
        assert_eq!(Theme::Dark.cell_text(&c, Color32::WHITE), Color32::WHITE);
        // Sélection gris clair (texte sombre), sauf couleur donnée.
        assert_eq!(
            Theme::Neutral.selection(&c, &v),
            Color32::from(c.neutral_selection)
        );
        assert_eq!(Theme::Dark.selection(&c, &v), v.selection.bg_fill);
        let mut chosen = c;
        chosen.selection = Some(Color([1, 2, 3]));
        assert_eq!(
            Theme::Neutral.selection(&chosen, &v),
            Color32::from_rgb(1, 2, 3)
        );
        assert_eq!(
            Theme::Neutral.selected_text(&chosen, Color32::WHITE),
            Color32::WHITE
        );
        let parsed: AppSettings = tri::settings::parse("[gui]\ntheme = \"neutral\"\n").unwrap();
        assert_eq!(parsed.gui.theme, Theme::Neutral);
        assert_eq!(
            egui::ThemePreference::from(Theme::Neutral),
            egui::ThemePreference::Dark
        );
    }

    #[test]
    fn the_loupe_key_is_a_modifier_or_any_key() {
        let parse = |v: &str| {
            tri::settings::parse::<AppSettings>(&format!("[gui.zoom]\nloupe_key = \"{v}\"\n"))
                .map(|s| s.gui.zoom.loupe_key)
        };
        assert_eq!(parse("t"), Ok(LoupeKey::Key(egui::Key::T)));
        assert_eq!(parse("F3"), Ok(LoupeKey::Key(egui::Key::F3)));
        assert_eq!(parse("shift"), Ok(LoupeKey::Shift));
        assert_eq!(parse("none"), Ok(LoupeKey::None));
        assert!(
            parse("touche-magique")
                .unwrap_err()
                .contains("touche-magique")
        );
        assert_eq!(
            ZoomSettings::default().loupe_key,
            LoupeKey::Key(egui::Key::T)
        );
        // Libre par défaut ; une touche déjà prise est signalée.
        assert!(AppSettings::default().keymap().1.is_empty());
        let mut taken = AppSettings::default();
        taken.gui.zoom.loupe_key = LoupeKey::Key(egui::Key::O);
        assert_eq!(taken.keymap().1.len(), 1, "o : superpositions");
    }

    #[test]
    fn each_profile_has_only_its_own_keys() {
        let keymap = |profile| {
            GuiSettings {
                profile,
                ..GuiSettings::default()
            }
            .keymap()
        };
        let next = |km: &Keymap, k| km.action(k, egui::Modifiers::NONE);
        let (classic, vim, both) = (
            keymap(Profile::Classic),
            keymap(Profile::Vim),
            keymap(Profile::Both),
        );
        use crate::actions::Action;
        use egui::Key::{ArrowRight, L, X, Y};
        assert_eq!(next(&classic, ArrowRight), Some(Action::NavNext));
        assert_eq!(next(&classic, L), None, "pas de hjkl en classique");
        assert_eq!(next(&classic, X), None);
        assert_eq!(next(&vim, L), Some(Action::NavNext));
        assert_eq!(next(&vim, ArrowRight), None, "pas de flèches en vim");
        assert_eq!(next(&vim, Y), Some(Action::FileCopy));
        assert_eq!(next(&both, L), next(&both, ArrowRight));
        // Communs : la notation, dans les deux.
        for km in [&classic, &vim] {
            assert_eq!(next(km, egui::Key::Num3), Some(Action::Stars(3)));
        }
        assert_eq!(GuiSettings::default().profile, Profile::Classic);
    }
}
