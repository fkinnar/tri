//! Dessin d'une palette de couleurs (#492) dans le panneau de droite : la bande des
//! couleurs (chacune aussi large que sa part) et la roue chromatique (teinte en angle,
//! chroma en rayon, taille selon la part). Le calcul est dans le moteur
//! ([`tri::palette`]) ; ici, le dessin seul, et la géométrie testable sans rien dessiner.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};
use tri::palette::{Palette, Swatch};

/// Couleur d'une pastille à l'écran : son sRGB (celui qu'on écrit en `#rrggbb`).
pub fn color(swatch: &Swatch) -> Color32 {
    let [r, g, b] = swatch.srgb();
    Color32::from_rgb(r, g, b)
}

/// `#rrggbb` d'une pastille.
pub fn hex(swatch: &Swatch) -> String {
    let [r, g, b] = swatch.srgb();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Rectangles des pastilles de la bande `rect`, de gauche à droite, chacune aussi large
/// que sa part.
pub fn band_rects(palette: &Palette, rect: Rect) -> Vec<Rect> {
    let mut x = rect.left();
    palette
        .swatches
        .iter()
        .map(|s| {
            let w = rect.width() * s.share;
            let r = Rect::from_x_y_ranges(x..=x + w, rect.y_range());
            x += w;
            r
        })
        .collect()
}

/// Chroma au bord de la roue : au-delà, les couleurs y sont posées (un sRGB très saturé
/// atteint ~0,32 en OKLab).
const MAX_CHROMA: f32 = 0.3;

/// Place d'une pastille sur la roue de centre `centre` et de rayon `radius` : la teinte en
/// angle (rouge à droite, sens trigonométrique comme a / b d'OKLab), le chroma en distance
/// au centre (un gris au centre), en racine : les couleurs d'une photo, peu saturées,
/// ne se tassent pas au centre.
pub fn wheel_pos(swatch: &Swatch, centre: Pos2, radius: f32) -> Pos2 {
    let (hue, chroma) = swatch.hue_chroma();
    let r = radius * (chroma / MAX_CHROMA).min(1.0).sqrt();
    let a = hue.to_radians();
    pos2(centre.x + r * a.cos(), centre.y - r * a.sin())
}

/// Couleur au point `pos` de la roue du **sélecteur** de centre `centre` et de rayon
/// `radius`, à la clarté `lightness` : la teinte en angle, la saturation en distance au
/// centre, **relative** à la plus forte affichable pour cette teinte et cette clarté (le
/// bord est toujours la couleur la plus saturée possible : le point reste sous le
/// pointeur, retour du 21/11/2026) ; `None` hors de la roue.
pub fn wheel_colour(pos: Pos2, centre: Pos2, radius: f32, lightness: f32) -> Option<[f32; 3]> {
    let d = pos - centre;
    let r = d.length() / radius.max(1.0);
    if r > 1.0 {
        return None;
    }
    let hue = (-d.y).atan2(d.x);
    let chroma = r * tri::palette::max_chroma(lightness, hue);
    Some([lightness, chroma * hue.cos(), chroma * hue.sin()])
}

/// Place de `oklab` sur la roue du sélecteur (l'inverse de [`wheel_colour`]).
pub fn picker_pos(oklab: [f32; 3], centre: Pos2, radius: f32) -> Pos2 {
    let [l, a, b] = oklab;
    let hue = b.atan2(a);
    let max = tri::palette::max_chroma(l, hue).max(1e-6);
    let r = radius * (a.hypot(b) / max).min(1.0);
    pos2(centre.x + r * hue.cos(), centre.y - r * hue.sin())
}

/// Roue du sélecteur de couleur : les teintes et saturations à la clarté `lightness`,
/// en dégradé (gris au centre, au bord la couleur la plus saturée affichable de chaque
/// teinte).
pub fn draw_picker_wheel(painter: &Painter, rect: Rect, lightness: f32) {
    let centre = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    let (rings, sectors) = (14usize, 72usize);
    let max: Vec<f32> = (0..=sectors)
        .map(|k| {
            let a = k as f32 / sectors as f32 * std::f32::consts::TAU;
            tri::palette::max_chroma(lightness, a)
        })
        .collect();
    let mut mesh = egui::Mesh::default();
    for ring in 0..=rings {
        let f = ring as f32 / rings as f32;
        let r = radius * f;
        for (sector, &m) in max.iter().enumerate() {
            let a = sector as f32 / sectors as f32 * std::f32::consts::TAU;
            let pos = pos2(centre.x + r * a.cos(), centre.y - r * a.sin());
            // De l'angle et du rayon directement (par la position, les sommets du bord
            // tombaient parfois juste dehors, en gris : des pointillés).
            let chroma = f * m;
            let oklab = [lightness, chroma * a.cos(), chroma * a.sin()];
            mesh.colored_vertex(pos, color(&Swatch { oklab, share: 0.0 }));
        }
    }
    let row = (sectors + 1) as u32;
    for ring in 0..rings as u32 {
        for sector in 0..sectors as u32 {
            let i = ring * row + sector;
            mesh.add_triangle(i, i + 1, i + row);
            mesh.add_triangle(i + 1, i + row + 1, i + row);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    // Le bord adouci : un trait fin sur les facettes du maillage.
    painter.circle_stroke(
        centre,
        radius,
        Stroke::new(1.5, Color32::from_black_alpha(90)),
    );
}

/// Dessine la roue : un cercle de teintes en bordure, et chaque pastille, d'autant plus
/// grande que sa part.
pub fn draw_wheel(painter: &Painter, rect: Rect, palette: &Palette, stroke: Color32) {
    let centre = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 6.0;
    // Bordure en teintes : 36 arcs, à chroma moyen et clarté moyenne.
    let steps = 36;
    for k in 0..steps {
        let a0 = k as f32 / steps as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / steps as f32 * std::f32::consts::TAU;
        let mid = (a0 + a1) / 2.0;
        let hue = Swatch {
            oklab: [0.7, 0.15 * mid.cos(), 0.15 * mid.sin()],
            share: 0.0,
        };
        let p = |a: f32| pos2(centre.x + radius * a.cos(), centre.y - radius * a.sin());
        painter.line_segment([p(a0), p(a1)], Stroke::new(3.0, color(&hue)));
    }
    painter.circle_stroke(centre, radius * 0.5, Stroke::new(1.0, stroke));
    painter.circle_filled(centre, 1.5, stroke);
    for s in &palette.swatches {
        let at = wheel_pos(s, centre, radius);
        let size = 3.0 + 9.0 * s.share.sqrt();
        painter.circle_filled(at, size, color(s));
        painter.circle_stroke(at, size, Stroke::new(1.0, stroke));
    }
}

/// Hauteur de la bande des couleurs (points).
pub const BAND_HEIGHT: f32 = 22.0;

/// Dessine la bande ; renvoie les rectangles des pastilles (survol, clic).
pub fn draw_band(painter: &Painter, rect: Rect, palette: &Palette) -> Vec<Rect> {
    let rects = band_rects(palette, rect);
    for (r, s) in rects.iter().zip(&palette.swatches) {
        painter.rect_filled(*r, 0.0, color(s));
    }
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_black_alpha(80)),
        egui::StrokeKind::Inside,
    );
    rects
}

/// Taille de la roue dans une largeur `width` (points).
pub fn wheel_size(width: f32) -> egui::Vec2 {
    let side = (width * 0.45).clamp(80.0, 140.0);
    vec2(side, side)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tri::palette::srgb_to_oklab;

    fn swatch(rgb: [u8; 3], share: f32) -> Swatch {
        Swatch {
            oklab: srgb_to_oklab(rgb),
            share,
        }
    }

    #[test]
    fn the_band_splits_by_share() {
        let p = Palette {
            swatches: vec![swatch([200, 0, 0], 0.75), swatch([0, 0, 200], 0.25)],
        };
        let rects = band_rects(&p, Rect::from_min_size(Pos2::ZERO, vec2(200.0, 20.0)));
        assert_eq!(rects[0].width(), 150.0);
        assert_eq!(rects[1].left(), 150.0);
        assert_eq!(rects[1].right(), 200.0);
    }

    #[test]
    fn the_picked_point_stays_under_the_pointer_even_at_the_edge() {
        // Retour du 21/11/2026 : le point de la roue du sélecteur ne suivait pas le
        // pointeur (ramené dans le gamut, il reculait vers le centre).
        let (c, r) = (pos2(100.0, 100.0), 50.0);
        for at in [
            pos2(140.0, 80.0),
            pos2(100.0, 51.0),
            pos2(62.0, 120.0),
            pos2(149.5, 100.0),
        ] {
            for l in [0.3, 0.6, 0.9] {
                let oklab = wheel_colour(at, c, r, l).unwrap();
                let back = picker_pos(oklab, c, r);
                assert!(
                    back.distance(at) < 0.5,
                    "{at:?} à L {l} : revenu en {back:?}"
                );
                let srgb = Swatch { oklab, share: 0.0 }.srgb();
                let again = tri::palette::srgb_to_oklab(srgb);
                assert!(
                    tri::palette::oklab_distance(again, oklab) < 0.02,
                    "affichable"
                );
            }
        }
        assert!(
            wheel_colour(pos2(200.0, 100.0), c, r, 0.5).is_none(),
            "hors de la roue"
        );
    }

    #[test]
    fn grey_sits_in_the_centre_and_red_to_the_right() {
        let c = pos2(100.0, 100.0);
        let grey = wheel_pos(&swatch([128, 128, 128], 1.0), c, 50.0);
        assert!((grey - c).length() < 0.5);
        let red = wheel_pos(&swatch([220, 30, 30], 1.0), c, 50.0);
        assert!(red.x > c.x + 20.0, "{red:?}");
        assert_eq!(hex(&swatch([18, 52, 86], 1.0)), "#123456");
    }
}
