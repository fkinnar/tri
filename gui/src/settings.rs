//! Réglages de l'interface : le fichier de configuration entier, sections du moteur
//! comprises, plus la section `[gui]`.
//!
//! Même fichier que le moteur (voir `tri::settings`) ; toutes les valeurs par défaut sont
//! dans `doc/config.example.toml`, qu'un test compare au code.

use std::collections::BTreeMap;
use std::time::Duration;

use eframe::egui::{self, Color32};
use serde::Deserialize;
use serde::de::{self, Deserializer};
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
    /// `[exposure]` (moteur).
    pub exposure: tri::exposure::ExposureSettings,
    /// `[offload]` (moteur).
    pub offload: tri::offload::OffloadSettings,
    /// `[data]` (moteur).
    pub data: tri::settings::DataSection,
    /// `[files]` (moteur).
    pub files: tri::files::FileSettings,
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
            exposure: self.exposure,
            offload: self.offload.clone(),
            data: self.data.clone(),
            files: self.files.clone(),
        }
    }

    /// Table des raccourcis : profils de `[gui] keys`, puis section `[keys]` ; avec les
    /// problèmes à signaler (touche inconnue, conflit...).
    pub fn keymap(&self) -> (Keymap, Vec<String>) {
        self.gui.keymap().with_overrides(&self.keys)
    }
}

/// Section `[gui]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GuiSettings {
    /// Thème de l'interface.
    pub theme: Theme,
    /// Profils de raccourcis actifs ; le premier l'emporte en cas de conflit. Liste vide :
    /// profils par défaut.
    pub keys: Vec<Profile>,
    /// Délai maximal entre deux touches d'une séquence (`g g`), en ms, comme le
    /// `timeoutlen` de vim.
    pub key_sequence_timeout_ms: u64,
    /// Taille initiale de la fenêtre (points).
    pub window: [f32; 2],
    /// Rendus RAW : à la demande (image affichée et voisines, en mode RAW) ou tout le
    /// dossier en fond.
    pub raw_previews: RawPreviews,
    /// Temps maximal passé à charger des vignettes par image affichée (ms) : au-delà, on
    /// reprend à l'image suivante pour ne pas figer l'interface.
    pub load_budget_ms: u64,
    /// Barre de filtres affichée au démarrage (Maj+F l'affiche ou la masque).
    pub filter_bar: bool,
    /// Sans dossier en argument, rouvrir le dernier dossier ouvert.
    pub reopen_last: bool,
    /// Bandeau de notation à la souris affiché au démarrage (Maj+N l'affiche ou le masque).
    pub rating_bar: bool,
    /// Après une note au clavier ou au bandeau, passer à l'image suivante (Maj+A l'active
    /// ou le désactive).
    pub auto_advance: bool,
    /// `[gui.grid]`.
    pub grid: GridSettings,
    /// `[gui.view]`.
    pub view: ViewSettings,
    /// `[gui.colors]`.
    pub colors: Colors,
    /// `[gui.folders]`.
    pub folders: FolderSettings,
    /// `[gui.histogram]`.
    pub histogram: HistogramSettings,
    /// `[gui.metadata]`.
    pub metadata: MetadataSettings,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            // Fond sombre et neutre : la norme pour juger des photos.
            theme: Theme::Dark,
            keys: vec![Profile::Classic, Profile::Vim],
            key_sequence_timeout_ms: 1000,
            window: [1400.0, 900.0],
            // En mode JPEG boîtier, la machine reste libre.
            raw_previews: RawPreviews::OnDemand,
            load_budget_ms: 8,
            filter_bar: true,
            reopen_last: true,
            rating_bar: true,
            auto_advance: false,
            grid: GridSettings::default(),
            view: ViewSettings::default(),
            colors: Colors::default(),
            folders: FolderSettings::default(),
            histogram: HistogramSettings::default(),
            metadata: MetadataSettings::default(),
        }
    }
}

impl GuiSettings {
    /// Table des raccourcis des profils choisis.
    pub fn keymap(&self) -> Keymap {
        if self.keys.is_empty() {
            Keymap::default()
        } else {
            Keymap::from_profiles(&self.keys)
        }
    }

    /// Budget de chargement des vignettes par image affichée.
    pub fn load_budget(&self) -> Duration {
        Duration::from_millis(self.load_budget_ms)
    }

    /// Délai maximal entre deux touches d'une séquence.
    pub fn key_sequence_timeout(&self) -> Duration {
        Duration::from_millis(self.key_sequence_timeout_ms)
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
}

impl From<Theme> for egui::ThemePreference {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
            Theme::System => egui::ThemePreference::System,
        }
    }
}

/// Section `[gui.grid]` : dimensions en points.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridSettings {
    /// Côté d'une cellule.
    pub cell: f32,
    /// Espace entre les cellules.
    pub gap: f32,
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
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            cell: 200.0,
            gap: 8.0,
            label: 18.0,
            padding: 6.0,
            name_chars: 24,
            textures: 600,
            marks: 12.0,
            rejected_opacity: 0.35,
            hover_frame: 2.0,
            marked_frame: 3.0,
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
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            full_raw_dwell_ms: 400,
            textures: 8,
            pan_step: 0.2,
            marks: 18.0,
        }
    }
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
}

impl Default for FolderSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: 240.0,
            indent: 14.0,
            hidden: false,
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
        }
    }
}

/// Section `[gui.colors]` : couleurs `"#rrggbb"`. Sélection absente : couleur du thème.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
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
    /// Badge « RAW » des cellules.
    pub badge: Color,
    /// Icône d'une vignette impossible à calculer.
    pub warning: Color,
    /// Messages d'erreur.
    pub error: Color,
    /// Fond du plein écran.
    pub view_background: Color,
    /// Étoiles.
    pub stars: Color,
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
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            selection: None,
            progress: None,
            hover: Color([180, 180, 180]),
            marked: Color([255, 170, 40]),
            badge: Color([120, 170, 230]),
            warning: Color([230, 160, 60]),
            error: Color([230, 90, 90]),
            view_background: Color([12, 12, 12]),
            stars: Color([240, 196, 64]),
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
            .ok_or_else(|| format!("couleur « {s} » : attendu \"#rrggbb\""))?;
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("hexadécimal vérifié");
        Ok(Color([byte(0), byte(2), byte(4)]))
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
            keys = ["vim"]
            raw_previews = "background"
            [gui.view]
            full_raw_dwell_ms = 250
            [gui.colors]
            selection = "#1E6F7F"
            "##,
        )
        .unwrap();
        assert_eq!(parsed.gui.theme, Theme::Light);
        assert_eq!(parsed.gui.keys, [Profile::Vim]);
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
            ("[gui]\nkeys = [\"emacs\"]", "emacs"),
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
    fn empty_key_list_falls_back_to_default_profiles() {
        let gui = GuiSettings {
            keys: Vec::new(),
            ..GuiSettings::default()
        };
        let next = |km: &Keymap| km.action(egui::Key::L, egui::Modifiers::NONE);
        assert_eq!(next(&gui.keymap()), next(&Keymap::default()));
        assert!(next(&gui.keymap()).is_some());
    }
}
