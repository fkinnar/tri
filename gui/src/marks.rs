//! Dessin des notes d'une image : étoiles, couleurs, drapeau accepté / rejeté.
//!
//! Tout est dessiné en formes, pas en texte : « ★ » ou « ✔ » manquent dans les polices
//! d'egui (comme les flèches, voir `actions::key_label`), et celles du système ne sont pas
//! garanties.

use std::f32::consts::{FRAC_PI_2, PI};

use eframe::egui::{
    self, Color32, CursorIcon, Mesh, Painter, Pos2, Rect, Response, Role, Sense, Shape, Stroke, Ui,
    WidgetInfo, pos2, vec2,
};
use tri::xmp::{Color, Flag, Notes};

use crate::actions::Action;
use crate::settings::{Colors, FocusMarker};

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
    badge_with_glyph(painter, center, radius, accepted, fill, Color32::WHITE);
}

/// [`badge`] avec la couleur du signe `glyph` (estompée avec la pastille, bandeau de
/// notation : retour du 31/10/2026, les drapeaux non choisis en couleur fondue comme les
/// pastilles de couleur, plus en gris).
fn badge_with_glyph(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    accepted: bool,
    fill: Color32,
    glyph: Color32,
) {
    painter.circle_filled(center, radius, fill);
    let stroke = Stroke::new((radius * 0.28).max(1.5), glyph);
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

/// Opacité d'un contrôle de notation non choisi (pastilles de couleur, drapeaux).
const FADED: f32 = 0.4;

/// Cercle vide du drapeau « neutre » (retour du 31/10/2026 : la même icône partout —
/// vignette, bandeau de notation, filtre), de la taille des pastilles coche / croix.
pub fn neutral_ring(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    // Un anneau paraît plus grand qu'un disque plein de même rayon (retour du
    // 31/10/2026) : un peu plus petit pour sembler de la même taille.
    let radius = radius * 0.88;
    let width = (radius * 0.28).max(1.5);
    painter.circle_stroke(center, radius - width / 2.0, Stroke::new(width, color));
}

/// Pastille de couleur d'un contrôle (bandeau de notation, sélecteur, barre des filtres) :
/// pleine quand elle est posée ou active, plus petite et fondue sinon (même effet
/// partout, retour du 31/10/2026).
pub fn color_dot(painter: &Painter, center: Pos2, size: f32, fill: Color32, active: bool) {
    if active {
        painter.circle_filled(center, size * 0.4, fill);
    } else {
        painter.circle_filled(center, size * 0.3, fill.gamma_multiply(FADED));
    }
}

/// Couleur du texte ou du signe posé sur un fond `fill` : blanc sur un fond sombre, noir
/// sur un fond clair.
pub fn on_color(fill: Color32) -> Color32 {
    let luminance =
        0.2126 * f32::from(fill.r()) + 0.7152 * f32::from(fill.g()) + 0.0722 * f32::from(fill.b());
    if luminance < 140.0 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// Couleur d'un contrôle non choisi ou d'un filtre éteint : la même, fondue.
pub fn faded(color: Color32) -> Color32 {
    color.gamma_multiply(FADED)
}

/// Bouton d'un drapeau dans la barre des filtres (31/10/2026 : trois boutons combinables,
/// acceptées / neutres / rejetées), dans `rect` : la pastille du drapeau, ou le cercle du
/// neutre, en couleur si `active`, fondu sinon.
pub fn flag_toggle(painter: &Painter, rect: Rect, flag: Flag, colors: &Colors, active: bool) {
    let (center, radius) = (rect.center(), rect.width() * 0.4);
    let fade = |c: Color32| if active { c } else { faded(c) };
    match flag {
        Flag::Accepted => badge_with_glyph(
            painter,
            center,
            radius,
            true,
            fade(Color32::from(colors.accepted)),
            fade(Color32::WHITE),
        ),
        Flag::Rejected => badge_with_glyph(
            painter,
            center,
            radius,
            false,
            fade(Color32::from(colors.rejected)),
            fade(Color32::WHITE),
        ),
        Flag::None => neutral_ring(painter, center, radius, fade(Color32::from_gray(230))),
    }
}

/// Contour de goutte, discret (photo jugée floue, sous la vignette ; retour du 21/10/2026 :
/// le mot « floue » en toutes lettres).
pub fn blur_drop_outline(painter: &Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new((rect.width() * 0.09).max(1.0), color);
    painter.add(Shape::closed_line(drop_points(rect), stroke));
}

/// Pourtour d'une goutte dans `rect`.
fn drop_points(rect: Rect) -> Vec<Pos2> {
    // Cercle en bas, pointe en haut : deux tangentes du sommet au cercle.
    let r = rect.width() * 0.28;
    let c = rect.center() + vec2(0.0, rect.height() * 0.12);
    let tip = pos2(rect.center().x, rect.top() + rect.height() * 0.08);
    let d = c.y - tip.y;
    // Angle des points de tangence, depuis la verticale.
    let a = (r / d).clamp(-1.0, 1.0).acos();
    let mut points = vec![tip];
    let steps = 24;
    for i in 0..=steps {
        // De la tangence droite à la tangence gauche, par le bas.
        let t = -FRAC_PI_2 + a + (2.0 * PI - 2.0 * a) * i as f32 / steps as f32;
        points.push(c + vec2(t.cos(), t.sin()) * r);
    }
    points
}

/// Icône « netteté » : un triangle pointe en bas, de la surface de la goutte (filtre
/// « sans les floues », retour du 21/10/2026 : la goutte barrée ne se comprenait pas).
pub fn sharp_triangle(painter: &Painter, rect: Rect, color: Color32) {
    let side = rect.width() * 0.74;
    let height = side * 3f32.sqrt() / 2.0;
    let c = rect.center();
    let top = c.y - height / 2.0;
    let points = vec![
        pos2(c.x - side / 2.0, top),
        pos2(c.x + side / 2.0, top),
        pos2(c.x, top + height),
    ];
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Fond sombre des marques posées sur une vignette : lisibles sur une photo claire comme
/// sombre.
const RIM: Color32 = Color32::from_black_alpha(170);

/// Notes posées sur une vignette de la grille, dans `area` (retour du 21/10/2026 : plus de
/// rectangle sombre, chaque marque cerclée de sombre comme les pastilles) : étoiles en bas
/// à gauche, drapeau en bas à droite, couleurs en haut à droite. `size` : taille d'une
/// marque.
pub fn thumbnail_notes(
    painter: &Painter,
    area: Rect,
    notes: &Notes,
    size: f32,
    colors: &Colors,
    neutral: bool,
) {
    let radius = size / 2.0;
    let gap = size * 0.25;
    let inset = radius + 3.0;
    let star_fill = chip_star_color(painter.ctx(), colors);
    let rim = (radius * 0.18).max(1.2);
    for k in 0..notes.stars {
        let c = pos2(
            area.min.x + inset + k as f32 * (size + gap),
            area.max.y - inset,
        );
        star(painter, c, radius + rim * 1.6, RIM);
        star(painter, c, radius, star_fill);
    }
    if notes.flag != Flag::None {
        let c = pos2(area.max.x - inset, area.max.y - inset);
        painter.circle_filled(c, radius + rim, RIM);
        flag(painter, c, radius, notes.flag, colors);
    } else if neutral {
        // Neutre : un seul cercle sombre à la même place (`[gui.grid] neutral_flag`,
        // 31/10/2026 : chaque vignette montre toutes ses informations ; retour du même
        // jour : juste un cercle, sans blanc), de la couleur du liseré des autres marques.
        let c = pos2(area.max.x - inset, area.max.y - inset);
        neutral_ring(painter, c, radius + rim, RIM);
    }
    for (k, &color) in notes.colors.iter().enumerate() {
        let c = pos2(
            area.max.x - inset - k as f32 * (size + gap),
            area.min.y + inset,
        );
        painter.circle_filled(c, radius * 0.8 + rim, RIM);
        painter.circle_filled(c, radius * 0.8, color_of(colors, color));
    }
}

/// Groupe de marques d'une vignette qu'un clic peut modifier (#257, 31/10/2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// Les étoiles, en bas à gauche.
    Stars,
    /// Le drapeau, en bas à droite.
    Flag,
    /// Les couleurs, en haut à droite.
    Colors,
}

/// Zones cliquables des marques d'une vignette (même géométrie que
/// [`thumbnail_notes`]) : les cinq emplacements d'étoiles, qu'elles soient posées ou non,
/// le drapeau (posé ou cercle du neutre), les couleurs (au moins un emplacement).
pub fn thumbnail_mark_zones(area: Rect, notes: &Notes, size: f32) -> [(MarkKind, Rect); 3] {
    let radius = size / 2.0;
    let gap = size * 0.25;
    let inset = radius + 3.0;
    let slot = size + gap;
    let stars = Rect::from_min_max(
        pos2(area.min.x + inset - radius, area.max.y - inset - radius),
        pos2(
            area.min.x + inset + 4.0 * slot + radius,
            area.max.y - inset + radius,
        ),
    );
    let flag = Rect::from_center_size(
        pos2(area.max.x - inset, area.max.y - inset),
        vec2(size, size),
    );
    let n = notes.colors.len().max(1) as f32;
    let colors = Rect::from_min_max(
        pos2(
            area.max.x - inset - (n - 1.0) * slot - radius,
            area.min.y + inset - radius,
        ),
        pos2(area.max.x - inset + radius, area.min.y + inset + radius),
    );
    [
        (MarkKind::Stars, stars),
        (MarkKind::Flag, flag),
        (MarkKind::Colors, colors),
    ]
}

/// Badge d'écrêtage d'une vignette, dans le carré `rect` : un triangle rectangle (angle
/// droit en bas à droite) cerclé de sombre, moitié droite blanche et gauche noire pour une
/// photo brûlée (`over`), l'inverse pour une photo bouchée. Retour du 21/10/2026 : la
/// pastille magenta se confondait avec un libellé de couleur, puis un petit histogramme
/// ne se lisait pas.
pub fn clip_badge(painter: &Painter, rect: Rect, over: bool) {
    let r = rect.shrink(rect.width() * 0.08);
    let (a, b, c) = (r.left_bottom(), r.right_top(), r.right_bottom());
    let rim = Stroke::new((rect.width() * 0.12).max(1.5), RIM);
    painter.add(Shape::convex_polygon(vec![a, b, c], RIM, rim));
    // Coupé au milieu par une verticale : à gauche, le petit triangle ; à droite, le reste.
    let mid = r.center().x;
    let m_top = pos2(mid, r.center().y);
    let m_bottom = pos2(mid, r.bottom());
    let (left, right) = if over {
        (Color32::BLACK, Color32::WHITE)
    } else {
        (Color32::WHITE, Color32::BLACK)
    };
    painter.add(Shape::convex_polygon(
        vec![a, m_top, m_bottom],
        left,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![m_top, b, c, m_bottom],
        right,
        Stroke::NONE,
    ));
}

/// Icône des infos des vignettes : une vignette (cadre) et la ligne de texte dessous.
pub fn grid_info_icon(painter: &Painter, rect: Rect, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let p = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.1).max(1.2), color);
    painter.rect_stroke(
        Rect::from_min_max(p(0.12, 0.08), p(0.88, 0.62)),
        2.0,
        stroke,
        egui::StrokeKind::Middle,
    );
    painter.line_segment([p(0.12, 0.86), p(0.62, 0.86)], stroke);
    painter.line_segment([p(0.74, 0.86), p(0.88, 0.86)], stroke);
}

