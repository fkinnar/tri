//! Netteté : carte de « focus peaking » (contours nets mis en évidence), pour vérifier la
//! mise au point à 1:1.
//!
//! Calculée sur l'image affichée (JPEG boîtier à sa taille, ou rendu RAW complet) : la carte
//! se superpose pixel pour pixel, orientation comprise. Le principe est celui des boîtiers :
//! un pixel est marqué si le gradient de luminance (filtre de Sobel) y dépasse un seuil
//! fixe — fixe et non relatif à l'image, pour que deux photos d'une rafale se comparent : la
//! plus nette montre plus de contours. Le JPEG boîtier est accentué par le boîtier, ce qui
//! gonfle un peu la carte ; le rendu RAW ne l'est pas.
//!
//! **Score de netteté** ([`score`]) : pour repérer les photos franchement floues (mise au
//! point ratée, bougé), sans IA. L'image, réduite à une taille fixe (les boîtiers se
//! comparent, le bruit pèse moins), est découpée en cases ; l'énergie du gradient de
//! chaque case est mesurée, et le score est celle des cases **les plus nettes** : un
//! portrait au fond flou garde un bon score, un bougé fait tout chuter. Le score ne se
//! juge pas dans l'absolu (ISO, objectif, accentuation du boîtier) mais **par rapport à la
//! médiane du dossier** ([`BlurFilter`]). Voir `doc/sharpness.md`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::bitmap::Bitmap;
use crate::filter::{Filter, Groups, retain};

/// Version du calcul du score, dans la clé du cache d'analyse : à incrémenter quand le
/// calcul change sans que les réglages changent.
/// (2 : luminance étirée avant la mesure, le score ne dépend plus de l'exposition.)
pub const SCORE_VERSION: u32 = 2;

/// Section `[sharpness]` du fichier de configuration.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SharpnessSettings {
    /// Seuil du gradient de luminance (Sobel, |gx| + |gy| divisé par 4 : 0 à 510 sur une
    /// image 8 bits ; un bord franc noir / blanc donne 255) au-delà duquel un pixel est
    /// marqué. Plus bas : plus de contours, bruit compris. 48 (17/10/2026, retour : 32
    /// était trop permissif) : sur des RAF X-T4 ratés exprès, une photo molle n'est
    /// presque plus marquée (1 % des pixels contre 4 % à 32), une nette le reste (8 %).
    pub threshold: u16,
    /// Épaississement des contours marqués, en pixels de chaque côté (0 : aucun). Un
    /// contour d'un pixel est à peine visible à 1:1 sur un écran haute densité.
    pub spread: u8,
    /// Voisins marqués (sur 8) qu'un pixel marqué doit avoir pour rester marqué : un contour
    /// est une ligne (au moins 2 voisins), le bruit du capteur à ISO élevé fait des points
    /// isolés. 0 : tout garder.
    pub min_neighbours: u8,
    /// Score : côté long (pixels) de l'image analysée, réduite à cette taille.
    pub score_side: u32,
    /// Score : cases sur le côté long de l'image (le côté court en a proportionnellement
    /// moins).
    pub score_tiles: u32,
    /// Score : part des cases les plus nettes dont la moyenne fait le score (0 à 1).
    pub score_top_share: f32,
    /// Score : la luminance est étirée entre ses centiles `stretch` et `1 - stretch` avant
    /// la mesure (0,01 : 1 % et 99 %), pour qu'une photo sous-exposée mais nette ne passe
    /// pas pour floue (mesuré le 17/10/2026 : un portrait sombre et net était à 0,21 × la
    /// médiane).
    pub score_stretch: f32,
    /// Score : écart de luminance minimal (0 à 255) entre ces centiles ; en dessous,
    /// l'image n'a pas de détail (toute blanche, toute noire) et son score est nul.
    pub score_min_range: u8,
    /// Photo jugée floue sous cette fraction de la médiane des scores du dossier.
    pub blur_ratio: f32,
    /// Voisines comparées de chaque côté (photos avant et après, dans l'ordre du dossier ;
    /// 0 : pas de comparaison aux voisines).
    pub neighbours: u32,
    /// Photo jugée floue sous cette fraction de la médiane de ses voisines.
    pub neighbour_ratio: f32,
    /// Piles connues (images semblables, `similarity`) : une photo n'est plus comparée à
    /// ses voisines (une autre pile est une autre scène), mais aux autres images de sa
    /// pile si elle en compte au moins ce nombre ; le dossier compte toujours.
    pub group_min: u32,
    /// Dans une pile : photo jugée floue sous cette fraction de la médiane des autres.
    pub group_ratio: f32,
    /// Seuil de regroupement pour le jugement (distance d'empreinte, voir `similarity`) :
    /// plus large que celui des piles affichées, la « même scène » d'une comparaison de
    /// netteté dépasse la rafale (mesuré le 18/10/2026 : 0,45 garde 14 photos ratées sur
    /// 15 sans faux positif, et retire les faux positifs d'un dossier aux sujets variés).
    pub group_threshold: f32,
    /// Calcul du score de tout le dossier en fond (cache d'analyse).
    pub background: bool,
}

