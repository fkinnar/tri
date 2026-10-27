//! État d'affichage gardé d'un lancement à l'autre (retour du 17/10/2026 : le collimateur
//! masqué réapparaissait au lancement suivant) : ce qu'on affiche ou masque d'une touche
//! (collimateur, carte de netteté, zones, histogramme, panneaux, barres, avance
//! automatique). Le fichier de configuration donne la valeur du premier lancement ;
//! ensuite, l'état est repris de `gui-state.toml`, dans le dossier de données (à côté du
//! dernier dossier ouvert). `[gui] remember_display = false` : toujours les valeurs de la
//! configuration.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Nom du fichier, dans le dossier de données.
const FILE: &str = "gui-state.toml";

/// État d'affichage ; `None` : pas encore mémorisé (valeur de la configuration).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayState {
    /// Collimateur de mise au point (`m`).
    pub focus_point: Option<bool>,
    /// Carte de netteté (`p`).
    pub peaking: Option<bool>,
    /// Zones brûlées et bouchées (`o`).
    pub clipping: Option<bool>,
    /// Histogramme (`i`).
    pub histogram: Option<bool>,
    /// Panneau des dossiers (`Maj+D`).
    pub folder_panel: Option<bool>,
    /// Panneau des métadonnées (`Maj+M`).
    pub metadata_panel: Option<bool>,
    /// Barre de filtres (`Maj+F`).
    pub filter_bar: Option<bool>,
    /// Bandeau de notation (`Maj+N`).
    pub rating_bar: Option<bool>,
    /// Infos et notes sur l'image en mode concentration (`Ctrl+I`).
    pub image_info: Option<bool>,
    /// Ordre des images (`f s`, `f r`).
    pub sort: Option<tri::sort::Order>,
    /// Fond de la grille et du plein écran (`Ctrl+L`) ; absent : celui du thème.
    pub background: Option<crate::settings::Color>,
    /// Avance automatique après une note (`Maj+A`).
    pub auto_advance: Option<bool>,
    /// En entrant dans une pile : couverture ou première image (bouton du mode pile).
    pub stack_enter: Option<crate::settings::StackEnter>,
    /// Infos sur les vignettes de la grille (`Maj+T`).
    pub grid_info: Option<bool>,
    /// Taille des vignettes de la grille (curseur, `+` / `-`, `*`).
    pub grid_cell: Option<f32>,
    /// Rendu des vignettes (barre des filtres, `f d`, #114).
    pub grid_render: Option<crate::grid_render::GridRender>,
    /// Logiciels cochés du filtre « développées » (identifiants), 27/10/2026.
    pub developed_engines: Option<Vec<String>>,
    /// Sections repliées du panneau de droite (« filters », « grid »…), 27/10/2026.
    pub panel_folded: Option<Vec<String>>,
    /// Fenêtre à la fermeture (issue n° 58), reprise au lancement suivant.
    pub window: Option<WindowState>,
}

/// Taille, position et agrandissement de la fenêtre (points d'egui).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    /// Taille de l'intérieur hors agrandissement (rendue en sortant de l'agrandissement).
    pub size: [f32; 2],
    /// Coin haut gauche du cadre ; absent quand le système ne le donne pas (Wayland).
    pub position: Option<[f32; 2]>,
    /// Agrandie (plein écran du bureau, pas le mode concentration).
    pub maximized: bool,
}

/// La fenêtre `window` du lancement précédent appliquée à `viewport` : taille (si
/// plausible), position (si connue), agrandie.
pub fn restore_window(
    viewport: eframe::egui::ViewportBuilder,
    window: &WindowState,
) -> eframe::egui::ViewportBuilder {
    let [w, h] = window.size;
    let mut viewport = viewport.with_maximized(window.maximized);
    if w >= 1.0 && h >= 1.0 {
        viewport = viewport.with_inner_size([w, h]);
    }
    if let Some(position) = window.position {
        viewport = viewport.with_position(position);
    }
    viewport
}

/// État lu dans le dossier de données `data` ; vide si absent ou illisible (on repart des
/// valeurs de la configuration, sans gêner le lancement).
pub fn load(data: &Path) -> DisplayState {
    std::fs::read_to_string(data.join(FILE))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Écrit l'état dans le dossier de données `data` (fichier temporaire puis renommage : un
/// arrêt brutal ne laisse pas un fichier à moitié écrit).
pub fn save(data: &Path, state: &DisplayState) -> io::Result<()> {
    std::fs::create_dir_all(data)?;
    let text = toml::to_string(state).map_err(io::Error::other)?;
    let tmp = data.join(format!(".{FILE}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, data.join(FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_round_trips_and_a_bad_file_gives_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), DisplayState::default(), "absent");
        let state = DisplayState {
            focus_point: Some(false),
            histogram: Some(true),
            window: Some(WindowState {
                size: [1200.0, 800.0],
                position: Some([40.0, 30.0]),
                maximized: true,
            }),
            ..DisplayState::default()
        };
        save(dir.path(), &state).unwrap();
        assert_eq!(load(dir.path()), state);
        std::fs::write(dir.path().join(FILE), "focus_point = 12").unwrap();
        assert_eq!(load(dir.path()), DisplayState::default(), "illisible");
    }

    #[test]
    fn the_window_comes_back_as_it_was() {
        // Issue n° 58 : taille, position, agrandie ; sans position (Wayland), seulement le
        // reste ; une taille absurde ignorée.
        let window = WindowState {
            size: [1200.0, 800.0],
            position: Some([40.0, 30.0]),
            maximized: true,
        };
        let v = restore_window(eframe::egui::ViewportBuilder::default(), &window);
        assert_eq!(v.inner_size, Some(eframe::egui::vec2(1200.0, 800.0)));
        assert_eq!(v.position, Some(eframe::egui::pos2(40.0, 30.0)));
        assert_eq!(v.maximized, Some(true));
        let wayland = WindowState {
            size: [0.0, 0.0],
            position: None,
            maximized: false,
        };
        let v = restore_window(eframe::egui::ViewportBuilder::default(), &wayland);
        assert_eq!((v.inner_size, v.position), (None, None));
        assert_eq!(v.maximized, Some(false));
    }
}
