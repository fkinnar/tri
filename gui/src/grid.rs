//! Calculs de la grille de vignettes, sans dépendance à l'affichage : nombre de colonnes,
//! déplacement de la sélection, défilement pour garder la sélection visible.

use crate::actions::Action;

/// Géométrie d'une grille : cellules carrées de `cell` points, séparées de `gap`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Côté d'une cellule (points).
    pub cell: f32,
    /// Espace entre deux cellules (points).
    pub gap: f32,
    /// Nombre de colonnes.
    pub columns: usize,
}

impl Layout {
    /// Autant de colonnes que la largeur `width` en contient (au moins une).
    pub fn fit(width: f32, cell: f32, gap: f32) -> Layout {
        let columns = (((width + gap) / (cell + gap)).floor() as usize).max(1);
        Layout { cell, gap, columns }
    }

    /// Largeur occupée par les colonnes (sans marge extérieure).
    pub fn width(&self) -> f32 {
        self.columns as f32 * self.cell + (self.columns.saturating_sub(1)) as f32 * self.gap
    }

    /// Nombre de lignes pour `n` images.
    pub fn rows(&self, n: usize) -> usize {
        n.div_ceil(self.columns)
    }

    /// Distance entre le haut de deux lignes consécutives.
    pub fn row_pitch(&self) -> f32 {
        self.cell + self.gap
    }

    /// Lignes entièrement visibles dans une hauteur `height` (au moins une).
    pub fn rows_in(&self, height: f32) -> usize {
        ((height / self.row_pitch()).floor() as usize).max(1)
    }
}

/// Plus grand côté de cellule, entre `min` et `max`, qui montre les `n` images à la fois
/// dans `width` × `height` (bouton « tout à l'écran », `*`) ; `min` si même la plus petite
/// taille déborde (au point près : la grille se mesure en points entiers).
pub fn fit_all(n: usize, width: f32, height: f32, gap: f32, min: f32, max: f32) -> f32 {
    let max = max.max(min);
    let mut cell = max.floor();
    while cell > min {
        let layout = Layout::fit(width, cell, gap);
        let rows = layout.rows(n.max(1)) as f32;
        if rows * cell + (rows - 1.0) * gap <= height {
            return cell;
        }
        cell -= 1.0;
    }
    min
}

/// Taille de cellule suivante au clavier (`+` / `-` en grille) : un quart de plus ou de
/// moins, dans les bornes.
pub fn step_cell(cell: f32, larger: bool, min: f32, max: f32) -> f32 {
    let next = if larger { cell * 1.25 } else { cell / 1.25 };
    next.clamp(min, max.max(min))
}