/// Icône « bien exposées » : un petit soleil (disque et huit rayons).
pub fn sun(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    painter.circle_filled(c, s * 0.2, color);
    let stroke = Stroke::new((s * 0.09).max(1.2), color);
    for k in 0..8 {
        let a = std::f32::consts::TAU * k as f32 / 8.0;
        let d = vec2(a.cos(), a.sin());
        painter.line_segment([c + d * s * 0.3, c + d * s * 0.44], stroke);
    }
}

/// Icône du filtre « développées » : une bobine de film (cercle, cinq trous autour de
/// l'axe), avec la pellicule qui en part vers la droite.
pub fn film_reel(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center() - vec2(s * 0.06, 0.0);
    let r = s * 0.36;
    // Trait fin (retour du 31/10/2026 : trop épais sur les vignettes).
    let stroke = Stroke::new((s * 0.055).max(1.0), color);
    painter.circle_stroke(c, r, stroke);
    painter.circle_filled(c, s * 0.055, color);
    for k in 0..5 {
        let a = -FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 5.0;
        painter.circle_filled(c + vec2(a.cos(), a.sin()) * r * 0.55, s * 0.08, color);
    }
    // La pellicule, tangente au bas de la bobine.
    painter.line_segment([c + vec2(0.0, r), c + vec2(s * 0.5, r)], stroke);
}

