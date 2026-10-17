//! Dessin des notes d'une image : étoiles, couleurs, drapeau accepté / rejeté.
//!
//! Tout est dessiné en formes, pas en texte : « ★ » ou « ✔ » manquent dans les polices
//! d'egui (comme les flèches, voir `actions::key_label`), et celles du système ne sont pas
//! garanties.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{
    Color32, CursorIcon, Mesh, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, Ui,
    WidgetInfo, pos2, vec2,
};
use tri::xmp::{Color, Flag, Notes};

use crate::actions::Action;
use crate::settings::Colors;

/// Couleur d'écran d'une couleur de notation.
pub fn color_of(colors: &Colors, color: Color) -> Color32 {
    match color {
        Color::Red => colors.red,
        Color::Yellow => colors.yellow,
        Color::Green => colors.green,
        Color::Blue => colors.blue,
        Color::Purple => colors.purple,
    }
    .into()
}

/// Étoile pleine à cinq branches, centrée en `center`, de rayon extérieur `radius`.
pub fn star(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    // Éventail de triangles depuis le centre : une étoile n'est pas convexe, mais tous ses
    // points sont visibles depuis son centre.
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..10 {
        let r = if i % 2 == 0 { radius } else { radius * 0.45 };
        let angle = -FRAC_PI_2 + i as f32 * PI / 5.0;
        mesh.colored_vertex(center + vec2(angle.cos(), angle.sin()) * r, color);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    painter.add(Shape::mesh(mesh));
}

/// Pastille du drapeau : coche verte (acceptée) ou croix rouge (rejetée).
pub fn flag(painter: &Painter, center: Pos2, radius: f32, flag: Flag, colors: &Colors) {
    let (fill, accepted) = match flag {
        Flag::None => return,
        Flag::Accepted => (Color32::from(colors.accepted), true),
        Flag::Rejected => (Color32::from(colors.rejected), false),
    };
    badge(painter, center, radius, accepted, fill);
}

/// Pastille coche (`accepted`) ou croix, de couleur `fill`.
fn badge(painter: &Painter, center: Pos2, radius: f32, accepted: bool, fill: Color32) {
    painter.circle_filled(center, radius, fill);
    let stroke = Stroke::new((radius * 0.28).max(1.5), Color32::WHITE);
    let r = radius * 0.5;
    if accepted {
        painter.line(
            vec![
                center + vec2(-r, 0.0),
                center + vec2(-r * 0.3, r * 0.7),
                center + vec2(r, -r * 0.6),
            ],
            stroke,
        );
    } else {
        painter.line_segment([center + vec2(-r, -r), center + vec2(r, r)], stroke);
        painter.line_segment([center + vec2(-r, r), center + vec2(r, -r)], stroke);
    }
}

/// Largeur occupée par [`row`] pour ces notes, à la taille `size`.
pub fn row_width(notes: &Notes, size: f32) -> f32 {
    let gap = size * 0.25;
    let items = notes.stars as usize + notes.colors.len() + usize::from(notes.flag != Flag::None);
    if items == 0 {
        0.0
    } else {
        items as f32 * size
            + (items - 1) as f32 * gap
            + if notes.stars > 0 && !notes.colors.is_empty() {
                gap
            } else {
                0.0
            }
    }
}

/// Une ligne de notes à partir de `left_center` (bord gauche, milieu vertical) : drapeau,
/// étoiles, puis pastilles de couleur. `size` : hauteur d'un élément. Renvoie le rectangle
/// occupé (vide si aucune note).
pub fn row(
    painter: &Painter,
    left_center: Pos2,
    notes: &Notes,
    size: f32,
    colors: &Colors,
) -> Rect {
    let gap = size * 0.25;
    let radius = size / 2.0;
    let mut x = left_center.x;
    let y = left_center.y;
    if notes.flag != Flag::None {
        flag(painter, pos2(x + radius, y), radius, notes.flag, colors);
        x += size + gap;
    }
    for _ in 0..notes.stars {
        star(painter, pos2(x + radius, y), radius, colors.stars.into());
        x += size + gap;
    }
    if notes.stars > 0 && !notes.colors.is_empty() {
        x += gap; // séparer les étoiles des couleurs
    }
    for &c in &notes.colors {
        painter.circle_filled(pos2(x + radius, y), radius * 0.8, color_of(colors, c));
        x += size + gap;
    }
    Rect::from_min_max(
        pos2(left_center.x, y - radius),
        pos2((x - gap).max(left_center.x), y + radius),
    )
}

/// Case cliquable de côté `size` pour un symbole dessiné (étoile, pastille) : nom
/// accessible et bulle `tip`, et réaction au survol (pointeur en main, fond surligné), pour
/// qu'on voie que c'est cliquable. Le fond est peint avant le symbole, que l'appelant
/// dessine ensuite dans le rectangle renvoyé.
pub fn clickable(ui: &mut Ui, size: f32, tip: &str) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    // Nom lisible pour l'accessibilité (et les tests d'interface).
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, tip));
    let response = response
        .on_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.hovered() {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(rect.expand(2.0), 4.0, fill);
    }
    (rect, response)
}