/// Nouvelle sélection après `action`, dans une grille de `n` images. `page_rows` : lignes
/// sautées par page. Les actions non liées à la navigation laissent la sélection en place.
pub fn navigate(
    selected: usize,
    n: usize,
    layout: &Layout,
    page_rows: usize,
    action: Action,
) -> usize {
    if n == 0 {
        return 0;
    }
    let last = n - 1;
    let cols = layout.columns;
    let selected = selected.min(last);
    match action {
        Action::NavPrev => selected.saturating_sub(1),
        Action::NavNext => (selected + 1).min(last),
        // Haut / bas : même colonne ; sur la dernière ligne incomplète, la dernière image.
        Action::NavUp => selected.checked_sub(cols).unwrap_or(selected),
        Action::NavDown if selected + cols <= last => selected + cols,
        Action::NavDown if selected / cols < last / cols => last,
        Action::NavDown => selected,
        Action::NavFirst => 0,
        Action::NavLast => last,
        Action::NavPageUp => selected.saturating_sub(cols * page_rows.max(1)),
        Action::NavPageDown => (selected + cols * page_rows.max(1)).min(last),
        Action::FilterCycle
        | Action::FilterStars(_)
        | Action::FilterStarsOp
        | Action::FilterFlag(_)
        | Action::FilterColor(_)
        | Action::FilterClear
        | Action::FilterBlurry
        | Action::FilterWellExposed
        | Action::FilterStack
        | Action::FilterInvert
        | Action::SortNext
        | Action::SortReverse
        | Action::GridRenderNext
        | Action::ToggleFilterBar
        | Action::ToggleRatingBar
        | Action::ToggleImageInfo
        | Action::BackgroundNext
        | Action::ToggleGridInfo
        | Action::ToggleAutoAdvance
        | Action::SelectToggle
        | Action::Compare
        | Action::TogglePeaking
        | Action::PeakingColor
        | Action::SharpnessLenient
        | Action::SharpnessStrict
        | Action::ToggleFocusPoint
        | Action::OverlayNext
        | Action::OverlayOrient
        | Action::SelectVisual
        | Action::SelectAll
        | Action::SelectClear
        | Action::FileRename
        | Action::FileDelete
        | Action::FileReveal
        | Action::FileEdit
        | Action::FileCopy
        | Action::FileMove
        | Action::FileDeleteStack
        | Action::FileCopyStack
        | Action::FileExport
        | Action::ExportReport
        | Action::Crop
        | Action::CropMove(_)
        | Action::CropSize(_)
        | Action::CropRatio(_)
        | Action::CropRotate
        | Action::CropSave
        | Action::CropPreset
        | Action::CropPresetPrev
        | Action::VersionNext
        | Action::Send(_)
        | Action::Undo
        | Action::Redo
        | Action::History
        | Action::StackToggle
        | Action::StacksAll
        | Action::GroupingWindow
        | Action::StackMode
        | Action::ZoomIn
        | Action::ZoomOut
        | Action::ZoomFit
        | Action::Zoom100
        | Action::StackChoose
        | Action::StackFirst
        | Action::StackLast
        | Action::StackEnterToggle
        | Action::FocusLeft
        | Action::FocusRight
        | Action::FocusNext
        | Action::FolderUp
        | Action::FolderDown
        | Action::FolderExpand
        | Action::FolderCollapse
        | Action::FolderOpen
        | Action::FolderFirst
        | Action::FolderLast
        | Action::FolderOffload
        | Action::FolderEject
        | Action::FolderCreate
        | Action::FolderRename
        | Action::FolderDelete
        | Action::FolderFavorite
        | Action::FolderFavoriteName
        | Action::FolderCopy
        | Action::FolderMove
        | Action::MetaOpen
        | Action::MetaSelectUp
        | Action::MetaSelectDown
        | Action::MetaCopyAll
        | Action::MetaUp
        | Action::MetaDown
        | Action::MetaCopy
        | Action::ToggleFolderPanel
        | Action::Search
        | Action::FocusMode
        | Action::Develop
        | Action::DevelopWith(_)
        | Action::DevelopFolder
        | Action::RefreshFolders
        | Action::ToggleHistogram
        | Action::HistogramScale
        | Action::HistogramLarge
        | Action::HistogramChannel
        | Action::HistogramRed
        | Action::HistogramGreen
        | Action::HistogramBlue
        | Action::HistogramAll
        | Action::HistogramPhoto
        | Action::ToggleClipping
        | Action::ToggleMetadata
        | Action::Open
        | Action::ToggleZoom
        | Action::ToggleSource
        | Action::PanLeft
        | Action::PanRight
        | Action::PanUp
        | Action::PanDown
        | Action::Back
        | Action::Help
        | Action::Settings
        | Action::CheckUpdate
        | Action::ReportIssue
        | Action::Quit
        | Action::Stars(_)
        | Action::Color(_)
        | Action::Flag(_) => selected,
    }
}