/// Icône « inverser les filtres » : un cercle à moitié plein (◐).
pub fn invert_icon(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    // Rayon d'une pastille de couleur de la barre des filtres, contour compris.
    let (c, r) = (rect.center(), s * 0.31);
    painter.circle_stroke(c, r, Stroke::new(1.2, color));
    // Moitié gauche pleine : demi-disque en éventail depuis le centre.
    let steps = 16;
    let mut points = vec![c + vec2(0.0, -r)];
    for i in 0..=steps {
        let a = -FRAC_PI_2 - PI * i as f32 / steps as f32;
        points.push(c + vec2(a.cos(), a.sin()) * r);
    }
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Icône « effacer les filtres » : un entonnoir barré.
pub fn funnel_cleared(painter: &Painter, rect: Rect, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let p = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.1).max(1.2), color);
    painter.add(Shape::closed_line(
        vec![
            p(0.1, 0.12),
            p(0.9, 0.12),
            p(0.58, 0.5),
            p(0.58, 0.88),
            p(0.42, 0.78),
            p(0.42, 0.5),
        ],
        stroke,
    ));
    painter.line_segment([p(0.08, 0.92), p(0.92, 0.08)], stroke);
}

/// Icône « tout à l'écran » de la grille : neuf petites vignettes.
pub fn grid_fit_icon(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let cell = s * 0.26;
    let step = s * 0.37;
    let origin = rect.center() - vec2(step + cell / 2.0, step + cell / 2.0);
    for i in 0..3 {
        for j in 0..3 {
            let min = origin + vec2(i as f32 * step, j as f32 * step);
            painter.rect_filled(Rect::from_min_size(min, vec2(cell, cell)), 1.0, color);
        }
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
        star(
            painter,
            pos2(x + radius, y),
            radius,
            chip_star_color(painter.ctx(), colors),
        );
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
    clickable_named(ui, size, tip, tip)
}

/// [`clickable`] dont le nom accessible (`label`) diffère de la bulle (`tip`, avec les
/// raccourcis par exemple).
pub fn clickable_named(ui: &mut Ui, size: f32, label: &str, tip: &str) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    // Nom lisible pour l'accessibilité (et les tests d'interface).
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let response = response
        .on_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.hovered() {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(rect.expand(2.0), 4.0, fill);
    }
    (rect, response)
}

/// Bouton carré de côté `size`, au fond de bouton du thème (survol, appui) : nom `label`
/// pour l'accessibilité et les tests, bulle `tip`. L'icône est dessinée ensuite par
/// l'appelant dans le rectangle renvoyé.
pub fn icon_button(ui: &mut Ui, size: f32, label: &str, tip: &str) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let response = response
        .on_hover_text(tip)
        .on_disabled_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand);
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    (rect, response)
}

