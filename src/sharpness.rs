//! Netteté : carte de « focus peaking » (contours nets mis en évidence), pour vérifier la
//! mise au point à 1:1.
//!
//! Calculée sur l'image affichée (JPEG boîtier à sa taille, ou rendu RAW complet) : la carte
//! se superpose pixel pour pixel, orientation comprise. Le principe est celui des boîtiers :
//! un pixel est marqué si le gradient de luminance (filtre de Sobel) y dépasse un seuil
//! fixe — fixe et non relatif à l'image, pour que deux photos d'une rafale se comparent : la
//! plus nette montre plus de contours. Le JPEG boîtier est accentué par le boîtier, ce qui
//! gonfle un peu la carte ; le rendu RAW ne l'est pas. Voir `doc/sharpness.md`.

use serde::Deserialize;

use crate::bitmap::Bitmap;

/// Section `[sharpness]` du fichier de configuration.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SharpnessSettings {
    /// Seuil du gradient de luminance (Sobel, |gx| + |gy| divisé par 4 : 0 à 510 sur une
    /// image 8 bits ; un bord franc noir / blanc donne 255) au-delà duquel un pixel est
    /// marqué. Plus bas : plus de contours, bruit compris. 32 (mesuré le 17/10/2026 sur
    /// des JPEG boîtier X-T4) : lignes de la main et lèvres nettes marquées ; 48 : seulement
    /// les bords francs.
    pub threshold: u16,
    /// Épaississement des contours marqués, en pixels de chaque côté (0 : aucun). Un
    /// contour d'un pixel est à peine visible à 1:1 sur un écran haute densité.
    pub spread: u8,
    /// Voisins marqués (sur 8) qu'un pixel marqué doit avoir pour rester marqué : un contour
    /// est une ligne (au moins 2 voisins), le bruit du capteur à ISO élevé fait des points
    /// isolés. 0 : tout garder.
    pub min_neighbours: u8,
}

impl Default for SharpnessSettings {
    fn default() -> Self {
        SharpnessSettings {
            threshold: 32,
            spread: 1,
            min_neighbours: 2,
        }
    }
}

/// Carte de netteté : un octet par pixel (255 : contour net, 0 : rien), aux dimensions de
/// l'image analysée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peaking {
    /// Largeur (pixels).
    pub width: u32,
    /// Hauteur (pixels).
    pub height: u32,
    /// Valeurs, ligne par ligne.
    pub mask: Vec<u8>,
    /// Pixels marqués (points isolés retirés, avant épaississement).
    pub marked: u64,
}

impl Peaking {
    /// Part des pixels marqués (0 à 1).
    pub fn fraction(&self) -> f64 {
        let total = self.width as u64 * self.height as u64;
        if total == 0 {
            0.0
        } else {
            self.marked as f64 / total as f64
        }
    }
}

/// Carte de netteté de `image`. Les bords de l'image (1 pixel) ne sont jamais marqués.
pub fn peaking(image: &Bitmap, settings: &SharpnessSettings) -> Peaking {
    let (w, h) = (image.width as usize, image.height as usize);
    // Luminance approchée (R + 2G + B) / 4 : le vert porte l'essentiel du détail.
    let luma: Vec<i32> = image
        .data
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| (p[0] as i32 + 2 * p[1] as i32 + p[2] as i32) / 4)
        .collect();
    let mut mask = vec![0u8; w * h];
    let threshold = settings.threshold as i32;
    if w >= 3 && h >= 3 {
        for y in 1..h - 1 {
            let (up, row, down) = ((y - 1) * w, y * w, (y + 1) * w);
            for x in 1..w - 1 {
                let p = |base: usize, dx: isize| luma[(base as isize + x as isize + dx) as usize];
                let gx = (p(up, 1) + 2 * p(row, 1) + p(down, 1))
                    - (p(up, -1) + 2 * p(row, -1) + p(down, -1));
                let gy = (p(down, -1) + 2 * p(down, 0) + p(down, 1))
                    - (p(up, -1) + 2 * p(up, 0) + p(up, 1));
                if (gx.abs() + gy.abs()) / 4 >= threshold {
                    mask[row + x] = 255;
                }
            }
        }
    }
    let mask = isolated_removed(mask, w, h, settings.min_neighbours);
    let marked = mask.iter().filter(|&&m| m > 0).count() as u64;
    Peaking {
        width: image.width,
        height: image.height,
        mask: spread(mask, w, h, settings.spread as usize),
        marked,
    }
}

