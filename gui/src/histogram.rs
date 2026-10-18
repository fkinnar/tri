//! Affichage de l'exposition calculée par le moteur ([`tri::exposure::Exposure`]) :
//! histogramme (en IL ou linéaire), indicateurs, masque des zones saturées / bouchées en
//! texture. Aucun calcul sur les pixels ici : seulement du dessin.

use eframe::egui::{Align2, Color32, ColorImage, FontId, Mesh, Painter, Rect, Stroke, pos2, vec2};
use tri::exposure::{CHANNELS, Exposure, Mask, Source, mask};
use tri::i18n::decimal;

use crate::settings::{Colors, HistogramScale, HistogramSettings};

/// Valeurs de l'histogramme regroupées en `columns` colonnes, par canal, selon l'échelle.
/// En IL : de `-ev_shown` à 0 ; ce qui est plus sombre va dans la première colonne.
pub fn columns(
    e: &Exposure,
    scale: HistogramScale,
    ev_shown: f32,
    columns: usize,
) -> Vec<[u64; CHANNELS]> {
    let columns = columns.max(1);
    let mut out = vec![[0u64; CHANNELS]; columns];
    match scale {
        HistogramScale::Linear => {
            for (c, channel) in e.linear.iter().enumerate() {
                let bins = channel.len().max(1);
                for (i, &n) in channel.iter().enumerate() {
                    out[(i * columns / bins).min(columns - 1)][c] += n as u64;
                }
            }
        }
        HistogramScale::Ev => {
            let shown = shown_stops(e, ev_shown);
            for (c, channel) in e.ev.iter().enumerate() {
                let bins = channel.len().max(1);
                let per_stop = bins as f32 / e.ev_range;
                for (i, &n) in channel.iter().enumerate() {
                    // Centre de la case, en IL sous la saturation (négatif) : par son
                    // début, la dernière case (le blanc) tombait dans l'avant-dernière
                    // colonne.
                    let ev = (i as f32 + 0.5) / per_stop - e.ev_range;
                    let x = ((ev + shown) / shown * columns as f32).floor();
                    let col = (x.max(0.0) as usize).min(columns - 1);
                    out[col][c] += n as u64;
                }
            }
        }
    }
    out
}

/// IL montrés sous la saturation : la plage utile des données (`Exposure::stops` : ~13,9
/// pour un RAW 14 bits, ~11,7 pour une image 8 bits sRGB), plafonnée par `setting` s'il
/// est positif (0 : automatique).
pub fn shown_stops(e: &Exposure, setting: f32) -> f32 {
    let data = e.stops.clamp(1.0, e.ev_range);
    if setting > 0.0 {
        setting.min(data)
    } else {
        data
    }
}