/// Icône d'une action sur un fichier dans `rect` : crayon (renommer), poubelle (corbeille),
/// dossier (afficher dans le gestionnaire de fichiers), flèche sortant d'un cadre (ouvrir
/// dans l'éditeur externe).
pub fn file_action(painter: &Painter, rect: Rect, action: Action, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let p = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.09).max(1.5), color);
    match action {
        Action::FileRename => {
            // Crayon en diagonale : corps, pointe, et la ligne d'écriture.
            let body = vec![p(0.62, 0.08), p(0.92, 0.38), p(0.42, 0.88), p(0.12, 0.58)];
            painter.add(Shape::closed_line(body, stroke));
            painter.line_segment([p(0.12, 0.58), p(0.0, 1.0)], stroke);
            painter.line_segment([p(0.42, 0.88), p(0.0, 1.0)], stroke);
            painter.line_segment([p(0.52, 0.18), p(0.82, 0.48)], stroke);
        }
        Action::FileDelete => {
            // Poubelle : couvercle, poignée, corps et rainures.
            painter.line_segment([p(0.05, 0.2), p(0.95, 0.2)], stroke);
            painter.add(Shape::line(
                vec![p(0.35, 0.2), p(0.38, 0.05), p(0.62, 0.05), p(0.65, 0.2)],
                stroke,
            ));
            painter.add(Shape::closed_line(
                vec![p(0.15, 0.2), p(0.85, 0.2), p(0.77, 1.0), p(0.23, 1.0)],
                stroke,
            ));
            for x in [0.38, 0.62] {
                painter.line_segment([p(x, 0.38), p(x, 0.84)], stroke);
            }
        }
        Action::Sidecars => {
            // Sidecar : une feuille au coin corné, et un point (la note) à côté.
            painter.add(Shape::closed_line(
                vec![
                    p(0.06, 0.04),
                    p(0.5, 0.04),
                    p(0.68, 0.22),
                    p(0.68, 0.96),
                    p(0.06, 0.96),
                ],
                stroke,
            ));
            painter.line_segment([p(0.5, 0.04), p(0.5, 0.22)], stroke);
            painter.line_segment([p(0.5, 0.22), p(0.68, 0.22)], stroke);
            painter.circle_filled(p(0.86, 0.8), w * 0.1, color);
        }
        Action::FileReveal => {
            // Dossier : onglet puis corps.
            painter.add(Shape::closed_line(
                vec![
                    p(0.0, 0.12),
                    p(0.38, 0.12),
                    p(0.48, 0.26),
                    p(1.0, 0.26),
                    p(1.0, 0.92),
                    p(0.0, 0.92),
                ],
                stroke,
            ));
            painter.line_segment([p(0.0, 0.4), p(1.0, 0.4)], stroke);
        }
        Action::FileCopy => {
            // Deux feuilles décalées : celle de derrière seulement en partie visible.
            painter.add(Shape::line(
                vec![
                    p(0.3, 0.3),
                    p(0.0, 0.3),
                    p(0.0, 1.0),
                    p(0.7, 1.0),
                    p(0.7, 0.7),
                ],
                stroke,
            ));
            painter.add(Shape::closed_line(
                vec![p(0.3, 0.0), p(1.0, 0.0), p(1.0, 0.7), p(0.3, 0.7)],
                stroke,
            ));
        }
        Action::FileMove => {
            // Dossier, et une flèche qui y entre par la gauche.
            painter.add(Shape::line(
                vec![
                    p(0.3, 0.42),
                    p(0.3, 0.12),
                    p(0.6, 0.12),
                    p(0.7, 0.26),
                    p(1.0, 0.26),
                    p(1.0, 0.92),
                    p(0.3, 0.92),
                    p(0.3, 0.72),
                ],
                stroke,
            ));
            painter.line_segment([p(0.0, 0.57), p(0.72, 0.57)], stroke);
            painter.add(Shape::line(
                vec![p(0.52, 0.4), p(0.72, 0.57), p(0.52, 0.74)],
                stroke,
            ));
        }
        Action::Send(_) => {
            // Partager (comme macOS) : boîte ouverte en haut, flèche qui en monte.
            painter.add(Shape::line(
                vec![
                    p(0.3, 0.38),
                    p(0.1, 0.38),
                    p(0.1, 0.98),
                    p(0.9, 0.98),
                    p(0.9, 0.38),
                    p(0.7, 0.38),
                ],
                stroke,
            ));
            painter.line_segment([p(0.5, 0.02), p(0.5, 0.68)], stroke);
            painter.add(Shape::line(
                vec![p(0.3, 0.22), p(0.5, 0.02), p(0.7, 0.22)],
                stroke,
            ));
        }
        _ => {
            // Ouvrir ailleurs : cadre ouvert en haut à droite, flèche qui en sort.
            painter.add(Shape::line(
                vec![
                    p(0.45, 0.12),
                    p(0.05, 0.12),
                    p(0.05, 0.95),
                    p(0.88, 0.95),
                    p(0.88, 0.55),
                ],
                stroke,
            ));
            painter.line_segment([p(0.42, 0.58), p(0.98, 0.02)], stroke);
            painter.add(Shape::line(
                vec![p(0.6, 0.02), p(0.98, 0.02), p(0.98, 0.4)],
                stroke,
            ));
        }
    }
}

/// Icône du bandeau de notation dans `rect` : flèche (image précédente / suivante), ou
/// étoile suivie d'une flèche (avance automatique) ; `on` : bouton allumé, sur le fond d'accent. L'étoile de l'avance
/// automatique, à l'accent sinon, y prend la couleur du texte sélectionné (elle
/// disparaissait dans le fond, issue n° 59).
pub fn bar_icon_on(
    painter: &Painter,
    rect: Rect,
    action: Action,
    color: Color32,
    colors: &Colors,
    on: bool,
) {
    let (w, h) = (rect.width(), rect.height());
    let p = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.12).max(1.5), color);
    // Flèches moins hautes que leur case (retour du 18/10/2026 : elles dépassaient les
    // icônes voisines), même angle.
    let chevron = |tip: f32, back: f32| {
        painter.add(Shape::line(
            vec![p(back, 0.22), p(tip, 0.5), p(back, 0.78)],
            stroke,
        ));
    };
    match action {
        Action::NavPrev => chevron(0.32, 0.64),
        Action::NavNext => chevron(0.68, 0.36),
        Action::NavUp | Action::NavDown => {
            // Chevron vertical, même taille que ‹ ›.
            let (tip, back) = if action == Action::NavUp {
                (0.32, 0.64)
            } else {
                (0.68, 0.36)
            };
            painter.add(Shape::line(
                vec![p(0.22, back), p(0.5, tip), p(0.78, back)],
                stroke,
            ));
        }
        Action::StackEnterToggle => {
            // Entrer par la première photo : barre et chevron (« au début ») ; allumé
            // quand c'est le choix, éteint : la couverture.
            painter.line_segment([p(0.24, 0.22), p(0.24, 0.78)], stroke);
            chevron(0.5, 0.78);
        }
        Action::DevPrev | Action::DevNext => {
            // Développement précédent / suivant : deux chevrons, comme ‹ › doublés.
            // Symétriques (#240) : chaque chevron large de 0,26, la paire centrée.
            let thin = Stroke::new(stroke.width * 0.8, color);
            let (xs, tip): ([f32; 2], f32) = if action == Action::DevNext {
                ([0.22, 0.52], 0.26)
            } else {
                ([0.78, 0.48], -0.26)
            };
            for x in xs {
                painter.add(Shape::line(
                    vec![p(x, 0.22), p(x + tip, 0.5), p(x, 0.78)],
                    thin,
                ));
            }
        }
        Action::StacksAll => {
            // Piles dépliées : trois feuilles côte à côte.
            for x in [0.0, 0.36, 0.72] {
                let r = Rect::from_min_max(p(x, 0.3), p(x + 0.28, 0.7));
                painter.rect_stroke(r, 1.5, stroke, egui::StrokeKind::Middle);
            }
        }
        Action::StackMode => {
            // Pile : trois feuilles décalées.
            let thin = Stroke::new(stroke.width * 0.7, color);
            for (k, o) in [(0, 0.0), (1, 0.12), (2, 0.24)] {
                let r = Rect::from_min_max(p(0.1 + o, 0.36 - o), p(0.66 + o, 0.9 - o));
                if k == 0 {
                    painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Middle);
                } else {
                    painter.line_segment([r.left_top(), r.right_top()], thin);
                    painter.line_segment([r.right_top(), r.right_bottom()], thin);
                }
            }
        }
        Action::Develop => {
            // Développer : un petit diaphragme (cercle et lamelles en hélice).
            let c = rect.center();
            let r = rect.width().min(rect.height()) * 0.45;
            let stroke = Stroke::new((r * 0.16).max(1.2), color);
            painter.circle_stroke(c, r, stroke);
            for k in 0..6 {
                let phi = std::f32::consts::TAU * k as f32 / 6.0 + 0.3;
                let (n, t) = (vec2(phi.cos(), phi.sin()), vec2(-phi.sin(), phi.cos()));
                let base = c + n * r * 0.36;
                let length = (r * r - (r * 0.36).powi(2)).sqrt();
                painter.line_segment([base - t * r * 0.2, base + t * length], stroke);
            }
        }
        Action::ToggleZoom => {
            // « 1:1 » écrit : chiffres et deux-points sont dans toutes les polices.
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "1:1",
                egui::FontId::proportional(h * 0.8),
                color,
            );
        }
        // Retour à la grille (issue n° 193) : la grille de neuf cases.
        Action::Back => grid_fit_icon(painter, rect.shrink(w * 0.05), color),
        _ => {
            // Petite étoile à gauche, flèche à droite.
            let star_fill = if on {
                painter.ctx().global_style().visuals.selection.stroke.color
            } else {
                star_color(painter.ctx(), colors)
            };
            star(painter, p(0.26, 0.5), w * 0.32, star_fill);
            painter.line_segment([p(0.58, 0.5), p(1.0, 0.5)], stroke);
            painter.add(Shape::line(
                vec![p(0.78, 0.26), p(1.0, 0.5), p(0.78, 0.74)],
                stroke,
            ));
        }
    }
}