impl Default for SharpnessSettings {
    fn default() -> Self {
        SharpnessSettings {
            threshold: 48,
            spread: 1,
            min_neighbours: 2,
            score_side: 1536,
            score_tiles: 16,
            score_top_share: 0.05,
            score_stretch: 0.01,
            score_min_range: 8,
            blur_ratio: 0.5,
            neighbours: 3,
            neighbour_ratio: 0.8,
            group_min: 3,
            group_ratio: 0.8,
            group_threshold: 0.45,
            background: true,
        }
    }
}

/// Score de netteté d'une image.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    /// Score : gradient moyen (racine de l'énergie, 0 à ~255) des cases les plus nettes.
    pub value: f32,
    /// Cases en largeur.
    pub cols: u32,
    /// Cases en hauteur.
    pub rows: u32,
    /// Gradient moyen de chaque case, ligne par ligne (carte grossière, pour plus tard).
    pub tiles: Vec<f32>,
}

/// Score de netteté de `image` (déjà réduite à `score_side`, voir [`SharpnessSettings`]).
pub fn score(image: &Bitmap, settings: &SharpnessSettings) -> Score {
    let (w, h) = (image.width as usize, image.height as usize);
    let long = w.max(h).max(1);
    let n = settings.score_tiles.max(1) as usize;
    let cols = ((w * n).div_ceil(long)).max(1);
    let rows = ((h * n).div_ceil(long)).max(1);
    let raw: Vec<u8> = image
        .data
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| ((p[0] as u32 + 2 * p[1] as u32 + p[2] as u32) / 4) as u8)
        .collect();
    let luma = stretched(&raw, settings);
    let mut energy = vec![0f64; cols * rows];
    let mut count = vec![0u32; cols * rows];
    if w >= 3 && h >= 3 {
        for y in 1..h - 1 {
            let (up, row, down) = ((y - 1) * w, y * w, (y + 1) * w);
            let tile_row = y * rows / h;
            for x in 1..w - 1 {
                let p = |base: usize, dx: isize| luma[(base as isize + x as isize + dx) as usize];
                let gx = (p(up, 1) + 2 * p(row, 1) + p(down, 1))
                    - (p(up, -1) + 2 * p(row, -1) + p(down, -1));
                let gy = (p(down, -1) + 2 * p(down, 0) + p(down, 1))
                    - (p(up, -1) + 2 * p(up, 0) + p(up, 1));
                // /4 (Sobel) et /16 (luminance étirée en ×16) : 0 à ~255.
                let (gx, gy) = (gx as f64 / 64.0, gy as f64 / 64.0);
                let t = tile_row * cols + x * cols / w;
                energy[t] += gx * gx + gy * gy;
                count[t] += 1;
            }
        }
    }
    let tiles: Vec<f32> = energy
        .iter()
        .zip(&count)
        .map(|(&e, &c)| {
            if c == 0 {
                0.0
            } else {
                (e / c as f64).sqrt() as f32
            }
        })
        .collect();
    let mut sorted = tiles.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let top =
        ((sorted.len() as f32 * settings.score_top_share).round() as usize).clamp(1, sorted.len());
    let value = sorted[..top].iter().sum::<f32>() / top as f32;
    Score {
        value,
        cols: cols as u32,
        rows: rows as u32,
        tiles,
    }
}

