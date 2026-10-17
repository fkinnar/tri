//! Comparaison côte à côte : disposition des images dans la fenêtre, sans dépendance à
//! l'état de l'application (testable seule). Le dessin est dans `app.rs`
//! (`TriApp::compare_view`), avec le chargement des images du plein écran.

use eframe::egui::{Rect, pos2, vec2};

/// Disposition de `n` images de proportions `aspect` (largeur / hauteur) dans `area`, avec
/// `gap` points entre elles : le nombre de colonnes qui montre les images le plus grand,
/// puis les cases, ligne par ligne. La dernière ligne incomplète est centrée.
pub fn layout(n: usize, area: Rect, aspect: f32, gap: f32) -> (usize, Vec<Rect>) {
    if n == 0 {
        return (1, Vec::new());
    }
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.5
    };
    let cell = |cols: usize| {
        let rows = n.div_ceil(cols);
        let w = (area.width() - gap * (cols - 1) as f32) / cols as f32;
        let h = (area.height() - gap * (rows - 1) as f32) / rows as f32;
        (w.max(1.0), h.max(1.0))
    };
    // Surface de l'image ajustée dans une case.
    let shown = |(w, h): (f32, f32)| {
        let width = w.min(h * aspect);
        width * width / aspect
    };
    let cols = (1..=n)
        .max_by(|&a, &b| shown(cell(a)).total_cmp(&shown(cell(b))))
        .unwrap_or(1);
    let (w, h) = cell(cols);
    let rows = n.div_ceil(cols);
    let mut rects = Vec::with_capacity(n);
    for row in 0..rows {
        let in_row = (n - row * cols).min(cols);
        // Dernière ligne incomplète : centrée.
        let shift = (cols - in_row) as f32 * (w + gap) / 2.0;
        for col in 0..in_row {
            let min = pos2(
                area.min.x + shift + col as f32 * (w + gap),
                area.min.y + row as f32 * (h + gap),
            );
            rects.push(Rect::from_min_size(min, vec2(w, h)));
        }
    }
    (cols, rects)
}

/// Case voisine de `active` parmi `n` cases sur `cols` colonnes, dans la direction
/// (`dx`, `dy`) : bornée aux bords (pas de bouclage).
pub fn neighbour(active: usize, n: usize, cols: usize, dx: i32, dy: i32) -> usize {
    if n == 0 {
        return 0;
    }
    let cols = cols.max(1) as i32;
    let target = active as i32 + dx + dy * cols;
    if dy != 0 && !(0..n as i32).contains(&target) {
        return active;
    }
    target.clamp(0, n as i32 - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
    }

    #[test]
    fn two_landscape_images_side_by_side_in_a_wide_window() {
        let (cols, rects) = layout(2, area(1600.0, 800.0), 1.5, 0.0);
        assert_eq!(cols, 2);
        assert_eq!(rects[1].min.x, 800.0);
    }

    #[test]
    fn two_portraits_side_by_side_and_four_in_a_square() {
        assert_eq!(layout(2, area(1000.0, 1000.0), 2.0 / 3.0, 0.0).0, 2);
        assert_eq!(layout(4, area(1000.0, 1000.0), 1.0, 0.0).0, 2);
        // Paysages dans une fenêtre très haute : l'une sous l'autre.
        assert_eq!(layout(2, area(600.0, 1400.0), 1.5, 0.0).0, 1);
    }

    #[test]
    fn last_incomplete_row_is_centered() {
        let (cols, rects) = layout(3, area(1000.0, 1000.0), 1.0, 0.0);
        assert_eq!(cols, 2);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[2].center().x, 500.0);
    }

    #[test]
    fn neighbours_stay_inside() {
        // 5 cases sur 3 colonnes : 0 1 2 / 3 4.
        assert_eq!(neighbour(0, 5, 3, -1, 0), 0);
        assert_eq!(
            neighbour(2, 5, 3, 1, 0),
            3,
            "à droite au bout de la ligne : suivante"
        );
        assert_eq!(neighbour(1, 5, 3, 0, 1), 4);
        assert_eq!(neighbour(2, 5, 3, 0, 1), 2, "pas de case dessous");
        assert_eq!(neighbour(4, 5, 3, 0, -1), 1);
        assert_eq!(neighbour(0, 0, 3, 1, 0), 0);
    }
}