/// Avertissement : triangle plein et point d'exclamation évidé (vignette impossible à
/// calculer). Dessiné, pas de glyphe « ⚠ » : la police de l'interface ne l'a pas.
pub fn warning(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let h = s * 0.85;
    let w = h * 1.1;
    let top = c + vec2(0.0, -h * 0.5);
    let triangle = vec![top, c + vec2(w * 0.5, h * 0.5), c + vec2(-w * 0.5, h * 0.5)];
    painter.add(Shape::convex_polygon(triangle, color, Stroke::NONE));
    // Point d'exclamation, de la couleur du fond (évidé).
    let hole = Color32::from_black_alpha(200);
    let bar = Stroke::new(h * 0.11, hole);
    painter.line_segment([c + vec2(0.0, -h * 0.17), c + vec2(0.0, h * 0.18)], bar);
    painter.circle_filled(c + vec2(0.0, h * 0.33), h * 0.065, hole);
}

/// Icône de recadrage : deux équerres croisées (badge des versions dans la grille).
pub fn crop_icon(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let stroke = Stroke::new((s * 0.13).max(1.2), color);
    let (min, max) = (rect.min, rect.max);
    let k = s * 0.22;
    // Équerre du haut à gauche, qui dépasse en haut et à gauche.
    painter.add(Shape::line(
        vec![
            pos2(min.x + k, min.y),
            pos2(min.x + k, max.y - k),
            pos2(max.x, max.y - k),
        ],
        stroke,
    ));
    // Équerre du bas à droite.
    painter.add(Shape::line(
        vec![
            pos2(min.x, min.y + k),
            pos2(max.x - k, min.y + k),
            pos2(max.x - k, max.y),
        ],
        stroke,
    ));
}

/// Disquette (enregistrer) dans `rect` : contour au coin haut droit coupé, volet en
/// haut, étiquette en bas.
pub fn floppy(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let rect = Rect::from_center_size(rect.center(), vec2(s, s));
    let stroke = Stroke::new((s * 0.11).max(1.2), color);
    let (min, max) = (rect.min, rect.max);
    let cut = s * 0.22;
    painter.add(Shape::closed_line(
        vec![
            min,
            pos2(max.x - cut, min.y),
            pos2(max.x, min.y + cut),
            max,
            pos2(min.x, max.y),
        ],
        stroke,
    ));
    // Volet métallique, en haut.
    painter.rect_filled(
        Rect::from_min_max(
            pos2(min.x + s * 0.25, min.y),
            pos2(max.x - s * 0.32, min.y + s * 0.3),
        ),
        0.0,
        color,
    );
    // Étiquette, en bas.
    painter.rect_stroke(
        Rect::from_min_max(
            pos2(min.x + s * 0.2, min.y + s * 0.55),
            pos2(max.x - s * 0.2, max.y),
        ),
        0.0,
        stroke,
        egui::StrokeKind::Inside,
    );
}

/// Sens de l'ordre : flèche vers le bas (décroissant) ou vers le haut (croissant), avec
/// trois traits de longueur croissante à côté (comme les icônes de tri habituelles).
pub fn sort_arrow(painter: &Painter, rect: Rect, descending: bool, color: Color32) {
    let s = rect.width().min(rect.height());
    let rect = Rect::from_center_size(rect.center(), vec2(s, s));
    let stroke = Stroke::new((s * 0.1).max(1.2), color);
    let x = rect.left() + s * 0.25;
    let (top, bottom) = (rect.top() + s * 0.12, rect.bottom() - s * 0.12);
    painter.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
    let (tip, back) = if descending {
        (bottom, bottom - s * 0.22)
    } else {
        (top, top + s * 0.22)
    };
    painter.line_segment([pos2(x, tip), pos2(x - s * 0.18, back)], stroke);
    painter.line_segment([pos2(x, tip), pos2(x + s * 0.18, back)], stroke);
    // Traits : du plus court au plus long dans le sens de l'ordre.
    for i in 0..3 {
        let y = rect.top() + s * (0.25 + 0.25 * i as f32);
        let len = s * 0.18 * (if descending { 3 - i } else { i + 1 }) as f32;
        let left = rect.left() + s * 0.5;
        painter.line_segment([pos2(left, y), pos2(left + len, y)], stroke);
    }
}

