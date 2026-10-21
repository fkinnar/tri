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

/// Icône d'un filtre sur le drapeau, dans `rect` : coche verte (acceptées), croix rouge
/// (rejetées seulement), la même croix en gris (masquer les rejetées).
pub fn flag_filter(
    painter: &Painter,
    rect: Rect,
    choice: crate::actions::FlagFilter,
    colors: &Colors,
) {
    use crate::actions::FlagFilter;
    let (center, radius) = (rect.center(), rect.width() * 0.4);
    match choice {
        FlagFilter::Accepted => flag(painter, center, radius, Flag::Accepted, colors),
        FlagFilter::Rejected => flag(painter, center, radius, Flag::Rejected, colors),
        FlagFilter::NotRejected => badge(painter, center, radius, false, Color32::from_gray(120)),
    }
}

/// Goutte d'eau (le flou) dans `rect` ; `barred` : barrée (masquer les floues).
pub fn blur_drop(painter: &Painter, rect: Rect, color: Color32, barred: bool) {
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
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    if barred {
        let s = rect.width() * 0.42;
        let o = rect.center();
        let back = Stroke::new(rect.width() * 0.2, Color32::from_black_alpha(200));
        let line = Stroke::new(rect.width() * 0.1, Color32::from_gray(235));
        for stroke in [back, line] {
            painter.line_segment([o + vec2(-s, s), o + vec2(s, -s)], stroke);
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
            star_color(painter.ctx(), colors),
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

/// Bouton carré de côté `size`, au fond de bouton du thème (survol, appui) : nom `label`
/// pour l'accessibilité et les tests, bulle `tip`. L'icône est dessinée ensuite par
/// l'appelant dans le rectangle renvoyé.
pub fn icon_button(ui: &mut Ui, size: f32, label: &str, tip: &str) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    let response = response
        .on_hover_text(tip)
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
/// étoile suivie d'une flèche (avance automatique : « après une note, on avance »).
pub fn bar_icon(painter: &Painter, rect: Rect, action: Action, color: Color32, colors: &Colors) {
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
        _ => {
            // Petite étoile à gauche, flèche à droite.
            star(
                painter,
                p(0.26, 0.5),
                w * 0.32,
                star_color(painter.ctx(), colors),
            );
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

/// Couleur des étoiles : `[gui.colors] stars`, sinon l'accent du thème.
pub fn star_color(ctx: &egui::Context, colors: &Colors) -> Color32 {
    colors.stars.map_or_else(|| accent_mark(ctx), Color32::from)
}

/// Étiquette de type de fichier dans `rect` : un rectangle arrondi plein où `label`
/// (« RAW », « JPG ») s'écrit en creux, couleur `back` (filtre par type ; retour du
/// 21/10/2026 : sans l'icône de page, à la taille des autres icônes).
pub fn file_type(painter: &Painter, rect: Rect, label: &str, color: Color32, back: Color32) {
    let h = rect.height();
    let band = Rect::from_center_size(rect.center(), vec2(rect.width(), h * 0.62));
    painter.rect_filled(band, h * 0.14, color);
    painter.text(
        band.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(band.height() * 0.78),
        back,
    );
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
/// la même que celle du clavier : recliquer la note active la retire (drapeau, étoiles),
/// une couleur s'ajoute ou se retire.
pub fn controls(ui: &mut Ui, notes: &Notes, colors: &Colors, size: f32) -> Option<Action> {
    let weak = ui.visuals().weak_text_color();
    let strong = ui.visuals().strong_text_color();
    let mut clicked = None;
    let cell = |ui: &mut Ui, tip: &str| {
        let (rect, response) = clickable(ui, size, tip);
        (rect, response.clicked())
    };
    // Drapeau : trois contrôles, comme les touches Maj+P / Maj+X / Maj+U. Recliquer le
    // drapeau posé ne le retire pas (retour du 17/10/2026 : un double clic involontaire
    // effaçait une note).
    for (flag, color) in [
        (Flag::Accepted, colors.accepted),
        (Flag::Rejected, colors.rejected),
    ] {
        let action = Action::Flag(flag);
        let (rect, hit) = cell(ui, &action.label());
        let fill = if notes.flag == flag {
            Color32::from(color)
        } else {
            weak
        };
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
    let action = Action::Flag(Flag::None);
    let (rect, hit) = cell(ui, &action.label());
    let ring = if notes.flag == Flag::None {
        strong
    } else {
        weak
    };
    ui.painter()
        .circle_stroke(rect.center(), size * 0.38, Stroke::new(1.5, ring));
    if hit {
        clicked = Some(action);
    }
    ui.add_space(size * 0.5);
    // Étoiles : un contrôle « 0 » à part, comme la touche 0 (Lightroom, Capture One) ;
    // recliquer l'étoile posée ne remet plus à zéro (même retour).
    let action = Action::Stars(0);
    let (rect, hit) = cell(ui, &action.label());
    let color = if notes.stars == 0 { strong } else { weak };
    none_sign(ui.painter(), rect, color);
    if hit {
        clicked = Some(action);
    }
    for n in 1..=5u8 {
        let action = Action::Stars(n);
        let (rect, hit) = cell(ui, &action.label());
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
    ui.add_space(size * 0.5);
    for color in Color::ALL {
        let action = Action::Color(color);
        let (rect, hit) = cell(ui, &action.label());
        let active = notes.has_color(color);
        let fill = color_of(colors, color);
        let painter = ui.painter();
        if active {
            painter.circle_filled(rect.center(), size * 0.4, fill);
            painter.circle_stroke(
                rect.center(),
                size * 0.48,
                Stroke::new(2.0, accent_mark(ui.ctx())),
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

/// Signe « aucun » (cercle barré) dans `rect`.
fn none_sign(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width() * 0.32;
    let stroke = Stroke::new(1.5, color);
    painter.circle_stroke(c, r, stroke);
    let d = r * std::f32::consts::FRAC_1_SQRT_2;
    painter.line_segment([c + vec2(-d, d), c + vec2(d, -d)], stroke);
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
