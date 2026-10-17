//! Affichage de l'exposition calculée par le moteur ([`tri::exposure::Exposure`]) :
//! histogramme (en IL ou linéaire), indicateurs, masque des zones saturées / bouchées en
//! texture. Aucun calcul sur les pixels ici : seulement du dessin.

use eframe::egui::{Align2, Color32, ColorImage, FontId, Mesh, Painter, Rect, Stroke, pos2, vec2};
use tri::exposure::{CHANNELS, Exposure, Mask, Source, mask};

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

/// Nombre à la française, `decimals` chiffres après la virgule.
fn french(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
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
        Source::Flat => "8 bits".into(),
    };
    let text = format!(
        "{source} · marge {} IL · saturés {} % · bouchés {} %",
        french(e.headroom_ev() as f64, 2),
        french(e.over_fraction() * 100.0, 2),
        french(e.under_fraction() * 100.0, 2)
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
    // Une barre par case de l'histogramme (pas par pixel : trop de colonnes vides entre
    // deux cases donnaient un « peigne »).
    let count = match scale {
        HistogramScale::Ev => {
            let per_stop = e.ev[0].len() as f32 / e.ev_range;
            (shown_stops(e, settings.ev_shown) * per_stop).round() as usize
        }
        HistogramScale::Linear => e.linear[0].len(),
    }
    .clamp(1, graph.width().max(1.0) as usize);
    let cols = columns(e, scale, settings.ev_shown, count);
    // Échelle verticale : le plus haut pic, sans les colonnes extrêmes (noir pur, blanc
    // saturé), qui écraseraient le reste ; elles sont signalées à part.
    let peak = cols
        .iter()
        .skip(1)
        .take(cols.len().saturating_sub(2))
        .flat_map(|c| c.iter().copied())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let channel_colors = [colors.hist_red, colors.hist_green, colors.hist_blue];
    let column_w = graph.width() / cols.len() as f32;
    for c in 0..CHANNELS {
        let color = Color32::from(channel_colors[c]).gamma_multiply(0.6);
        let mut mesh = Mesh::default();
        for (i, values) in cols.iter().enumerate() {
            let h = (values[c] as f32 / peak).min(1.0) * graph.height();
            if h <= 0.0 {
                continue;
            }
            let x = graph.min.x + i as f32 * column_w;
            mesh.add_colored_rect(
                Rect::from_min_max(pos2(x, graph.max.y - h), pos2(x + column_w, graph.max.y)),
                color,
            );
        }
        painter.add(mesh);
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
                            "0 IL".into()
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
    // Repères d'alerte : pastille rouge en haut à droite si des pixels sont saturés, bleue
    // en haut à gauche s'il y en a de bouchés.
    if e.over.iter().any(|&n| n > 0) {
        painter.circle_filled(graph.right_top() + vec2(-4.0, 4.0), 4.0, colors.clip_over);
    }
    if e.under.iter().any(|&n| n > 0) {
        painter.circle_filled(graph.left_top() + vec2(4.0, 4.0), 4.0, colors.clip_under);
    }
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
    fn french_decimals() {
        assert_eq!(french(-0.456, 2), "-0,46");
    }
}
