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
            let shown = ev_shown.clamp(1.0, e.ev_range);
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
    // Bandes réservées sous le graphe : graduations, puis indicateurs.
    let (labels_h, text_h) = (12.0, 16.0);
    let graph = Rect::from_min_max(
        rect.min + vec2(6.0, 6.0),
        pos2(rect.max.x - 6.0, rect.max.y - text_h - labels_h - 6.0),
    );
    // Une barre par case de l'histogramme (pas par pixel : trop de colonnes vides entre
    // deux cases donnaient un « peigne »).
    let count = match scale {
        HistogramScale::Ev => {
            let per_stop = e.ev[0].len() as f32 / e.ev_range;
            (settings.ev_shown.clamp(1.0, e.ev_range) * per_stop).round() as usize
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
            let shown = settings.ev_shown.clamp(1.0, e.ev_range);
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
    // Indicateurs.
    let source = match e.source {
        Source::Raw { bits, .. } => format!("RAW {bits} bits"),
        Source::Flat => "8 bits".into(),
    };
    let over = e.over_fraction() * 100.0;
    let under = e.under_fraction() * 100.0;
    let text = format!(
        "{source} · marge {} IL · saturés {} % · bouchés {} %",
        french(e.headroom_ev() as f64, 2),
        french(over, 2),
        french(under, 2)
    );
    painter.text(
        pos2(rect.min.x + 6.0, rect.max.y - 5.0),
        Align2::LEFT_BOTTOM,
        text,
        FontId::proportional(11.0),
        Color32::from_gray(225),
    );
}

/// Masque des zones en image : saturé (un canal au moins) en `over`, bouché en `under`,
/// transparent ailleurs. À afficher étiré sur l'image, en échantillonnage au plus proche.
pub fn mask_image(m: &Mask, over: Color32, under: Color32) -> ColorImage {
    let pixels = m
        .cells
        .iter()
        .map(|&cell| {
            if cell & mask::OVER != 0 {
                over
            } else if cell & mask::UNDER != 0 {
                under
            } else {
                Color32::TRANSPARENT
            }
        })
        .collect();
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
        // 64 / 255 : -2 IL ; sur 12 IL et 120 colonnes (10 par IL), colonne 99.
        let cols = columns(&grey(64), HistogramScale::Ev, 12.0, 120);
        let col = cols.iter().position(|v| v[0] > 0).unwrap();
        assert!((99..=100).contains(&col), "colonne {col}");
        // Le blanc tout à droite, le noir tout à gauche.
        assert_eq!(
            columns(&grey(255), HistogramScale::Ev, 12.0, 120)[119][1],
            16
        );
        assert_eq!(columns(&grey(0), HistogramScale::Ev, 12.0, 120)[0][2], 16);
    }

    #[test]
    fn mask_colors() {
        let m = Mask {
            width: 3,
            height: 1,
            cells: vec![0, mask::OVER_G, mask::UNDER],
        };
        let image = mask_image(&m, Color32::RED, Color32::BLUE);
        assert_eq!(image.size, [3, 1]);
        assert_eq!(
            image.pixels,
            [Color32::TRANSPARENT, Color32::RED, Color32::BLUE]
        );
    }

    #[test]
    fn french_decimals() {
        assert_eq!(french(-0.456, 2), "-0,46");
    }
}
