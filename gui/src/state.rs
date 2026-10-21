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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// Avance automatique après une note (`Maj+A`).
    pub auto_advance: Option<bool>,
    /// Infos sur les vignettes de la grille (`Maj+T`).
    pub grid_info: Option<bool>,
    /// Taille des vignettes de la grille (curseur, `+` / `-`, `*`).
    pub grid_cell: Option<f32>,
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
            ..DisplayState::default()
        };
        save(dir.path(), &state).unwrap();
        assert_eq!(load(dir.path()), state);
        std::fs::write(dir.path().join(FILE), "focus_point = 12").unwrap();
        assert_eq!(load(dir.path()), DisplayState::default(), "illisible");
    }
}