/// Retire les pixels marqués qui ont moins de `min` voisins marqués (sur 8).
fn isolated_removed(mask: Vec<u8>, w: usize, h: usize, min: u8) -> Vec<u8> {
    if min == 0 {
        return mask;
    }
    let mut out = mask.clone();
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] == 0 {
                continue;
            }
            let mut neighbours = 0;
            for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    if (xx, yy) != (x, y) && mask[yy * w + xx] > 0 {
                        neighbours += 1;
                    }
                }
            }
            if neighbours < min {
                out[y * w + x] = 0;
            }
        }
    }
    out
}

/// Épaissit les pixels marqués de `r` pixels de chaque côté (maximum sur un carré, en deux
/// passes : lignes puis colonnes).
fn spread(mask: Vec<u8>, w: usize, h: usize, r: usize) -> Vec<u8> {
    if r == 0 || w == 0 || h == 0 {
        return mask;
    }
    let mut rows = vec![0u8; w * h];
    for y in 0..h {
        let line = &mask[y * w..(y + 1) * w];
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            rows[y * w + x] = line[a..=b].iter().copied().max().unwrap_or(0);
        }
    }
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (a..=b).map(|yy| rows[yy * w + x]).max().unwrap_or(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Image `w`×`h` : gauche à `left`, droite à `right` (gris).
    fn edge(w: u32, h: u32, left: u8, right: u8) -> Bitmap {
        let data = (0..h)
            .flat_map(|_| (0..w).flat_map(move |x| [if x < w / 2 { left } else { right }; 3]))
            .collect();
        Bitmap {
            width: w,
            height: h,
            data,
        }
    }

    fn thin() -> SharpnessSettings {
        SharpnessSettings {
            spread: 0,
            min_neighbours: 0,
            ..SharpnessSettings::default()
        }
    }

    #[test]
    fn a_sharp_edge_is_marked_a_flat_area_is_not() {
        let p = peaking(&edge(8, 5, 0, 255), &thin());
        assert_eq!((p.width, p.height, p.mask.len()), (8, 5, 40));
        // Bord entre les colonnes 3 et 4 : marqué des deux côtés, sur les lignes intérieures.
        for y in 1..4 {
            assert_eq!(p.mask[y * 8 + 3], 255, "ligne {y}");
            assert_eq!(p.mask[y * 8 + 4], 255, "ligne {y}");
            assert_eq!(p.mask[y * 8 + 1], 0, "zone uniforme");
        }
        assert_eq!(p.mask[3], 0, "bord de l'image jamais marqué");
        assert_eq!(p.marked, 6);
        assert!((p.fraction() - 6.0 / 40.0).abs() < 1e-12);
    }

    #[test]
    fn the_threshold_is_absolute() {
        // Bord doux (100 → 120) : gradient 20, sous le seuil par défaut, au-dessus de 16.
        let soft = edge(8, 5, 100, 120);
        assert_eq!(peaking(&soft, &thin()).marked, 0);
        let low = SharpnessSettings {
            threshold: 16,
            ..thin()
        };
        assert_eq!(peaking(&soft, &low).marked, 6);
    }

    #[test]
    fn isolated_points_are_noise() {
        // Un point clair isolé sur un fond uniforme : marqué en anneau autour de lui ; les
        // pixels de l'anneau ont des voisins, le test porte sur un point unique.
        let mask = vec![0, 0, 0, 0, 255, 0, 0, 0, 0];
        assert_eq!(isolated_removed(mask.clone(), 3, 3, 2), vec![0; 9]);
        assert_eq!(isolated_removed(mask.clone(), 3, 3, 0), mask);
        // Une ligne reste.
        let line = vec![255, 255, 255, 0, 0, 0, 0, 0, 0];
        assert_eq!(isolated_removed(line.clone(), 3, 3, 1), line);
        assert_eq!(
            isolated_removed(line, 3, 3, 2),
            vec![0, 255, 0, 0, 0, 0, 0, 0, 0],
            "les bouts d'une ligne de 3 n'ont qu'un voisin"
        );
    }

    #[test]
    fn spread_thickens_the_marks() {
        let p = peaking(&edge(8, 5, 0, 255), &SharpnessSettings::default());
        assert_eq!(p.marked, 6, "compté avant épaississement");
        // Colonnes 2 à 5, toutes les lignes (les bords aussi, par débordement).
        for y in 0..5 {
            let row: Vec<u8> = p.mask[y * 8..(y + 1) * 8].to_vec();
            assert_eq!(row, [0, 0, 255, 255, 255, 255, 0, 0], "ligne {y}");
        }
    }

    #[test]
    fn tiny_images_do_not_panic() {
        for (w, h) in [(0, 0), (1, 1), (2, 5), (5, 2)] {
            let p = peaking(&edge(w, h, 0, 255), &SharpnessSettings::default());
            assert_eq!(p.marked, 0);
            assert_eq!(p.mask.len(), (w * h) as usize);
        }
    }
}