/// Défilement nécessaire pour que la ligne `row` soit entièrement visible, la vue
/// commençant à `offset` et faisant `height` de haut. `None` si elle l'est déjà.
pub fn scroll_to_show(layout: &Layout, row: usize, offset: f32, height: f32) -> Option<f32> {
    let top = row as f32 * layout.row_pitch();
    let bottom = top + layout.cell;
    if top < offset {
        Some(top)
    } else if bottom > offset + height {
        Some((bottom - height).max(0.0))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;

    /// 4 colonnes.
    fn layout() -> Layout {
        Layout::fit(4.0 * 100.0 + 3.0 * 10.0, 100.0, 10.0)
    }

    #[test]
    fn fit_all_finds_the_largest_cell_that_shows_everything() {
        // 6 images dans 430 × 210 : 4 colonnes de 100 ne tiennent qu'en 2 lignes de 100
        // (210 ≥ 2 × 100 + 10).
        assert_eq!(fit_all(6, 430.0, 210.0, 10.0, 50.0, 300.0), 100.0);
        // Une seule image : la plus grande taille qui tient en hauteur.
        assert_eq!(fit_all(1, 1000.0, 250.0, 10.0, 50.0, 300.0), 250.0);
        // Trop d'images : la taille minimale.
        assert_eq!(fit_all(10_000, 430.0, 210.0, 10.0, 50.0, 300.0), 50.0);
        // Bornée par le maximum.
        assert_eq!(fit_all(1, 2000.0, 2000.0, 10.0, 50.0, 300.0), 300.0);
    }

    #[test]
    fn cell_steps_stay_within_bounds() {
        assert_eq!(step_cell(200.0, true, 60.0, 600.0), 250.0);
        assert_eq!(step_cell(250.0, false, 60.0, 600.0), 200.0);
        assert_eq!(step_cell(560.0, true, 60.0, 600.0), 600.0);
        assert_eq!(step_cell(70.0, false, 60.0, 600.0), 60.0);
    }

    #[test]
    fn columns_fit_the_width() {
        assert_eq!(layout().columns, 4);
        assert_eq!(layout().width(), 430.0);
        assert_eq!(
            Layout::fit(429.0, 100.0, 10.0).columns,
            3,
            "il manque 1 point"
        );
        assert_eq!(
            Layout::fit(10.0, 100.0, 10.0).columns,
            1,
            "au moins une colonne"
        );
        assert_eq!(layout().rows(9), 3);
        assert_eq!(layout().rows(0), 0);
        assert_eq!(layout().rows_in(330.0), 3);
        assert_eq!(layout().rows_in(5.0), 1);
    }

    #[test]
    fn left_right_stop_at_the_ends() {
        let l = layout();
        assert_eq!(navigate(0, 10, &l, 2, NavPrev), 0);
        assert_eq!(navigate(5, 10, &l, 2, NavPrev), 4);
        assert_eq!(navigate(5, 10, &l, 2, NavNext), 6);
        assert_eq!(navigate(9, 10, &l, 2, NavNext), 9);
    }

    #[test]
    fn up_down_keep_the_column() {
        // 10 images, 4 colonnes :
        // 0 1 2 3
        // 4 5 6 7
        // 8 9
        let l = layout();
        assert_eq!(navigate(5, 10, &l, 2, NavUp), 1);
        assert_eq!(navigate(1, 10, &l, 2, NavUp), 1, "première ligne");
        assert_eq!(navigate(1, 10, &l, 2, NavDown), 5);
        assert_eq!(navigate(5, 10, &l, 2, NavDown), 9);
        assert_eq!(
            navigate(6, 10, &l, 2, NavDown),
            9,
            "ligne incomplète : dernière image"
        );
        assert_eq!(navigate(9, 10, &l, 2, NavDown), 9, "dernière ligne");
        assert_eq!(navigate(8, 10, &l, 2, NavDown), 8);
    }

    #[test]
    fn first_last_and_pages() {
        let l = layout();
        assert_eq!(navigate(6, 10, &l, 2, NavFirst), 0);
        assert_eq!(navigate(2, 10, &l, 2, NavLast), 9);
        assert_eq!(navigate(1, 40, &l, 3, NavPageDown), 13);
        assert_eq!(navigate(38, 40, &l, 3, NavPageDown), 39);
        assert_eq!(navigate(13, 40, &l, 3, NavPageUp), 1);
        assert_eq!(navigate(5, 40, &l, 3, NavPageUp), 0);
    }

    #[test]
    fn empty_grid_and_out_of_range_selection() {
        let l = layout();
        assert_eq!(navigate(0, 0, &l, 2, NavNext), 0);
        assert_eq!(
            navigate(50, 10, &l, 2, NavPrev),
            8,
            "sélection ramenée dans la grille"
        );
        assert_eq!(navigate(3, 10, &l, 2, FilterCycle), 3);
    }

    #[test]
    fn scrolling_brings_the_row_into_view() {
        let l = layout(); // pas de ligne : 110
        assert_eq!(scroll_to_show(&l, 0, 0.0, 300.0), None);
        // ligne 2 : 220..320, dépasse la vue 0..300 de 20.
        assert_eq!(scroll_to_show(&l, 2, 0.0, 300.0), Some(20.0));
    }

    #[test]
    fn scrolling_up_and_down() {
        let l = layout();
        // ligne 3 : 330..430, vue 0..300 → descendre à 130.
        assert_eq!(scroll_to_show(&l, 3, 0.0, 300.0), Some(130.0));
        // ligne 1 : 110..210, vue à 200 → remonter à 110.
        assert_eq!(scroll_to_show(&l, 1, 200.0, 300.0), Some(110.0));
        assert_eq!(scroll_to_show(&l, 2, 130.0, 300.0), None);
    }
}