/// Luminance étirée sur 0–255 entre ses centiles `score_stretch` et `1 - score_stretch`
/// (en valeurs ×16, pour garder de la précision) ; tout à 0 si l'écart est sous
/// `score_min_range`.
fn stretched(luma: &[u8], settings: &SharpnessSettings) -> Vec<i32> {
    let mut histogram = [0u64; 256];
    for &v in luma {
        histogram[v as usize] += 1;
    }
    let n = luma.len() as u64;
    let cut = ((n as f64) * settings.score_stretch.clamp(0.0, 0.49) as f64) as u64;
    let centile = |from_top: bool| -> i32 {
        let mut seen = 0;
        for i in 0..256 {
            let v = if from_top { 255 - i } else { i };
            seen += histogram[v];
            if seen > cut {
                return v as i32;
            }
        }
        0
    };
    let (low, high) = (centile(false), centile(true));
    let range = high - low;
    if range < settings.score_min_range as i32 || range <= 0 {
        return vec![0; luma.len()];
    }
    luma.iter()
        .map(|&v| ((v as i32 - low).clamp(0, range) * 255 * 16) / range)
        .collect()
}

// Format binaire du cache d'analyse : petit-boutiste, entête versionné.
const MAGIC: &[u8; 8] = b"TRISHP01";

impl Score {
    /// Sérialise pour le cache d'analyse (~1 Ko).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&self.value.to_le_bytes());
        out.extend_from_slice(&self.cols.to_le_bytes());
        out.extend_from_slice(&self.rows.to_le_bytes());
        for t in &self.tiles {
            out.extend_from_slice(&t.to_le_bytes());
        }
        out
    }

    /// Relit des données de [`Score::to_bytes`] ; `None` si elles sont illisibles.
    pub fn from_bytes(bytes: &[u8]) -> Option<Score> {
        let rest = bytes.strip_prefix(MAGIC)?;
        let word = |i: usize| -> Option<[u8; 4]> { rest.get(i * 4..i * 4 + 4)?.try_into().ok() };
        let value = f32::from_le_bytes(word(0)?);
        let cols = u32::from_le_bytes(word(1)?);
        let rows = u32::from_le_bytes(word(2)?);
        let n = (cols as usize).checked_mul(rows as usize)?;
        if rest.len() != (3 + n) * 4 {
            return None;
        }
        let tiles = (0..n)
            .map(|i| word(3 + i).map(f32::from_le_bytes))
            .collect::<Option<Vec<f32>>>()?;
        Some(Score {
            value,
            cols,
            rows,
            tiles,
        })
    }
}

/// Médiane des scores (`None` sans score).
pub fn median(scores: impl IntoIterator<Item = f32>) -> Option<f32> {
    let mut v: Vec<f32> = scores.into_iter().filter(|s| s.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let m = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[m]
    } else {
        (v[m - 1] + v[m]) / 2.0
    })
}

/// Score rapporté à la médiane du dossier (1 : comme la médiane ; 0,5 : deux fois moins
/// net). `None` si la médiane est nulle.
pub fn relative(score: f32, median: f32) -> Option<f32> {
    (median > 0.0).then(|| score / median)
}

/// Jugement d'une photo : son score rapporté au dossier et à ses voisines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Judgement {
    /// Score de netteté.
    pub score: f32,
    /// Score / médiane du dossier.
    pub folder_ratio: Option<f32>,
    /// Score / médiane des voisines (même scène, même rafale le plus souvent).
    pub neighbour_ratio: Option<f32>,
    /// Score / médiane des autres images de sa pile, si elle est dans une pile d'au moins
    /// `group_min` images (jugée alors seulement contre elle).
    pub group_ratio: Option<f32>,
    /// Jugée floue : sous `blur_ratio` × la médiane du dossier, **ou** sous
    /// `neighbour_ratio` × celle de ses voisines.
    pub blurry: bool,
}

