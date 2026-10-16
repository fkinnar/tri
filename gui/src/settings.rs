//! Réglages de l'interface : le fichier de configuration entier, sections du moteur
//! comprises, plus la section `[gui]`.
//!
//! Même fichier que le moteur (voir `tri::settings`) ; toutes les valeurs par défaut sont
//! dans `doc/config.example.toml`, qu'un test compare au code.

use std::time::Duration;

use eframe::egui::{self, Color32};
use serde::Deserialize;
use serde::de::{self, Deserializer};
use tri::scheduler::RawPreviews;
use tri::settings::{
    CacheSection, EngineSettings, PreviewSection, RenderSection, SchedulerSection,
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
    /// `[gui]`.
    pub gui: GuiSettings,
}

impl AppSettings {
    /// Sections du moteur.
    pub fn engine(&self) -> EngineSettings {
        EngineSettings {
            preview: self.preview,
            render: self.render,
            cache: self.cache.clone(),
            scheduler: self.scheduler,
        }
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
    /// Taille initiale de la fenêtre (points).
    pub window: [f32; 2],
    /// Rendus RAW : à la demande (image affichée et voisines, en mode RAW) ou tout le
    /// dossier en fond.
    pub raw_previews: RawPreviews,
    /// Temps maximal passé à charger des vignettes par image affichée (ms) : au-delà, on
    /// reprend à l'image suivante pour ne pas figer l'interface.
    pub load_budget_ms: u64,
    /// `[gui.grid]`.
    pub grid: GridSettings,
    /// `[gui.view]`.
    pub view: ViewSettings,
    /// `[gui.colors]`.
    pub colors: Colors,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            // Fond sombre et neutre : la norme pour juger des photos.
            theme: Theme::Dark,
            keys: vec![Profile::Classic, Profile::Vim],
            window: [1400.0, 900.0],
            // En mode JPEG boîtier, la machine reste libre.
            raw_previews: RawPreviews::OnDemand,
            load_budget_ms: 8,
            grid: GridSettings::default(),
            view: ViewSettings::default(),
            colors: Colors::default(),
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
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            full_raw_dwell_ms: 400,
            textures: 8,
        }
    }
}

impl ViewSettings {
    /// Attente avant un 100 % RAW.
    pub fn full_raw_dwell(&self) -> Duration {
        Duration::from_millis(self.full_raw_dwell_ms)
    }
}

/// Section `[gui.colors]` : couleurs `"#rrggbb"`. Sélection et survol absents : couleurs
/// du thème.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// Fond de la cellule sélectionnée.
    pub selection: Option<Color>,
    /// Fond de la cellule survolée.
    pub hover: Option<Color>,
    /// Badge « RAW » des cellules.
    pub badge: Color,
    /// Icône d'une vignette impossible à calculer.
    pub warning: Color,
    /// Messages d'erreur.
    pub error: Color,
    /// Fond du plein écran.
    pub view_background: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            selection: None,
            hover: None,
            badge: Color([120, 170, 230]),
            warning: Color([230, 160, 60]),
            error: Color([230, 90, 90]),
            view_background: Color([12, 12, 12]),
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