/// Copier : deux feuilles décalées, celle de devant pleine du fond (`back`).
pub fn copy_icon(painter: &Painter, rect: Rect, color: Color32, back: Color32) {
    let s = rect.width().min(rect.height());
    let rect = Rect::from_center_size(rect.center(), vec2(s, s));
    let stroke = Stroke::new((s * 0.1).max(1.2), color);
    let sheet = vec2(s * 0.62, s * 0.72);
    let behind = Rect::from_min_size(rect.min, sheet);
    let front = Rect::from_min_size(rect.max - sheet, sheet);
    painter.rect_stroke(behind, s * 0.08, stroke, egui::StrokeKind::Inside);
    painter.rect_filled(front, s * 0.08, back);
    painter.rect_stroke(front, s * 0.08, stroke, egui::StrokeKind::Inside);
}

/// Couleur d'accent du thème (fond de sélection d'egui) : ce qui est allumé, choisi.
pub fn accent(ctx: &egui::Context) -> Color32 {
    ctx.global_style().visuals.selection.bg_fill
}

/// Accent pour un trait ou une petite forme sur le fond (étoiles, anneau d'une couleur
/// choisie) : en thème clair, le fond de sélection est trop pâle sur du blanc, on prend
/// sa variante foncée (le texte de la sélection d'egui).
pub fn accent_mark(ctx: &egui::Context) -> Color32 {
    let style = ctx.global_style();
    if style.visuals.dark_mode {
        style.visuals.selection.bg_fill
    } else {
        style.visuals.selection.stroke.color
    }
}

/// Couleur des étoiles sur la pastille sombre des vignettes (tous thèmes) :
/// `[gui.colors] stars`, sinon l'accent éclairci, le bleu foncé de l'accent ne s'y
/// lisait pas.
pub fn chip_star_color(ctx: &egui::Context, colors: &Colors) -> Color32 {
    colors.stars.map_or_else(
        || accent(ctx).lerp_to_gamma(Color32::WHITE, 0.4),
        Color32::from,
    )
}

/// Couleur des étoiles : `[gui.colors] stars`, sinon l'accent du thème.
pub fn star_color(ctx: &egui::Context, colors: &Colors) -> Color32 {
    colors.stars.map_or_else(|| accent_mark(ctx), Color32::from)
}

/// Balai (effacer une liste) dans `rect` : manche en diagonale, ligature, puis brins en
/// éventail vers le bas à gauche.
pub fn broom(painter: &Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let o = rect.center() - vec2(s, s) / 2.0;
    let p = |x: f32, y: f32| o + vec2(x * s, y * s);
    let stroke = Stroke::new((s * 0.11).max(1.2), color);
    painter.add(Shape::line_segment([p(0.95, 0.05), p(0.52, 0.48)], stroke));
    // Ligature, perpendiculaire au manche.
    painter.add(Shape::line_segment([p(0.36, 0.46), p(0.54, 0.64)], stroke));
    // Brins : de la ligature vers une base plus large.
    let thin = Stroke::new((s * 0.08).max(1.0), color);
    for i in 0..5 {
        let t = i as f32 / 4.0;
        let from = p(0.38 + 0.14 * t, 0.48 + 0.14 * t);
        let to = p(0.02 + 0.42 * t, 0.56 + 0.42 * t);
        painter.add(Shape::line_segment([from, to], thin));
    }
}

/// Flèche circulaire (actualiser) dans `rect` : arc de cercle ouvert en haut à droite,
/// pointe de flèche au bout.
pub fn refresh(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.45;
    let stroke = Stroke::new((r * 0.24).max(1.5), color);
    // De -60° à 240° (sens horaire à l'écran : y vers le bas).
    let (start, end) = (-PI / 3.0, 4.0 * PI / 3.0);
    let steps = 28;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = start + (end - start) * i as f32 / steps as f32;
            c + vec2(t.cos(), t.sin()) * r
        })
        .collect();
    let tip = points[0];
    painter.add(Shape::line(points, stroke));
    // Pointe : tangente au début de l'arc, vers le haut.
    let dir = vec2(start.sin(), -start.cos());
    let side = vec2(-dir.y, dir.x);
    let h = r * 0.8;
    let head = vec![
        tip + dir * h * 0.35,
        tip - dir * h * 0.65 + side * h * 0.7,
        tip - dir * h * 0.65 - side * h * 0.7,
    ];
    painter.add(Shape::convex_polygon(head, color, Stroke::NONE));
}

/// Sous-dossiers dans `rect` : un dossier en haut, relié par une arborescence à trois
/// dossiers plus petits en dessous.
pub fn subfolders(painter: &Painter, rect: Rect, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let p = |x: f32, y: f32| pos2(rect.left() + x * w, rect.top() + y * h);
    let stroke = Stroke::new((w * 0.07).max(1.0), color);
    // Un dossier plein (onglet et corps), coin haut gauche (x, y), largeur `s` et hauteur
    // `t` (en fraction de `rect`).
    let folder = |x: f32, y: f32, s: f32, t: f32| {
        painter.add(Shape::convex_polygon(
            vec![
                p(x, y),
                p(x + s * 0.4, y),
                p(x + s * 0.5, y + t * 0.2),
                p(x + s, y + t * 0.2),
                p(x + s, y + t),
                p(x, y + t),
            ],
            color,
            Stroke::NONE,
        ));
    };
    folder(0.25, 0.0, 0.5, 0.38);
    for x in [0.0, 0.36, 0.72] {
        folder(x, 0.74, 0.28, 0.26);
    }
    // Tige sous le parent, barre, et une descente vers chaque petit dossier.
    painter.line_segment([p(0.5, 0.38), p(0.5, 0.74)], stroke);
    painter.add(Shape::line(
        vec![p(0.14, 0.74), p(0.14, 0.56), p(0.86, 0.56), p(0.86, 0.74)],
        stroke,
    ));
}

