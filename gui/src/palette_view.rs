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

/// Teinte (degrés), saturation et valeur (0 à 1) vers sRGB 8 bits.
pub fn hsv_to_rgb([h, s, v]: [f32; 3]) -> [u8; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|u| ((u + m).clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// sRGB 8 bits vers teinte (degrés), saturation et valeur ; un gris garde la teinte
/// `hue_if_grey` (sa teinte n'existe pas : la roue ne saute pas à zéro).
pub fn rgb_to_hsv(rgb: [u8; 3], hue_if_grey: f32) -> [f32; 3] {
    let [r, g, b] = rgb.map(|u| f32::from(u) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= f32::EPSILON {
        hue_if_grey
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max <= f32::EPSILON { 0.0 } else { d / max };
    [h, s, max]
}

/// Roue des peintres (rouge, jaune, bleu ; #492, 23/11/2026, comme Adobe Color) : la
/// teinte TSV (sRGB) de chaque angle de la roue RYB, par paliers ; entre deux, linéaire.
/// Le complémentaire du rouge y est le vert, celui du bleu l'orange, celui du jaune le
/// violet : les harmonies « qu'on connaît ».
const RYB_TO_RGB: [(f32, f32); 7] = [
    (0.0, 0.0),
    (60.0, 35.0),
    (120.0, 60.0),
    (180.0, 120.0),
    (240.0, 240.0),
    (300.0, 280.0),
    (360.0, 360.0),
];

/// Interpolation linéaire par paliers de `x` dans `table` (colonne `from` vers `to`).
fn piecewise(x: f32, from: usize, to: usize) -> f32 {
    let x = x.rem_euclid(360.0);
    let at = |i: usize, k: usize| {
        if k == 0 {
            RYB_TO_RGB[i].0
        } else {
            RYB_TO_RGB[i].1
        }
    };
    for i in 0..RYB_TO_RGB.len() - 1 {
        let (a, b) = (at(i, from), at(i + 1, from));
        if x >= a && x <= b {
            let t = if b > a { (x - a) / (b - a) } else { 0.0 };
            return (at(i, to) + t * (at(i + 1, to) - at(i, to))).rem_euclid(360.0);
        }
    }
    x
}

/// Teinte TSV (sRGB, degrés) de l'angle `angle` de la roue : RYB ou la roue TSV même.
pub fn wheel_to_hue(angle: f32, ryb: bool) -> f32 {
    if ryb {
        piecewise(angle, 0, 1)
    } else {
        angle.rem_euclid(360.0)
    }
}

/// Angle sur la roue (RYB ou TSV) de la teinte TSV `hue` (degrés).
pub fn hue_to_wheel(hue: f32, ryb: bool) -> f32 {
    if ryb {
        piecewise(hue, 1, 0)
    } else {
        hue.rem_euclid(360.0)
    }
}

/// Teinte TSV et saturation au point `pos` de la roue de centre `centre` et de rayon
/// `radius` (rouge à droite, sens trigonométrique, gris au centre ; roue RYB si `ryb`) ;
/// `None` hors de la roue.
pub fn hsv_at(pos: Pos2, centre: Pos2, radius: f32, ryb: bool) -> Option<(f32, f32)> {
    let d = pos - centre;
    let s = d.length() / radius.max(1.0);
    let angle = (-d.y).atan2(d.x).to_degrees().rem_euclid(360.0);
    (s <= 1.0).then(|| (wheel_to_hue(angle, ryb), s))
}

/// Place d'une teinte TSV (degrés) et d'une saturation sur la roue (RYB si `ryb`).
pub fn hsv_pos(hue: f32, saturation: f32, centre: Pos2, radius: f32, ryb: bool) -> Pos2 {
    let a = hue_to_wheel(hue, ryb).to_radians();
    let r = radius * saturation.clamp(0.0, 1.0);
    pos2(centre.x + r * a.cos(), centre.y - r * a.sin())
}

/// Harmonie de couleurs (#492, 23/11/2026, comme Adobe Color) : des teintes placées sur
/// la roue à partir d'une couleur de base, qui tournent avec elle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harmony {
    /// En face.
    Complementary,
    /// Les deux voisines de l'opposée.
    SplitComplementary,
    /// Les voisines.
    Analogous,
    /// Trois à 120°.
    Triad,
    /// Deux paires opposées, en rectangle.
    Rectangle,
    /// Quatre à 90°.
    Square,
    /// Même teinte, d'autres saturations et luminosités.
    Shades,
}

impl Harmony {
    /// Toutes, dans l'ordre des boutons.
    pub const ALL: [Harmony; 7] = [
        Harmony::Complementary,
        Harmony::SplitComplementary,
        Harmony::Analogous,
        Harmony::Triad,
        Harmony::Rectangle,
        Harmony::Square,
        Harmony::Shades,
    ];

    /// Identifiant (chaînes `palette-harmony-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Harmony::Complementary => "complementary",
            Harmony::SplitComplementary => "split",
            Harmony::Analogous => "analogous",
            Harmony::Triad => "triad",
            Harmony::Rectangle => "rectangle",
            Harmony::Square => "square",
            Harmony::Shades => "shades",
        }
    }

    /// Écarts d'angle sur la roue depuis la base (degrés), la base d'abord.
    pub fn offsets(self) -> &'static [f32] {
        match self {
            Harmony::Complementary => &[0.0, 180.0],
            Harmony::SplitComplementary => &[0.0, 150.0, 210.0],
            Harmony::Analogous => &[0.0, -30.0, 30.0],
            Harmony::Triad => &[0.0, 120.0, 240.0],
            Harmony::Rectangle => &[0.0, 60.0, 180.0, 240.0],
            Harmony::Square => &[0.0, 90.0, 180.0, 270.0],
            Harmony::Shades => &[0.0, 0.0, 0.0],
        }
    }

    /// Les couleurs (TSV) de l'harmonie autour de `base`, la base d'abord ; les angles sur
    /// la roue (RYB si `ryb`), saturation et valeur de la base (camaïeu : la même teinte,
    /// plus sombre puis moins saturée).
    pub fn colours(self, base: [f32; 3], ryb: bool) -> Vec<[f32; 3]> {
        let [h, s, v] = base;
        if self == Harmony::Shades {
            return vec![[h, s, v], [h, s, v * 0.6], [h, s * 0.5, v]];
        }
        let angle = hue_to_wheel(h, ryb);
        // La base telle quelle (pas d'aller-retour par la roue).
        std::iter::once(base)
            .chain(
                self.offsets()[1..]
                    .iter()
                    .map(|o| [wheel_to_hue(angle + o, ryb), s, v]),
            )
            .collect()
    }

    /// Petite icône : la roue, ses points et leurs rayons.
    pub fn draw_icon(self, painter: &Painter, rect: Rect, color: Color32) {
        let c = rect.center();
        let r = rect.width().min(rect.height()) / 2.0 - 1.0;
        painter.circle_stroke(c, r, Stroke::new(1.0, color.gamma_multiply(0.45)));
        let dot = |p: Pos2| painter.circle_filled(p, 2.0, color);
        let ray = Stroke::new(1.0, color.gamma_multiply(0.8));
        if self == Harmony::Shades {
            for t in [0.35, 0.65, 0.95] {
                let p = pos2(c.x + r * t * 0.85, c.y - r * t * 0.5);
                painter.line_segment([c, p], ray);
                dot(p);
            }
            return;
        }
        for (i, o) in self.offsets().iter().enumerate() {
            // La base en haut à droite, comme un repère.
            let a = (30.0 + o).to_radians();
            let k = if i == 0 { 0.85 } else { 0.75 };
            let p = pos2(c.x + r * k * a.cos(), c.y - r * k * a.sin());
            painter.line_segment([c, p], ray);
            dot(p);
        }
    }
}

