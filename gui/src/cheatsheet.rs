//! Aide « ici » (`?`, idée de l'utilisateur du 06/11/2026 : qu'un débutant trouve
//! facilement quoi faire) : par-dessus l'écran, seulement les raccourcis de l'endroit où
//! l'on est (grille, plein écran, comparaison, recadrage, grand histogramme, panneau des
//! dossiers ou des métadonnées), rangés par thème. La liste complète, avec recherche, reste
//! sur F1.

use crate::actions::{Action, Group, Keymap, Scope};

/// Endroit de l'interface dont l'aide « ici » montre les raccourcis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Grille des vignettes.
    Grid,
    /// Une image en plein écran.
    View,
    /// Comparaison côte à côte.
    Compare,
    /// Recadrage d'une version.
    Crop,
    /// Grand histogramme ouvert.
    Histogram,
    /// Panneau des dossiers (il a le clavier).
    Folders,
    /// Panneau des métadonnées (il a le clavier).
    Metadata,
}

impl Place {
    /// Nom de l'endroit, dans la langue de l'interface (titre de l'aide).
    pub fn label(self) -> String {
        t!(&format!("place-{self:?}").to_lowercase())
    }

    /// Panneau qui a le clavier à cet endroit.
    fn scope(self) -> Scope {
        match self {
            Place::Grid | Place::View | Place::Compare => Scope::Images,
            Place::Crop => Scope::Crop,
            Place::Histogram => Scope::Histogram,
            Place::Folders => Scope::Folders,
            Place::Metadata => Scope::Metadata,
        }
    }
}

/// Vrai si l'action a un sens à l'endroit `place` : celles de son panneau, et pour les
/// images, ce qui agit sur ce mode (le zoom et la carte de netteté en plein écran, le
/// rendu des vignettes en grille…).
pub fn applies(action: Action, place: Place) -> bool {
    use Action::*;
    // Partout : se repérer, changer de panneau, l'aide, quitter.
    if matches!(
        action,
        Back | Help | HelpHere | Quit | FocusLeft | FocusRight | FocusNext
    ) {
        return true;
    }
    match action.scope() {
        Some(scope) => return scope == place.scope(),
        None if place.scope() != Scope::Images => {
            // Recadrage : annuler, défaire. Grand histogramme : ses réglages (portés par
            // le leader ailleurs) ; panneaux : afficher / masquer.
            return match place {
                Place::Crop => matches!(action, Undo | Redo),
                Place::Folders => matches!(
                    action,
                    ToggleFolderPanel | ToggleMetadata | RefreshFolders | Search
                ),
                Place::Metadata => matches!(action, ToggleMetadata | ToggleFolderPanel),
                _ => false,
            };
        }
        None => {}
    }
    // Ce qui ne vaut que sur une image affichée en grand.
    let image = matches!(
        action,
        ToggleZoom
            | ZoomFit
            | PanLeft
            | PanRight
            | PanUp
            | PanDown
            | TogglePeaking
            | PeakingColor
            | SharpnessLenient
            | SharpnessStrict
            | ToggleFocusPoint
            | OverlayNext
            | OverlayOrient
            | BackgroundNext
            | ToggleImageInfo
            | ToggleClipping
            | ToggleSource
    );
    // Seulement en plein écran (une seule image).
    let single = matches!(action, Crop | VersionNext | VersionPrev);
    // Seulement dans la grille.
    let grid = matches!(
        action,
        GridRenderNext
            | ToggleGridInfo
            | SelectVisual
            | StacksAll
            | NavPageUp
            | NavPageDown
            | Search
            | ToggleFolderPanel
            | RefreshFolders
            | FilterCycle
            | SortNext
            | SortReverse
    );
    match place {
        Place::Grid => !image && !single,
        Place::View => !grid,
        Place::Compare => !grid && !single && !matches!(action, Compare | Open),
        _ => false,
    }
}

/// Une ligne de l'aide « ici » : touches, libellé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Touches (« Flèche gauche / h »).
    pub keys: String,
    /// Ce que fait l'action.
    pub label: String,
}

/// Raccourcis par thème.
pub type Sections = Vec<(Group, Vec<Row>)>;

/// Les raccourcis de l'endroit `place`, par thème (dans l'ordre de l'aide), avec les
/// touches de la table `keymap` en vigueur. Les actions sans touche n'y sont pas (elles
/// sont dans la liste complète, F1).
pub fn sections(keymap: &Keymap, place: Place) -> Sections {
    let mut sections: Sections = Vec::new();
    for group in Group::ALL {
        let rows: Vec<Row> = Action::ALL
            .iter()
            .filter(|a| a.group() == group && a.available() && applies(**a, place))
            .filter_map(|&a| {
                let keys = keymap.keys_for(a);
                (!keys.is_empty()).then(|| Row {
                    keys: keys.join(" / "),
                    label: a.label(),
                })
            })
            .collect();
        if !rows.is_empty() {
            sections.push((group, rows));
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keymap() -> Keymap {
        Keymap::default().with_leader("Ctrl+b").unwrap()
    }

    fn has(place: Place, action: Action) -> bool {
        let label = action.label();
        sections(&keymap(), place)
            .iter()
            .any(|(_, rows)| rows.iter().any(|r| r.label == label))
    }

    #[test]
    fn each_place_shows_its_own_keys() {
        // Plein écran : zoom, carte de netteté, recadrage ; pas le rendu des vignettes.
        assert!(has(Place::View, Action::ToggleZoom));
        assert!(has(Place::View, Action::TogglePeaking));
        assert!(has(Place::View, Action::Crop));
        assert!(!has(Place::View, Action::GridRenderNext));
        // Grille : l'inverse.
        assert!(has(Place::Grid, Action::GridRenderNext));
        assert!(!has(Place::Grid, Action::TogglePeaking));
        assert!(!has(Place::Grid, Action::Crop));
        // Partout sur les images : noter, filtrer.
        for place in [Place::Grid, Place::View, Place::Compare] {
            assert!(has(place, Action::Stars(3)), "{place:?}");
            assert!(has(place, Action::HelpHere), "{place:?}");
        }
        // Comparaison : pas de recadrage, pas de « comparer » de nouveau.
        assert!(!has(Place::Compare, Action::Crop));
        assert!(!has(Place::Compare, Action::Compare));
    }

    #[test]
    fn a_panel_shows_its_keys_and_not_the_images_ones() {
        assert!(has(Place::Folders, Action::FolderCreate));
        assert!(has(Place::Folders, Action::FocusRight));
        assert!(!has(Place::Folders, Action::Stars(3)));
        assert!(!has(Place::Folders, Action::MetaCopy));
        assert!(has(Place::Metadata, Action::MetaCopy));
        assert!(has(Place::Histogram, Action::HistogramRed));
        assert!(!has(Place::Histogram, Action::NavNext));
        assert!(has(Place::Crop, Action::CropSave));
        assert!(has(Place::Crop, Action::Back));
        assert!(!has(Place::Crop, Action::NavNext));
    }

    #[test]
    fn actions_without_keys_stay_in_the_full_help() {
        // Sans raccourci par défaut.
        assert!(keymap().keys_for(Action::CheckUpdate).is_empty());
        assert!(!has(Place::Grid, Action::CheckUpdate));
    }

    #[test]
    fn every_place_has_a_name() {
        for place in [
            Place::Grid,
            Place::View,
            Place::Compare,
            Place::Crop,
            Place::Histogram,
            Place::Folders,
            Place::Metadata,
        ] {
            assert!(!place.label().starts_with("place-"), "{place:?}");
        }
    }
}