/// Serveur réseau dans `rect` : deux tiroirs d'un boîtier, chacun avec son voyant.
pub fn server(painter: &Painter, rect: Rect, color: Color32) {
    let (w, h) = (rect.width(), rect.height());
    let stroke = Stroke::new((w * 0.08).max(1.2), color);
    for top in [0.08, 0.54] {
        let drawer = Rect::from_min_size(
            pos2(rect.left() + w * 0.05, rect.top() + h * top),
            vec2(w * 0.9, h * 0.38),
        );
        painter.rect_stroke(drawer, 2.0, stroke, egui::StrokeKind::Inside);
        painter.circle_filled(
            pos2(drawer.right() - w * 0.2, drawer.center().y),
            w * 0.07,
            color,
        );
    }
}

/// Triangle d'une branche d'arborescence, dans `rect` : vers le bas si `open`, sinon vers
/// la droite (dessiné : « ▶ » manque dans la police d'egui).
pub fn disclosure(painter: &Painter, rect: Rect, open: bool, color: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.25;
    let points = if open {
        vec![
            c + vec2(-r, -r * 0.6),
            c + vec2(r, -r * 0.6),
            c + vec2(0.0, r * 0.7),
        ]
    } else {
        vec![
            c + vec2(-r * 0.6, -r),
            c + vec2(r * 0.7, 0.0),
            c + vec2(-r * 0.6, r),
        ]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Icône « replier » d'une pile de développements dépliée (31/10/2026) : un chevron vers
/// le haut au-dessus d'un trait, les vignettes qui remontent sous la couverture ; le
/// dessin seul, dans la couleur du texte (retour du même soir).
pub fn fold(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.28;
    // Trait fin, comme la bobine (retour du 31/10/2026 : trop gras).
    let stroke = Stroke::new((rect.width() * 0.06).max(1.0), color);
    painter.line_segment([c + vec2(-r, -r * 0.15), c + vec2(0.0, -r * 0.85)], stroke);
    painter.line_segment([c + vec2(0.0, -r * 0.85), c + vec2(r, -r * 0.15)], stroke);
    painter.line_segment([c + vec2(-r, r * 0.6), c + vec2(r, r * 0.6)], stroke);
}

/// Côté d'un panneau, pour [`panel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Gauche (dossiers).
    Left,
    /// Droite (métadonnées).
    Right,
    /// Haut (barre de filtres).
    Top,
    /// Bas (bandeau de notation).
    Bottom,
}

/// Icône d'un panneau dans `rect` : une fenêtre (cadre) et la bande du panneau, pleine s'il
/// est affiché, en contour sinon.
pub fn panel(painter: &Painter, rect: Rect, side: Side, shown: bool, color: Color32) {
    let frame = rect.shrink(rect.width() * 0.18);
    let stroke = Stroke::new(1.2, color);
    painter.rect_stroke(frame, 1.5, stroke, egui::StrokeKind::Inside);
    let band = match side {
        Side::Left => Rect::from_min_max(
            frame.min,
            pos2(frame.min.x + frame.width() * 0.32, frame.max.y),
        ),
        Side::Right => Rect::from_min_max(
            pos2(frame.max.x - frame.width() * 0.32, frame.min.y),
            frame.max,
        ),
        Side::Top => Rect::from_min_max(
            frame.min,
            pos2(frame.max.x, frame.min.y + frame.height() * 0.3),
        ),
        Side::Bottom => Rect::from_min_max(
            pos2(frame.min.x, frame.max.y - frame.height() * 0.3),
            frame.max,
        ),
    };
    if shown {
        painter.rect_filled(band, 1.0, color);
    } else {
        painter.rect_stroke(band, 1.0, stroke, egui::StrokeKind::Inside);
    }
}

/// Icône « décharger » dans `rect` : flèche vers le bas au-dessus d'un bac.
pub fn offload(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.3;
    let stroke = Stroke::new(1.6, color);
    painter.line_segment([c + vec2(0.0, -r), c + vec2(0.0, r * 0.35)], stroke);
    painter.add(Shape::convex_polygon(
        vec![
            c + vec2(-r * 0.45, r * 0.05),
            c + vec2(r * 0.45, r * 0.05),
            c + vec2(0.0, r * 0.55),
        ],
        color,
        Stroke::NONE,
    ));
    painter.line(
        vec![
            c + vec2(-r, r * 0.35),
            c + vec2(-r, r * 0.8),
            c + vec2(r, r * 0.8),
            c + vec2(r, r * 0.35),
        ],
        stroke,
    );
}

/// Icône « éjecter » dans `rect` : triangle pointe en haut sur une barre.
pub fn eject(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.3;
    painter.add(Shape::convex_polygon(
        vec![
            c + vec2(0.0, -r),
            c + vec2(r, r * 0.2),
            c + vec2(-r, r * 0.2),
        ],
        color,
        Stroke::NONE,
    ));
    painter.rect_filled(
        Rect::from_min_max(c + vec2(-r, r * 0.45), c + vec2(r, r * 0.75)),
        0.0,
        color,
    );
}

/// Contrôles de notation cliquables (bandeau de notation, menu du clic droit) : drapeaux,
/// étoiles, couleurs, actifs en couleur, les autres estompés. Renvoie l'action cliquée,
/// la même que celle du clavier, sauf l'étoile posée, qui recliquée remet à zéro (le
/// drapeau recliqué reste) ; une couleur s'ajoute ou se retire. Bulle de chaque contrôle :
/// `tip` (libellé et raccourcis, demandé le 22/10/2026).
pub fn controls(
    ui: &mut Ui,
    notes: &Notes,
    colors: &Colors,
    size: f32,
    tip: &dyn Fn(Action) -> String,
) -> Option<Action> {
    let mut clicked = flag_controls(ui, notes, colors, size, tip);
    ui.add_space(size * 0.5);
    clicked = star_controls(ui, notes, colors, size, tip).or(clicked);
    ui.add_space(size * 0.5);
    color_controls(ui, notes, colors, size, tip).or(clicked)
}

/// Un contrôle de notation : son carré et vrai s'il est cliqué.
fn control_cell(
    ui: &mut Ui,
    size: f32,
    action: Action,
    tip: &dyn Fn(Action) -> String,
) -> (Rect, bool) {
    let (rect, response) = clickable_named(ui, size, &action.label(), &tip(action));
    (rect, response.clicked())
}

/// Les trois drapeaux ([`controls`], et le sélecteur d'une vignette : #257).
pub fn flag_controls(
    ui: &mut Ui,
    notes: &Notes,
    colors: &Colors,
    size: f32,
    tip: &dyn Fn(Action) -> String,
) -> Option<Action> {
    let strong = ui.visuals().strong_text_color();
    let mut clicked = None;
    let cell = |ui: &mut Ui, action: Action| control_cell(ui, size, action, tip);
    // Drapeau : trois contrôles, comme les touches Maj+P / Maj+X / Maj+U. Recliquer le
    // drapeau posé ne le retire pas (retour du 17/10/2026 : un double clic involontaire
    // effaçait une note).
    for (flag, color) in [
        (Flag::Accepted, colors.accepted),
        (Flag::Rejected, colors.rejected),
    ] {
        let action = Action::Flag(flag);
        let (rect, hit) = cell(ui, action);
        // Non choisi : la même couleur, fondue, comme les pastilles de couleur.
        let (fill, glyph) = if notes.flag == flag {
            (Color32::from(color), Color32::WHITE)
        } else {
            (
                Color32::from(color).gamma_multiply(FADED),
                Color32::WHITE.gamma_multiply(FADED),
            )
        };
        badge_with_glyph(
            ui.painter(),
            rect.center(),
            size * 0.45,
            flag == Flag::Accepted,
            fill,
            glyph,
        );
        if hit {
            clicked = Some(action);
        }
    }
    let action = Action::Flag(Flag::None);
    let (rect, hit) = cell(ui, action);
    let ring = if notes.flag == Flag::None {
        strong
    } else {
        strong.gamma_multiply(FADED)
    };
    // De la taille des deux pastilles (retour du 31/10/2026 : il paraissait plus grand).
    neutral_ring(ui.painter(), rect.center(), size * 0.45, ring);
    if hit {
        clicked = Some(action);
    }
    clicked
}

/// Les cinq étoiles ([`controls`], et le sélecteur d'une vignette : #257).
pub fn star_controls(
    ui: &mut Ui,
    notes: &Notes,
    colors: &Colors,
    size: f32,
    tip: &dyn Fn(Action) -> String,
) -> Option<Action> {
    let weak = ui.visuals().weak_text_color();
    let mut clicked = None;
    let cell = |ui: &mut Ui, action: Action| control_cell(ui, size, action, tip);
    // Étoiles : recliquer l'étoile posée remet à zéro, comme le filtre des étoiles
    // (retour du 21/10/2026 : à la souris, pas de double clic involontaire à craindre ;
    // le contrôle « ⊘ » est retiré). Au clavier, la touche 0 seule remet à zéro.
    for n in 1..=5u8 {
        let action = Action::Stars(n);
        let (rect, hit) = cell(ui, action);
        let action = if n == notes.stars {
            Action::Stars(0)
        } else {
            action
        };
        let fill = if n <= notes.stars {
            star_color(ui.ctx(), colors)
        } else {
            weak
        };
        star(ui.painter(), rect.center(), size / 2.0, fill);
        if hit {
            clicked = Some(action);
        }
    }
    clicked
}

/// Les cinq couleurs ([`controls`], et le sélecteur d'une vignette : #257).
pub fn color_controls(
    ui: &mut Ui,
    notes: &Notes,
    colors: &Colors,
    size: f32,
    tip: &dyn Fn(Action) -> String,
) -> Option<Action> {
    let mut clicked = None;
    let cell = |ui: &mut Ui, action: Action| control_cell(ui, size, action, tip);
    for color in Color::ALL {
        let action = Action::Color(color);
        let (rect, hit) = cell(ui, action);
        // Posée : pleine ; sinon plus petite et fondue. Plus de cercle d'accent autour de
        // la couleur posée (retour du 31/10/2026 : les étoiles et les drapeaux n'en ont
        // pas, le fondu suffit).
        color_dot(
            ui.painter(),
            rect.center(),
            size,
            color_of(colors, color),
            notes.has_color(color),
        );
        if hit {
            clicked = Some(action);
        }
    }
    clicked
}

/// Icône « disque externe » dans `rect` : boîtier arrondi et voyant.
pub fn drive(painter: &Painter, rect: Rect, color: Color32) {
    let body = Rect::from_center_size(
        rect.center(),
        vec2(rect.width() * 0.7, rect.height() * 0.42),
    );
    painter.rect_stroke(body, 2.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
    painter.circle_filled(
        pos2(body.max.x - body.height() * 0.4, body.center().y),
        body.height() * 0.15,
        color,
    );
}

/// Repère du collimateur de mise au point, centré en `center`, de côté `size`.
pub fn focus_marker(
    painter: &Painter,
    center: Pos2,
    size: f32,
    shape: FocusMarker,
    stroke: Stroke,
) {
    let half = size / 2.0;
    match shape {
        FocusMarker::Square => {
            painter.rect_stroke(
                Rect::from_center_size(center, vec2(size, size)),
                0.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        FocusMarker::Circle => {
            painter.circle_stroke(center, half, stroke);
        }
        FocusMarker::Cross => {
            painter.line_segment([center - vec2(half, 0.0), center + vec2(half, 0.0)], stroke);
            painter.line_segment([center - vec2(0.0, half), center + vec2(0.0, half)], stroke);
        }
        FocusMarker::Heart => {
            // Courbe classique du cœur : x = 16 sin³ t, y = 13 cos t − 5 cos 2t − 2 cos 3t −
            // cos 4t (y de −17 à 12, x de −16 à 16), ramenée à `size`.
            let points = (0..64)
                .map(|i| {
                    let t = i as f32 / 64.0 * 2.0 * PI;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos()
                        - 5.0 * (2.0 * t).cos()
                        - 2.0 * (3.0 * t).cos()
                        - (4.0 * t).cos();
                    center + vec2(x, -y - 2.5) * (size / 32.0)
                })
                .collect();
            painter.add(Shape::closed_line(points, stroke));
        }
    }
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