/// Roue TSV du sélecteur de couleur, à la valeur `value` (0 à 1) ; roue des peintres
/// (RYB) si `ryb` : les mêmes couleurs, placées autrement.
pub fn draw_hsv_wheel(painter: &Painter, rect: Rect, value: f32, ryb: bool) {
    let centre = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    let (rings, sectors) = (14usize, 72usize);
    let mut mesh = egui::Mesh::default();
    for ring in 0..=rings {
        let s = ring as f32 / rings as f32;
        for sector in 0..=sectors {
            let angle = sector as f32 / sectors as f32 * 360.0;
            let hue = wheel_to_hue(angle, ryb);
            // Placé à son angle sur la roue (la roue TSV : l'angle est la teinte).
            let pos = hsv_pos(angle, s, centre, radius, false);
            let [r, g, b] = hsv_to_rgb([hue, s, value]);
            mesh.colored_vertex(pos, Color32::from_rgb(r, g, b));
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
            coverage: Default::default(),
            swatches: vec![swatch([200, 0, 0], 0.75), swatch([0, 0, 200], 0.25)],
        };
        let rects = band_rects(&p, Rect::from_min_size(Pos2::ZERO, vec2(200.0, 20.0)));
        assert_eq!(rects[0].width(), 150.0);
        assert_eq!(rects[1].left(), 150.0);
        assert_eq!(rects[1].right(), 200.0);
    }

    #[test]
    fn hsv_round_trips_and_the_wheel_point_comes_back() {
        for rgb in [
            [255, 0, 0],
            [12, 200, 90],
            [3, 109, 255],
            [128, 128, 128],
            [0, 0, 0],
        ] {
            let hsv = rgb_to_hsv(rgb, 0.0);
            assert_eq!(hsv_to_rgb(hsv), rgb, "{rgb:?} → {hsv:?}");
        }
        assert_eq!(
            rgb_to_hsv([128, 128, 128], 200.0)[0],
            200.0,
            "le gris garde sa teinte"
        );
        let c = pos2(100.0, 100.0);
        let at = hsv_pos(210.0, 0.7, c, 50.0, false);
        let (h, s) = hsv_at(at, c, 50.0, false).unwrap();
        assert!((h - 210.0).abs() < 0.01 && (s - 0.7).abs() < 1e-4);
        assert!(hsv_at(pos2(160.0, 100.0), c, 50.0, false).is_none());
        // Sur la roue RYB aussi, aller et retour.
        let at = hsv_pos(210.0, 0.7, c, 50.0, true);
        let (h, _) = hsv_at(at, c, 50.0, true).unwrap();
        assert!((h - 210.0).abs() < 0.5, "{h}");
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

    #[test]
    fn the_painters_wheel_puts_the_known_complements_opposite() {
        // #492 : roue RYB, rouge ↔ vert, bleu ↔ orange, jaune ↔ violet.
        let opposite = |hue: f32| Harmony::Complementary.colours([hue, 1.0, 1.0], true)[1][0];
        assert!((opposite(0.0) - 120.0).abs() < 1.0, "rouge → vert");
        let orange = opposite(240.0);
        assert!((25.0..45.0).contains(&orange), "bleu → orange : {orange}");
        let violet = opposite(60.0);
        assert!(
            (260.0..300.0).contains(&violet),
            "jaune → violet : {violet}"
        );
        // Roue TSV : le complémentaire du bleu est le jaune.
        let yellow = Harmony::Complementary.colours([240.0, 1.0, 1.0], false)[1][0];
        assert!((yellow - 60.0).abs() < 1.0);
        for h in [0.0, 17.0, 95.0, 200.0, 359.0] {
            assert!(
                (wheel_to_hue(hue_to_wheel(h, true), true) - h).abs() < 0.01,
                "{h}"
            );
        }
        for harmony in Harmony::ALL {
            let colours = harmony.colours([10.0, 0.8, 0.9], true);
            assert_eq!(colours.len(), harmony.offsets().len());
            assert_eq!(colours[0], [10.0, 0.8, 0.9], "la base d'abord");
        }
    }
}
