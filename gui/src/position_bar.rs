//! Barre de position en plein écran (#537, idée de l'utilisateur, 20/11/2026 : en triant,
//! on ne sait plus où on en est ; la barre d'état le dit, mais on ne la regarde pas).
//!
//! Deux formes à l'essai (`[gui.position_bar] shape`, 21/11/2026) :
//!
//! - **segmentée** : une barre, la liste affichée (filtrée) découpée en segments, un par
//!   pile, aussi large que la pile ; la pile en cours éclairée et la photo marquée ;
//! - **trois barres** (idée de l'utilisateur, plus lisible avec de petites piles dans un
//!   grand dossier) : en haut, la photo dans la pile et la pile dans la liste, côte à côte ;
//!   en dessous, la photo dans la liste, sur toute la largeur.
//!
//! Sans piles, une seule barre, sans segments. Sur l'image ou dans le panneau de droite
//! (`place`). Position seulement pour l'instant (décidé le 21/11/2026 : les couleurs du
//! tri viendront quand on saura quoi y montrer d'utile).
//!
//! Ici, la géométrie (testable sans rien dessiner) et le dessin sur un `Painter`.

use eframe::egui::{Color32, Painter, Rect};

use crate::settings::BarShape;

/// Ce que la barre montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    /// Nombre de photos de chaque segment (pile) ; une seule valeur sans piles.
    pub segments: Vec<usize>,
    /// Les segments sont des piles (sinon, un seul segment : la liste).
    pub stacked: bool,
    /// Rang de la photo dans la liste (à partir de 0).
    pub photo: usize,
}

/// Géométrie de la barre segmentée, en fractions de sa longueur (0 à 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Début et fin de chaque segment.
    pub segments: Vec<(f32, f32)>,
    /// Segment de la photo, éclairé s'il est une pile (sans piles, le seul segment reste
    /// éteint : sinon, toute la barre semblait remplie).
    pub current: Option<usize>,
    /// Début et fin de la tranche de la photo.
    pub photo: (f32, f32),
}

/// Les trois barres : chacune un rang (à partir de 0) parmi un nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split {
    /// La photo dans sa pile ; `None` sans piles.
    pub in_stack: Option<(usize, usize)>,
    /// La pile dans la liste ; `None` sans piles.
    pub stack: Option<(usize, usize)>,
    /// La photo dans la liste.
    pub photo: (usize, usize),
}

impl Position {
    /// Géométrie de la barre segmentée ; `None` sans photo, ou la photo hors de la liste.
    pub fn layout(&self) -> Option<Layout> {
        let total: usize = self.segments.iter().sum();
        if total == 0 || self.photo >= total {
            return None;
        }
        let unit = 1.0 / total as f32;
        let mut segments = Vec::with_capacity(self.segments.len());
        let mut start = 0;
        let mut current = None;
        for (k, &n) in self.segments.iter().enumerate() {
            if (start..start + n).contains(&self.photo) {
                current = self.stacked.then_some(k);
            }
            segments.push((start as f32 * unit, (start + n) as f32 * unit));
            start += n;
        }
        let p = self.photo as f32 * unit;
        Some(Layout {
            segments,
            current,
            photo: (p, p + unit),
        })
    }

    /// Les trois barres ; `None` sans photo, ou la photo hors de la liste.
    pub fn split(&self) -> Option<Split> {
        let total: usize = self.segments.iter().sum();
        if total == 0 || self.photo >= total {
            return None;
        }
        let mut start = 0;
        let mut stacks = None;
        for (k, &n) in self.segments.iter().enumerate() {
            if (start..start + n).contains(&self.photo) {
                stacks = Some(((self.photo - start, n), (k, self.segments.len())));
                break;
            }
            start += n;
        }
        let stacks = stacks.filter(|_| self.stacked);
        Some(Split {
            in_stack: stacks.map(|s| s.0),
            stack: stacks.map(|s| s.1),
            photo: (self.photo, total),
        })
    }

    /// Hauteur dessinée par la forme `shape` avec des barres d'épaisseur `thickness`.
    pub fn height(&self, shape: BarShape, thickness: f32) -> f32 {
        match shape {
            BarShape::Split if self.stacked => 2.0 * thickness + ROW_GAP,
            _ => thickness,
        }
    }
}

/// Couleurs du dessin.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Épaisseur d'une barre (points).
    pub thickness: f32,
    /// Fond des barres et des segments.
    pub track: Color32,
    /// Segment de la pile en cours.
    pub stack: Color32,
    /// Tranche de la photo.
    pub photo: Color32,
    /// Fond sombre sous les barres (sur l'image : lisible sur une image claire).
    pub backdrop: Option<Color32>,
}

/// Espace entre deux segments (points) ; supprimé quand les segments deviennent trop
/// fins pour le garder lisible.
const GAP: f32 = 2.0;
/// Espace entre les deux barres du haut, et entre les deux lignes (trois barres).
const ROW_GAP: f32 = 6.0;
/// Largeur minimale de la tranche de la photo, pour qu'elle se voie dans un grand
/// dossier.
const MIN_PHOTO: f32 = 3.0;

