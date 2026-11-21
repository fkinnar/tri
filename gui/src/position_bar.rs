//! Barre de position en plein écran (#537, idée de l'utilisateur, 20/11/2026 : en triant,
//! on ne sait plus où on en est ; la barre d'état le dit, mais on ne la regarde pas).
//!
//! Une barre le long du bas de l'image : la liste affichée (filtrée), découpée en
//! segments, un par pile, aussi large que la pile ; la pile en cours éclairée et la photo
//! marquée. Au-dessus de la photo, une petite barre verticale pour la place dans la pile
//! (de bas en haut). Sans piles, la barre seule, sans segments. Position seulement pour
//! l'instant (décidé le 21/11/2026 : les couleurs du tri viendront quand on saura quoi y
//! montrer d'utile).
//!
//! Ici, la géométrie seule (testable sans rien dessiner) et le dessin sur un `Painter`.

use eframe::egui::{self, Color32, Painter, Rect, Stroke, pos2};

/// Ce que la barre montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    /// Nombre de photos de chaque segment (pile) ; une seule valeur sans piles.
    pub segments: Vec<usize>,
    /// Les segments sont des piles (sinon, un seul segment : le dossier, sans barre de
    /// pile).
    pub stacked: bool,
    /// Rang de la photo dans la liste (à partir de 0).
    pub photo: usize,
}

/// Géométrie de la barre, en fractions de sa longueur (0 à 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Début et fin de chaque segment.
    pub segments: Vec<(f32, f32)>,
    /// Segment de la photo, éclairé s'il est une pile (sans piles, le seul segment reste
    /// éteint : sinon, toute la barre semblait remplie).
    pub current: Option<usize>,
    /// Début et fin de la tranche de la photo.
    pub photo: (f32, f32),
    /// Place dans la pile (à partir de 0) et taille de la pile, si elle a plus d'une photo.
    pub in_stack: Option<(usize, usize)>,
}

impl Position {
    /// Géométrie ; `None` sans photo, ou la photo hors de la liste.
    pub fn layout(&self) -> Option<Layout> {
        let total: usize = self.segments.iter().sum();
        if total == 0 || self.photo >= total {
            return None;
        }
        let unit = 1.0 / total as f32;
        let mut segments = Vec::with_capacity(self.segments.len());
        let mut start = 0;
        let mut current = None;
        let mut in_stack = None;
        for (k, &n) in self.segments.iter().enumerate() {
            if (start..start + n).contains(&self.photo) {
                current = self.stacked.then_some(k);
                in_stack = (self.stacked && n > 1).then_some((self.photo - start, n));
            }
            segments.push((start as f32 * unit, (start + n) as f32 * unit));
            start += n;
        }
        let p = self.photo as f32 * unit;
        Some(Layout {
            segments,
            current,
            photo: (p, p + unit),
            in_stack,
        })
    }
}

/// Réglages du dessin (`[gui.position_bar]`) et couleurs.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Épaisseur de la barre (points).
    pub thickness: f32,
    /// Longueur de la barre verticale de la pile (points).
    pub stack_length: f32,
    /// Fond des segments.
    pub track: Color32,
    /// Segment de la pile en cours.
    pub stack: Color32,
    /// Tranche de la photo.
    pub photo: Color32,
}

/// Espace entre deux segments (points) ; supprimé quand les segments deviennent trop
/// fins pour le garder lisible.
const GAP: f32 = 2.0;
/// Largeur minimale de la tranche de la photo, pour qu'elle se voie dans un grand
/// dossier.
const MIN_PHOTO: f32 = 3.0;

/// Dessine la barre dans `rect` (sa longueur et son épaisseur) ; la barre de la pile
/// monte au-dessus de la photo.
pub fn draw(painter: &Painter, rect: Rect, layout: &Layout, style: &Style) {
    let x = |f: f32| rect.left() + f * rect.width();
    let gap = if rect.width() / layout.segments.len() as f32 >= 4.0 * GAP {
        GAP / 2.0
    } else {
        0.0
    };
    let radius = style.thickness / 2.0;
    // Fond sombre sous la barre : lisible sur une image claire.
    painter.rect_filled(
        rect.expand(2.0),
        radius + 2.0,
        Color32::from_black_alpha(110),
    );
    for (k, &(a, b)) in layout.segments.iter().enumerate() {
        let color = if Some(k) == layout.current {
            style.stack
        } else {
            style.track
        };
        let seg = Rect::from_x_y_ranges(
            x(a) + gap..=(x(b) - gap).max(x(a) + gap + 1.0),
            rect.y_range(),
        );
        painter.rect_filled(seg, radius, color);
    }
    let (a, b) = layout.photo;
    let centre = (x(a) + x(b)) / 2.0;
    let half = ((x(b) - x(a)) / 2.0).max(MIN_PHOTO / 2.0);
    let photo = Rect::from_x_y_ranges(centre - half..=centre + half, rect.y_range())
        .expand2(egui::vec2(0.0, 1.5));
    painter.rect_filled(photo, radius, style.photo);
    if let Some((at, n)) = layout.in_stack {
        let length = style.stack_length;
        let bottom = rect.top() - 4.0;
        let column =
            Rect::from_x_y_ranges(centre - radius..=centre + radius, bottom - length..=bottom);
        painter.rect_filled(
            column.expand(2.0),
            radius + 2.0,
            Color32::from_black_alpha(110),
        );
        painter.rect_filled(column, radius, style.track);
        let unit = length / n as f32;
        let low = bottom - unit * at as f32;
        let cell = Rect::from_x_y_ranges(column.x_range(), (low - unit).max(column.top())..=low)
            .expand2(egui::vec2(1.5, 0.0));
        painter.rect_filled(cell, radius, style.photo);
        painter.line_segment(
            [pos2(centre, bottom), pos2(centre, rect.top())],
            Stroke::new(1.0, style.stack),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_as_wide_as_their_stacks() {
        // Piles de 1, 3 et 4 photos ; la 5e photo (rang 4) est la 2e de la pile de 4.
        let l = Position {
            segments: vec![1, 3, 4],
            stacked: true,
            photo: 5,
        }
        .layout()
        .unwrap();
        assert_eq!(l.segments, vec![(0.0, 0.125), (0.125, 0.5), (0.5, 1.0)]);
        assert_eq!(l.current, Some(2));
        assert_eq!(l.photo, (0.625, 0.75));
        assert_eq!(l.in_stack, Some((1, 4)));
    }

    #[test]
    fn a_single_photo_has_no_stack_bar() {
        let l = Position {
            segments: vec![1, 3],
            stacked: true,
            photo: 0,
        }
        .layout()
        .unwrap();
        assert_eq!((l.current, l.in_stack), (Some(0), None));
    }

    #[test]
    fn without_stacks_one_segment_and_no_stack_bar() {
        // Sans piles, un seul segment : la barre ne doit pas monter une « pile » du dossier.
        let l = Position {
            segments: vec![10],
            stacked: false,
            photo: 3,
        }
        .layout()
        .unwrap();
        assert_eq!(l.segments, vec![(0.0, 1.0)]);
        assert_eq!(l.photo, (0.3, 0.4));
        assert_eq!((l.current, l.in_stack), (None, None));
    }

    #[test]
    fn nothing_to_show_outside_the_list() {
        let at = |segments: Vec<usize>, photo| Position {
            segments,
            stacked: true,
            photo,
        };
        assert!(at(vec![], 0).layout().is_none());
        assert!(at(vec![2], 2).layout().is_none());
    }
}