/// Contrôles de notation cliquables (bandeau de notation, menu du clic droit) : drapeaux,
/// étoiles, couleurs, actifs en couleur, les autres estompés. Renvoie l'action cliquée,
/// la même que celle du clavier : recliquer la note active la retire (drapeau, étoiles),
/// une couleur s'ajoute ou se retire.
pub fn controls(ui: &mut Ui, notes: &Notes, colors: &Colors, size: f32) -> Option<Action> {
    let weak = ui.visuals().weak_text_color();
    let mut clicked = None;
    let cell = |ui: &mut Ui, tip: &str| {
        let (rect, response) = clickable(ui, size, tip);
        (rect, response.clicked())
    };
    for (flag, color) in [
        (Flag::Accepted, colors.accepted),
        (Flag::Rejected, colors.rejected),
    ] {
        let active = notes.flag == flag;
        let action = Action::Flag(if active { Flag::None } else { flag });
        let (rect, hit) = cell(ui, action.label());
        let fill = if active { Color32::from(color) } else { weak };
        badge(
            ui.painter(),
            rect.center(),
            size * 0.45,
            flag == Flag::Accepted,
            fill,
        );
        if hit {
            clicked = Some(action);
        }
    }
    ui.add_space(size * 0.5);
    for n in 1..=5u8 {
        let action = Action::Stars(if n == notes.stars { 0 } else { n });
        let (rect, hit) = cell(ui, action.label());
        let fill = if n <= notes.stars {
            Color32::from(colors.stars)
        } else {
            weak
        };
        star(ui.painter(), rect.center(), size / 2.0, fill);
        if hit {
            clicked = Some(action);
        }
    }
    ui.add_space(size * 0.5);
    for color in Color::ALL {
        let action = Action::Color(color);
        let (rect, hit) = cell(ui, action.label());
        let active = notes.has_color(color);
        let fill = color_of(colors, color);
        let painter = ui.painter();
        if active {
            painter.circle_filled(rect.center(), size * 0.4, fill);
            painter.circle_stroke(
                rect.center(),
                size * 0.48,
                Stroke::new(2.0, ui.visuals().strong_text_color()),
            );
        } else {
            painter.circle_filled(rect.center(), size * 0.3, fill.gamma_multiply(0.4));
        }
        if hit {
            clicked = Some(action);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_counts_every_mark() {
        let size = 10.0;
        assert_eq!(row_width(&Notes::default(), size), 0.0);
        let notes = Notes {
            stars: 2,
            flag: Flag::Accepted,
            colors: vec![Color::Red],
        };
        // 4 éléments, 3 espaces, plus l'espace qui sépare étoiles et couleurs.
        assert_eq!(row_width(&notes, size), 4.0 * 10.0 + 3.0 * 2.5 + 2.5);
    }
}