/// Dessine la position dans `rect` (sa largeur ; sa hauteur : [`Position::height`]).
pub fn draw(painter: &Painter, rect: Rect, position: &Position, shape: BarShape, style: &Style) {
    if let Some(color) = style.backdrop {
        painter.rect_filled(rect.expand(3.0), style.thickness / 2.0 + 3.0, color);
    }
    match shape {
        BarShape::Segmented => {
            if let Some(layout) = position.layout() {
                segmented(painter, rect, &layout, style);
            }
        }
        BarShape::Split => {
            let Some(split) = position.split() else {
                return;
            };
            let t = style.thickness;
            let bottom = Rect::from_x_y_ranges(rect.x_range(), rect.bottom() - t..=rect.bottom());
            if let (Some(in_stack), Some(stack)) = (split.in_stack, split.stack) {
                let top = rect.top()..=rect.top() + t;
                let half = (rect.width() - ROW_GAP) / 2.0;
                let left = Rect::from_x_y_ranges(rect.left()..=rect.left() + half, top.clone());
                let right = Rect::from_x_y_ranges(rect.right() - half..=rect.right(), top);
                bar(painter, left, in_stack, style);
                bar(painter, right, stack, style);
            }
            bar(painter, bottom, split.photo, style);
        }
    }
}

/// Une barre simple : le rang `at` parmi `n`, marqué.
fn bar(painter: &Painter, rect: Rect, (at, n): (usize, usize), style: &Style) {
    let radius = style.thickness / 2.0;
    painter.rect_filled(rect, radius, style.track);
    let unit = rect.width() / n.max(1) as f32;
    slice(painter, rect, rect.left() + unit * at as f32, unit, style);
}

/// La tranche de la photo, de `start` sur `width`, élargie au besoin.
fn slice(painter: &Painter, rect: Rect, start: f32, width: f32, style: &Style) {
    let centre = start + width / 2.0;
    let half = (width / 2.0).max(MIN_PHOTO / 2.0);
    let photo = Rect::from_x_y_ranges(centre - half..=centre + half, rect.y_range())
        .expand2(eframe::egui::vec2(0.0, 1.5));
    painter.rect_filled(photo, style.thickness / 2.0, style.photo);
}

fn segmented(painter: &Painter, rect: Rect, layout: &Layout, style: &Style) {
    let x = |f: f32| rect.left() + f * rect.width();
    let gap = if rect.width() / layout.segments.len() as f32 >= 4.0 * GAP {
        GAP / 2.0
    } else {
        0.0
    };
    let radius = style.thickness / 2.0;
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
    slice(painter, rect, x(a), x(b) - x(a), style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(segments: Vec<usize>, stacked: bool, photo: usize) -> Position {
        Position {
            segments,
            stacked,
            photo,
        }
    }

    #[test]
    fn segments_are_as_wide_as_their_stacks() {
        // Piles de 1, 3 et 4 photos ; la 6e photo (rang 5) est la 2e de la pile de 4.
        let l = at(vec![1, 3, 4], true, 5).layout().unwrap();
        assert_eq!(l.segments, vec![(0.0, 0.125), (0.125, 0.5), (0.5, 1.0)]);
        assert_eq!(l.current, Some(2));
        assert_eq!(l.photo, (0.625, 0.75));
    }

    #[test]
    fn without_stacks_one_unlit_segment() {
        let l = at(vec![10], false, 3).layout().unwrap();
        assert_eq!(l.segments, vec![(0.0, 1.0)]);
        assert_eq!(l.photo, (0.3, 0.4));
        assert_eq!(l.current, None);
    }

    #[test]
    fn three_bars_photo_in_stack_stack_in_list_photo_in_list() {
        let s = at(vec![1, 3, 4], true, 5).split().unwrap();
        assert_eq!(
            s,
            Split {
                in_stack: Some((1, 4)),
                stack: Some((2, 3)),
                photo: (5, 8),
            }
        );
        // Une photo seule : une pile d'une photo.
        let s = at(vec![1, 3, 4], true, 0).split().unwrap();
        assert_eq!((s.in_stack, s.stack), (Some((0, 1)), Some((0, 3))));
    }

    #[test]
    fn three_bars_without_stacks_is_the_list_bar_alone() {
        let p = at(vec![10], false, 3);
        let s = p.split().unwrap();
        assert_eq!((s.in_stack, s.stack, s.photo), (None, None, (3, 10)));
        assert_eq!(p.height(BarShape::Split, 5.0), 5.0, "une ligne");
        assert_eq!(at(vec![2, 8], true, 3).height(BarShape::Split, 5.0), 16.0);
    }

    #[test]
    fn nothing_to_show_outside_the_list() {
        assert!(at(vec![], true, 0).layout().is_none());
        assert!(at(vec![2], true, 2).layout().is_none());
        assert!(at(vec![2], true, 2).split().is_none());
    }
}