/// Juge les photos d'un dossier, dans l'ordre du dossier (`ordered` : chemin et score, `None`
/// si le score n'est pas encore connu ; ces photos-là ne sont ni jugées ni comptées).
///
/// Deux règles, mesurées le 17/10/2026 sur 94 photos ratées exprès et étiquetées par
/// l'utilisateur, se complètent :
/// - **dossier** : les longues séries floues (leurs voisines sont floues aussi) ;
/// - **voisines** (`neighbours` photos avant et après) : une photo ratée isolée dans une
///   série nette, qui reste au niveau du dossier parce que la scène est très texturée.
pub fn judge(
    ordered: &[(PathBuf, Option<f32>)],
    settings: &SharpnessSettings,
) -> HashMap<PathBuf, Judgement> {
    judge_in_groups(ordered, &[], settings)
}

/// [`judge`], avec les piles de `ordered` (rangs dans `ordered`, voir
/// `similarity::group`) : une photo d'une pile d'au moins `group_min` images n'est jugée
/// que contre sa pile (décidé le 18/10/2026 : comparer des gros plans sur fond flou aux
/// paysages d'un même dossier les faisait passer pour flous) ; seule ou dans une petite
/// pile, comme [`judge`].
pub fn judge_in_groups(
    ordered: &[(PathBuf, Option<f32>)],
    groups: &[std::ops::Range<usize>],
    settings: &SharpnessSettings,
) -> HashMap<PathBuf, Judgement> {
    let mut group_of: HashMap<&PathBuf, &std::ops::Range<usize>> = HashMap::new();
    for g in groups.iter().filter(|g| {
        settings.group_min > 0 && g.len() >= settings.group_min as usize && g.end <= ordered.len()
    }) {
        for i in g.clone() {
            group_of.insert(&ordered[i].0, g);
        }
    }
    let grouped = !groups.is_empty();
    let scored: Vec<(&PathBuf, f32)> = ordered
        .iter()
        .filter_map(|(p, s)| s.filter(|v| v.is_finite()).map(|v| (p, v)))
        .collect();
    let folder = median(scored.iter().map(|s| s.1));
    let k = settings.neighbours as usize;
    scored
        .iter()
        .enumerate()
        .map(|(i, &(path, score))| {
            let folder_ratio = folder.and_then(|m| relative(score, m));
            let around = (i.saturating_sub(k)..(i + k + 1).min(scored.len()))
                .filter(|&j| j != i)
                .map(|j| scored[j].1);
            let neighbour_ratio = (k > 0)
                .then(|| median(around))
                .flatten()
                .and_then(|m| relative(score, m));
            let group_ratio = group_of.get(path).and_then(|g| {
                let others = (*g)
                    .clone()
                    .filter(|&j| ordered[j].0 != *path)
                    .filter_map(|j| ordered[j].1.filter(|v| v.is_finite()));
                median(others).and_then(|m| relative(score, m))
            });
            // Piles connues : les voisines d'une autre pile sont une autre scène, on ne s'y
            // compare plus ; la pile les remplace. Le dossier reste : il attrape une rafale
            // floue de bout en bout (mesuré le 18/10/2026, voir `doc/sharpness.md`).
            let neighbours_count = if grouped {
                false
            } else {
                neighbour_ratio.is_some_and(|r| r < settings.neighbour_ratio)
            };
            let blurry = folder_ratio.is_some_and(|r| r < settings.blur_ratio)
                || group_ratio.is_some_and(|r| r < settings.group_ratio)
                || neighbours_count;
            let judgement = Judgement {
                score,
                folder_ratio,
                neighbour_ratio,
                group_ratio,
                blurry,
            };
            (path.clone(), judgement)
        })
        .collect()
}

/// Ce que garde le filtre sur la netteté.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurRule {
    /// Masquer les photos jugées floues : une photo sans jugement (score pas encore
    /// calculé) est gardée, on ne cache pas ce qu'on n'a pas jugé.
    HideBlurry,
    /// Seulement les photos jugées floues (pour les vérifier avant de les rejeter).
    OnlyBlurry,
}