/// Histogramme dans `rect` : fond, canaux superposés, graduations, indicateurs.
pub fn draw(
    painter: &Painter,
    rect: Rect,
    e: &Exposure,
    scale: HistogramScale,
    settings: &HistogramSettings,
    colors: &Colors,
) {
    painter.rect_filled(rect, 4.0, Color32::from_black_alpha(190));
    // Indicateurs, mis en page d'abord : sur plusieurs lignes si la place manque
    // (panneau des métadonnées), le graphe prend le reste de la hauteur.
    let source = match e.source {
        Source::Raw { bits, .. } => format!("RAW {bits} bits"),
        Source::Flat { bits } => format!("{bits} bits"),
    };
    let text = t!(
        "histogram-indicators",
        source = source,
        headroom = decimal(e.headroom_ev() as f64, 2),
        over = decimal(e.over_fraction() * 100.0, 2),
        under = decimal(e.under_fraction() * 100.0, 2)
    );
    let galley = painter.layout(
        text,
        FontId::proportional(11.0),
        Color32::from_gray(225),
        (rect.width() - 12.0).max(10.0),
    );
    let text_h = galley.size().y + 4.0;
    painter.galley(
        pos2(rect.min.x + 6.0, rect.max.y - 4.0 - galley.size().y),
        galley,
        Color32::from_gray(225),
    );
    // Bande réservée aux graduations, sous le graphe.
    let labels_h = 12.0;
    let graph = Rect::from_min_max(
        rect.min + vec2(6.0, 6.0),
        pos2(rect.max.x - 6.0, rect.max.y - text_h - labels_h - 6.0),
    );
    // Courbe lissée (retour du 17/10/2026 : les barres faisaient escalier) : des classes
    // plus larges que les cases du moteur, puis un lissage gaussien sur les classes
    // voisines. Les classes larges comblent déjà les cases vides d'un JPEG en échelle IL,
    // qui donnaient un « peigne ». Pas d'échelle verticale : seule la forme compte.
    let count = settings.classes.clamp(8, graph.width().max(8.0) as usize);
    let cols = columns(e, scale, settings.ev_shown, count);
    let curves: Vec<Vec<f32>> = (0..CHANNELS)
        .map(|c| {
            let raw: Vec<f32> = cols.iter().map(|v| v[c] as f32).collect();
            smooth(&raw, settings.smoothing)
        })
        .collect();
    // Échelle verticale : le plus haut pic, sans les classes extrêmes (noir pur, blanc
    // saturé), qui écraseraient le reste ; elles sont signalées à part.
    let peak = curves
        .iter()
        .flat_map(|v| v.iter().skip(1).take(v.len().saturating_sub(2)).copied())
        .fold(0.0f32, f32::max)
        .max(1e-6);
    let channel_colors = [colors.hist_red, colors.hist_green, colors.hist_blue];
    let class_w = graph.width() / count as f32;
    for (c, curve) in curves.iter().enumerate() {
        let base = Color32::from(channel_colors[c]);
        let y = |v: f32| graph.max.y - (v / peak).min(1.0) * graph.height();
        // Points au centre des classes, prolongés jusqu'aux deux bords.
        let mut points = Vec::with_capacity(curve.len() + 2);
        points.push(pos2(graph.min.x, y(curve[0])));
        for (i, &v) in curve.iter().enumerate() {
            points.push(pos2(graph.min.x + (i as f32 + 0.5) * class_w, y(v)));
        }
        points.push(pos2(graph.max.x, y(curve[curve.len() - 1])));
        // Surface : un trapèze par segment, jusqu'à la ligne de base.
        let fill = base.gamma_multiply(0.45);
        let mut mesh = Mesh::default();
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let i = mesh.vertices.len() as u32;
            for p in [a, b, pos2(b.x, graph.max.y), pos2(a.x, graph.max.y)] {
                mesh.colored_vertex(p, fill);
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
        painter.add(mesh);
        painter.add(eframe::egui::Shape::line(
            points,
            Stroke::new(1.2, base.gamma_multiply(0.9)),
        ));
    }
    // Graduations.
    let grid = Stroke::new(1.0, Color32::from_white_alpha(40));
    let label = |x: f32, text: String| {
        painter.text(
            pos2(x, graph.max.y + 1.0),
            Align2::CENTER_TOP,
            text,
            FontId::proportional(10.0),
            Color32::from_gray(170),
        );
    };
    match scale {
        HistogramScale::Ev => {
            let shown = shown_stops(e, settings.ev_shown);
            let stops = shown.floor() as i32;
            for k in 0..=stops {
                let x = graph.max.x - k as f32 / shown * graph.width();
                painter.line_segment([pos2(x, graph.min.y), pos2(x, graph.max.y)], grid);
                if k % 2 == 0 {
                    label(
                        x,
                        if k == 0 {
                            t!("histogram-zero-ev")
                        } else {
                            format!("-{k}")
                        },
                    );
                }
            }
        }
        HistogramScale::Linear => {
            for k in 1..4 {
                let x = graph.min.x + k as f32 / 4.0 * graph.width();
                painter.line_segment([pos2(x, graph.min.y), pos2(x, graph.max.y)], grid);
            }
            label(graph.max.x - 12.0, "blanc".into());
        }
    }
    // Repères d'alerte : pastille en haut à droite si des pixels sont saturés, en haut à
    // gauche s'il y en a de bouchés, au-delà d'une petite part des pixels (retour du
    // 17/10/2026 : allumées dès un pixel, elles s'allumaient pour quelques pixels morts,
    // la courbe loin des bords).
    if e.over_fraction() >= settings.indicator_min {
        painter.circle_filled(graph.right_top() + vec2(-4.0, 4.0), 4.0, colors.clip_over);
    }
    if e.under_fraction() >= settings.indicator_min {
        painter.circle_filled(graph.left_top() + vec2(4.0, 4.0), 4.0, colors.clip_under);
    }
}

/// Lissage gaussien de `values` (écart type `sigma`, en classes ; 0 : inchangé). Les bords
/// sont traités en ne comptant que les voisins existants (pas de chute artificielle).
pub fn smooth(values: &[f32], sigma: f32) -> Vec<f32> {
    if sigma <= 0.0 || values.is_empty() {
        return values.to_vec();
    }
    let radius = (sigma * 3.0).ceil() as isize;
    let weights: Vec<f32> = (-radius..=radius)
        .map(|k| (-(k * k) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    (0..values.len() as isize)
        .map(|i| {
            let (mut sum, mut total) = (0.0, 0.0);
            for (k, w) in (-radius..=radius).zip(&weights) {
                if let Some(&v) = values.get((i + k) as usize).filter(|_| i + k >= 0) {
                    sum += v * w;
                    total += w;
                }
            }
            sum / total
        })
        .collect()
}

/// Couleurs des zones.
#[derive(Clone, Copy, Debug)]
pub struct ZoneColors {
    /// Saturé dans le seul canal rouge, vert, bleu.
    pub channels: [Color32; CHANNELS],
    /// Saturé dans les trois canaux (ou dans un canal quelconque, sans couleur par canal).
    pub all: Color32,
    /// Bouché.
    pub under: Color32,
    /// Couleur par canal saturé ; sinon `all` pour toute saturation.
    pub per_channel: bool,
}

impl ZoneColors {
    /// Couleur d'une case du masque, transparente si rien à signaler. Deux canaux
    /// saturés : leurs couleurs additionnées (rouge + vert = jaune).
    pub fn of(&self, cell: u8) -> Color32 {
        let over = cell & mask::OVER;
        if over != 0 {
            if !self.per_channel || over == mask::OVER {
                return self.all;
            }
            let mut rgb = [0u8; 3];
            for (c, color) in self.channels.iter().enumerate() {
                if over & (1 << c) != 0 {
                    for (v, k) in rgb.iter_mut().zip(color.to_array()) {
                        *v = (*v).max(k);
                    }
                }
            }
            return Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        }
        if cell & mask::UNDER != 0 {
            self.under
        } else {
            Color32::TRANSPARENT
        }
    }
}

/// Masque des zones en image (voir [`ZoneColors`]), transparent là où il n'y a rien à
/// signaler. À afficher étiré sur l'image, en échantillonnage au plus proche.
pub fn mask_image(m: &Mask, colors: &ZoneColors) -> ColorImage {
    let pixels = m.cells.iter().map(|&cell| colors.of(cell)).collect();
    ColorImage::new([m.width as usize, m.height as usize], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tri::bitmap::Bitmap;
    use tri::exposure::ExposureSettings;

    fn grey(value: u8) -> Exposure {
        let b = Bitmap {
            width: 4,
            height: 4,
            data: vec![value; 48],
        };
        Exposure::from_bitmap(&b, &ExposureSettings::default())
    }

    #[test]
    fn columns_keep_every_pixel() {
        let e = grey(64);
        for scale in [HistogramScale::Ev, HistogramScale::Linear] {
            let cols = columns(&e, scale, 12.0, 100);
            for c in 0..CHANNELS {
                assert_eq!(cols.iter().map(|v| v[c]).sum::<u64>(), 16, "{scale:?}");
            }
        }
    }

    #[test]
    fn ev_columns_place_values_by_stops() {
        // 64 sur 255 en sRGB : -4,28 IL. Plafond de 12 IL, mais la plage d'une image
        // 8 bits sRGB n'est que de 11,68 IL : 120 colonnes pour 11,68 IL, colonne
        // (11,68 - 4,28) / 11,68 × 120 ≈ 76.
        let cols = columns(&grey(64), HistogramScale::Ev, 12.0, 120);
        let col = cols.iter().position(|v| v[0] > 0).unwrap();
        assert!((74..=78).contains(&col), "colonne {col}");
        assert!(
            (shown_stops(&grey(64), 12.0) - 11.68).abs() < 0.01,
            "plafonné à la plage"
        );
        assert!(
            (shown_stops(&grey(64), 0.0) - 11.68).abs() < 0.01,
            "automatique"
        );
        assert_eq!(shown_stops(&grey(64), 6.0), 6.0, "plafond plus petit");
        // Le blanc tout à droite, le noir tout à gauche.
        assert_eq!(
            columns(&grey(255), HistogramScale::Ev, 12.0, 120)[119][1],
            16
        );
        assert_eq!(columns(&grey(0), HistogramScale::Ev, 12.0, 120)[0][2], 16);
    }

    fn zones(per_channel: bool) -> ZoneColors {
        ZoneColors {
            channels: [
                Color32::from_rgb(255, 0, 0),
                Color32::from_rgb(0, 255, 0),
                Color32::from_rgb(0, 0, 255),
            ],
            all: Color32::from_rgb(255, 0, 255),
            under: Color32::from_rgb(0, 200, 255),
            per_channel,
        }
    }

    #[test]
    fn mask_colors() {
        let m = Mask {
            width: 3,
            height: 1,
            cells: vec![0, mask::OVER_G, mask::UNDER],
        };
        let image = mask_image(&m, &zones(true));
        assert_eq!(image.size, [3, 1]);
        assert_eq!(
            image.pixels,
            [
                Color32::TRANSPARENT,
                Color32::from_rgb(0, 255, 0),
                Color32::from_rgb(0, 200, 255)
            ]
        );
    }

    #[test]
    fn channel_zones_mix_and_saturate_all() {
        let z = zones(true);
        assert_eq!(z.of(mask::OVER_R), Color32::from_rgb(255, 0, 0));
        assert_eq!(
            z.of(mask::OVER_R | mask::OVER_G),
            Color32::from_rgb(255, 255, 0),
            "rouge + vert = jaune"
        );
        assert_eq!(z.of(mask::OVER), z.all, "trois canaux");
        assert_eq!(
            z.of(mask::OVER_B | mask::UNDER),
            Color32::from_rgb(0, 0, 255),
            "saturé l'emporte"
        );
        let single = zones(false);
        assert_eq!(single.of(mask::OVER_R), single.all, "une seule couleur");
    }

    #[test]
    fn decimals_follow_the_language() {
        crate::i18n::register();
        tri::i18n::set_language(tri::i18n::Language::Fr);
        assert_eq!(decimal(-0.456, 2), "-0,46");
    }

    #[test]
    fn smoothing_fills_a_comb_and_keeps_the_shape() {
        // Peigne : une classe sur deux vide.
        let comb: Vec<f32> = (0..20)
            .map(|i| if i % 2 == 0 { 10.0 } else { 0.0 })
            .collect();
        let s = smooth(&comb, 1.5);
        let (min, max) = s[3..17]
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(max - min < 1.0, "peigne comblé : {min} à {max}");
        // Un pic reste un pic, au même endroit.
        let mut peak = vec![0.0f32; 21];
        peak[10] = 100.0;
        let s = smooth(&peak, 1.5);
        let top = s
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(top, 10);
        assert_eq!(smooth(&peak, 0.0), peak, "sans lissage : inchangé");
    }
}