/// Filtre sur la netteté ([`judge`]).
pub struct BlurFilter<'a> {
    /// Jugements, par chemin.
    pub judgements: &'a HashMap<PathBuf, Judgement>,
    /// Ce qui est gardé.
    pub rule: BlurRule,
}

impl Filter for BlurFilter<'_> {
    fn apply(&self, groups: Groups) -> Groups {
        retain(groups, |f| {
            let blurry = self.judgements.get(&f.path).map(|j| j.blurry);
            match self.rule {
                BlurRule::HideBlurry => blurry != Some(true),
                BlurRule::OnlyBlurry => blurry == Some(true),
            }
        })
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

    /// Image de mire : un damier de `square` pixels dans le rectangle (x0, y0)–(x1, y1),
    /// gris uniforme ailleurs ; `blur` : moyennes successives 3 × 3 (flou).
    fn scene(w: u32, h: u32, square: u32, area: (u32, u32, u32, u32), blur: usize) -> Bitmap {
        let mut luma: Vec<u8> = (0..h)
            .flat_map(|y| {
                (0..w).map(move |x| {
                    let inside = x >= area.0 && x < area.2 && y >= area.1 && y < area.3;
                    if inside && ((x / square + y / square).is_multiple_of(2)) {
                        230
                    } else if inside {
                        30
                    } else {
                        128
                    }
                })
            })
            .collect();
        for _ in 0..blur {
            let src = luma.clone();
            for y in 1..h as usize - 1 {
                for x in 1..w as usize - 1 {
                    let mut sum = 0u32;
                    for dy in 0..3 {
                        for dx in 0..3 {
                            sum += src[(y + dy - 1) * w as usize + x + dx - 1] as u32;
                        }
                    }
                    luma[y * w as usize + x] = (sum / 9) as u8;
                }
            }
        }
        Bitmap {
            width: w,
            height: h,
            data: luma.iter().flat_map(|&v| [v, v, v]).collect(),
        }
    }

    #[test]
    fn known_stacks_replace_the_neighbours_and_judge_within_the_stack() {
        let p = |i: usize| PathBuf::from(format!("{i}.jpg"));
        // Paysages nets (100), puis une pile de gros plans (60) dont un raté (30), puis un
        // paysage à 60 entre deux scènes sans rapport.
        let scores = [100.0, 100.0, 60.0, 62.0, 30.0, 61.0, 100.0, 60.0, 100.0];
        let ordered: Vec<(PathBuf, Option<f32>)> = scores
            .iter()
            .enumerate()
            .map(|(i, &v)| (p(i), Some(v)))
            .collect();
        let s = SharpnessSettings::default();
        let groups = [0..2, 2..6, 6..7, 7..8, 8..9];
        let j = judge_in_groups(&ordered, &groups, &s);
        assert!(j[&p(4)].blurry, "le raté de la pile");
        assert!(j[&p(4)].group_ratio.unwrap() < 0.6);
        assert!(!j[&p(2)].blurry && !j[&p(3)].blurry, "les gros plans nets");
        assert!(
            !j[&p(7)].blurry,
            "seule : pas comparée à des voisines d'une autre scène"
        );
        assert_eq!(j[&p(7)].group_ratio, None);
        // Sans piles : la règle des voisines la marquait.
        assert!(judge(&ordered, &s)[&p(7)].blurry);
    }

    #[test]
    fn blur_lowers_the_score_and_a_sharp_subject_on_a_plain_background_keeps_it() {
        let settings = SharpnessSettings::default();
        let full = (0, 0, 256, 192);
        let sharp = score(&scene(256, 192, 4, full, 0), &settings);
        let blurred = score(&scene(256, 192, 4, full, 4), &settings);
        assert!(
            blurred.value < sharp.value * 0.5,
            "{} / {}",
            blurred.value,
            sharp.value
        );
        // Sujet net sur un quart de l'image, fond uni : les cases nettes font le score.
        let subject = score(&scene(256, 192, 4, (64, 48, 128, 96), 0), &settings);
        assert!(
            subject.value > sharp.value * 0.8,
            "{} / {}",
            subject.value,
            sharp.value
        );
        assert_eq!((sharp.cols, sharp.rows), (16, 12));
        assert_eq!(sharp.tiles.len(), 16 * 12);
    }

    #[test]
    fn scores_round_trip_and_reject_garbage() {
        let s = score(
            &scene(64, 48, 4, (0, 0, 64, 48), 0),
            &SharpnessSettings::default(),
        );
        assert_eq!(Score::from_bytes(&s.to_bytes()), Some(s.clone()));
        let bytes = s.to_bytes();
        assert_eq!(
            Score::from_bytes(&bytes[..bytes.len() - 1]),
            None,
            "tronqué"
        );
        assert_eq!(Score::from_bytes(b"pas un score"), None);
    }

    #[test]
    fn blurry_is_relative_to_the_folder_and_to_the_neighbours() {
        let settings = SharpnessSettings::default();
        assert_eq!(median([3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median([4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median([]), None);
        let p = |i: usize| PathBuf::from(format!("img{i}"));
        // Série nette (10) avec une ratée isolée (7 : au niveau du dossier, mais sous ses
        // voisines), puis une longue série floue (2 : ses voisines aussi), une photo sans
        // score.
        let scores = [
            10.0, 10.0, 10.0, 7.0, 10.0, 10.0, 10.0, 10.0, 2.0, 2.0, 2.0, 2.0,
        ];
        let mut ordered: Vec<(PathBuf, Option<f32>)> = scores
            .iter()
            .enumerate()
            .map(|(i, &s)| (p(i), Some(s)))
            .collect();
        ordered.push((p(99), None));
        let j = judge(&ordered, &settings);
        assert!(j[&p(3)].blurry, "ratée isolée : {:?}", j[&p(3)]);
        assert!((j[&p(3)].neighbour_ratio.unwrap() - 0.7).abs() < 1e-6);
        assert!(!j[&p(0)].blurry && !j[&p(7)].blurry);
        assert!(j[&p(10)].blurry, "série floue : {:?}", j[&p(10)]);
        assert!(!j.contains_key(&p(99)), "pas de score, pas de jugement");
        // Sans comparaison aux voisines, la ratée isolée passe.
        let folder_only = SharpnessSettings {
            neighbours: 0,
            ..settings
        };
        assert!(!judge(&ordered, &folder_only)[&p(3)].blurry);
        // Filtre : les floues partent, les photos sans jugement restent.
        use crate::media::{MediaFile, MediaKind};
        let groups = vec![
            (0..12)
                .chain([99])
                .map(|i| MediaFile {
                    path: p(i),
                    kind: MediaKind::Flat,
                })
                .collect(),
        ];
        let kept = BlurFilter {
            judgements: &j,
            rule: BlurRule::HideBlurry,
        }
        .apply(groups.clone());
        assert_eq!(kept[0].len(), 13 - 5);
        // Seulement les floues : les 5, sans la photo sans jugement.
        let only = BlurFilter {
            judgements: &j,
            rule: BlurRule::OnlyBlurry,
        }
        .apply(groups);
        assert_eq!(only[0].len(), 5);
        assert!(only[0].iter().all(|f| j[&f.path].blurry));
    }

    #[test]
    fn exposure_does_not_change_the_score_and_a_blank_image_has_none() {
        let settings = SharpnessSettings::default();
        let full = (0, 0, 256, 192);
        let normal = scene(256, 192, 4, full, 0);
        // Même scène quatre fois plus sombre (sous-exposée).
        let dark = Bitmap {
            data: normal.data.iter().map(|v| v / 4).collect(),
            ..normal.clone()
        };
        let (a, b) = (
            score(&normal, &settings).value,
            score(&dark, &settings).value,
        );
        assert!((b / a - 1.0).abs() < 0.15, "{a} / {b}");
        let blank = Bitmap {
            data: vec![250; normal.data.len()],
            ..normal
        };
        assert_eq!(score(&blank, &settings).value, 0.0);
    }
}
