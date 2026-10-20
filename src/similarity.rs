//! Regroupement des images semblables (rafales, séries d'une même scène), comme les piles
//! automatiques de Capture One (décidé le 18/10/2026, voir `doc/similarity.md`).
//!
//! Chaque image reçoit une **empreinte** ([`Signature`]), calculée une fois sur une petite
//! version de l'image et gardée dans le cache d'analyse : la **composition** (empreinte
//! par différences de luminance, « dHash », 64 bits) et la **répartition des couleurs**
//! (couleur moyenne de 4 × 4 cases), plus l'heure de prise de vue. Deux images voisines
//! (ordre de la grille) sont dans le même groupe si leur **distance** reste sous un seuil,
//! et si elles ont été prises à peu d'intervalle. Les distances entre voisines sont
//! calculées une fois ([`distances`]) : regrouper ensuite ([`group`]) n'est qu'un parcours
//! de la liste, instantané même sur des milliers d'images (curseur du seuil en direct).

use serde::Deserialize;

use crate::bitmap::Bitmap;

/// Version du calcul de l'empreinte (dans la clé du cache : la changer recalcule tout).
pub const VERSION: u32 = 1;

/// Côté de la grille de couleurs.
const GRID: usize = 4;

/// Réglages (section `[similarity]` du fichier de configuration).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SimilaritySettings {
    /// Empreintes de tout le dossier calculées en fond (sinon pas de regroupement).
    pub background: bool,
    /// Côté long de l'image d'où l'empreinte est tirée (pixels) : petite, l'empreinte ne
    /// regarde que la composition, pas les détails.
    pub side: u32,
    /// Poids de la composition dans la distance (0 à 1) ; le reste : les couleurs.
    pub shape_weight: f32,
    /// Seuil par défaut (distance de 0 à 1) : deux images voisines plus proches sont dans
    /// le même groupe. Ajustable par dossier dans la fenêtre de regroupement.
    pub threshold: f32,
    /// Écart de temps de prise de vue au-delà duquel deux images voisines ne sont jamais
    /// dans le même groupe (secondes ; 0 : sans limite).
    pub max_gap_s: f32,
}

impl Default for SimilaritySettings {
    fn default() -> Self {
        Self {
            background: true,
            side: 64,
            shape_weight: 0.5,
            threshold: 0.25,
            max_gap_s: 60.0,
        }
    }
}

/// Empreinte d'une image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// Composition : un bit par paire de cases voisines (9 × 8 cases de luminance), vrai
    /// si la case de gauche est plus claire.
    pub shape: u64,
    /// Couleur moyenne de 4 × 4 cases, ligne par ligne (R, G, B).
    pub colors: [u8; GRID * GRID * 3],
    /// Heure de prise de vue (secondes, origine arbitraire : seuls les écarts comptent).
    pub time: Option<i64>,
}

/// Moyenne de la zone (x0..x1, y0..y1) de `b`, par canal.
fn mean(b: &Bitmap, (x0, x1): (usize, usize), (y0, y1): (usize, usize)) -> [f32; 3] {
    let w = b.width as usize;
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for y in y0..y1.max(y0 + 1) {
        for x in x0..x1.max(x0 + 1) {
            let p = &b.data[(y * w + x) * 3..][..3];
            for c in 0..3 {
                sum[c] += p[c] as u64;
            }
            n += 1;
        }
    }
    let n = n.max(1) as f32;
    [sum[0] as f32 / n, sum[1] as f32 / n, sum[2] as f32 / n]
}

/// Bornes de la case `i` sur `cells` cases d'une longueur `len`.
fn span(i: usize, cells: usize, len: usize) -> (usize, usize) {
    let a = i * len / cells;
    let b = ((i + 1) * len / cells).max(a + 1).min(len.max(1));
    (a.min(len.saturating_sub(1)), b)
}

impl Signature {
    /// Empreinte d'une image (déjà petite, voir [`SimilaritySettings::side`]) prise à
    /// `time`.
    pub fn of(b: &Bitmap, time: Option<i64>) -> Signature {
        let (w, h) = (b.width as usize, b.height as usize);
        let mut colors = [0u8; GRID * GRID * 3];
        if w == 0 || h == 0 {
            return Signature {
                shape: 0,
                colors,
                time,
            };
        }
        // Composition : 9 × 8 cases de luminance, comparées à leur voisine de droite.
        let mut shape = 0u64;
        for y in 0..8 {
            let row: Vec<f32> = (0..9)
                .map(|x| {
                    let [r, g, bl] = mean(b, span(x, 9, w), span(y, 8, h));
                    0.2126 * r + 0.7152 * g + 0.0722 * bl
                })
                .collect();
            for x in 0..8 {
                shape = (shape << 1) | u64::from(row[x] > row[x + 1]);
            }
        }
        for y in 0..GRID {
            for x in 0..GRID {
                let m = mean(b, span(x, GRID, w), span(y, GRID, h));
                for c in 0..3 {
                    colors[(y * GRID + x) * 3 + c] = m[c].round() as u8;
                }
            }
        }
        Signature {
            shape,
            colors,
            time,
        }
    }

    /// Distance à `other`, de 0 (identiques) à 1 : composition (bits différents) et
    /// couleurs (écart moyen), pondérées par `shape_weight`.
    pub fn distance(&self, other: &Signature, shape_weight: f32) -> f32 {
        let shape = (self.shape ^ other.shape).count_ones() as f32 / 64.0;
        let colors = self
            .colors
            .iter()
            .zip(&other.colors)
            .map(|(a, b)| a.abs_diff(*b) as f32)
            .sum::<f32>()
            / (self.colors.len() as f32 * 255.0);
        let w = shape_weight.clamp(0.0, 1.0);
        // Couleurs : écart moyen de 10 % déjà très visible, ramené vers le haut de
        // l'échelle pour peser autant que la composition.
        w * shape + (1.0 - w) * (colors * 4.0).min(1.0)
    }

    /// Format binaire du cache d'analyse : version, composition, couleurs, heure.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 8 + self.colors.len() + 9);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.shape.to_le_bytes());
        out.extend_from_slice(&self.colors);
        match self.time {
            Some(t) => {
                out.push(1);
                out.extend_from_slice(&t.to_le_bytes());
            }
            None => out.push(0),
        }
        out
    }

    /// Relit [`Signature::to_bytes`] ; `None` si l'entrée est illisible ou d'une autre
    /// version.
    pub fn from_bytes(bytes: &[u8]) -> Option<Signature> {
        let n = GRID * GRID * 3;
        let version = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
        if version != VERSION {
            return None;
        }
        let shape = u64::from_le_bytes(bytes.get(4..12)?.try_into().ok()?);
        let colors: [u8; GRID * GRID * 3] = bytes.get(12..12 + n)?.try_into().ok()?;
        let time = match bytes.get(12 + n)? {
            1 => Some(i64::from_le_bytes(
                bytes.get(13 + n..21 + n)?.try_into().ok()?,
            )),
            _ => None,
        };
        Some(Signature {
            shape,
            colors,
            time,
        })
    }
}

/// Heure « AAAA:MM:JJ HH:MM:SS » (EXIF, LibRaw ; tout séparateur) en secondes depuis
/// 1970, sans fuseau : seuls les écarts entre images servent.
pub fn parse_time(text: &str) -> Option<i64> {
    let digits: Vec<i64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [y, mo, d, h, mi, s] = digits.get(..6)?.try_into().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // Jours depuis 1970 (algorithme de H. Hinnant, days_from_civil).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// Lien entre chaque image et la suivante : distance des empreintes et écart de temps
/// (`None` si une empreinte manque : pas encore calculée).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    /// Distance des empreintes (0 à 1).
    pub distance: f32,
    /// Écart de temps de prise de vue (secondes), si les deux heures sont connues.
    pub gap_s: Option<f32>,
}

/// Liens entre images voisines de `signatures` (dans l'ordre de la grille) : `n - 1`
/// liens, `None` là où une empreinte manque.
pub fn distances(signatures: &[Option<&Signature>], shape_weight: f32) -> Vec<Option<Link>> {
    signatures
        .windows(2)
        .map(|w| match (w[0], w[1]) {
            (Some(a), Some(b)) => Some(Link {
                distance: a.distance(b, shape_weight),
                gap_s: a.time.zip(b.time).map(|(x, y)| (y - x).abs() as f32),
            }),
            _ => None,
        })
        .collect()
}

/// Groupes d'images voisines : rangs de début et de fin (exclue) de chaque groupe, dans
/// l'ordre. Une image rejoint la précédente si leur lien est sous `threshold` et leur
/// écart de temps sous `max_gap_s` (0 : sans limite) ; un lien inconnu coupe.
pub fn group(
    links: &[Option<Link>],
    threshold: f32,
    max_gap_s: f32,
) -> Vec<std::ops::Range<usize>> {
    let n = links.len() + 1;
    let mut groups = Vec::new();
    let mut start = 0;
    for (i, link) in links.iter().enumerate() {
        let joined = link.is_some_and(|l| {
            l.distance <= threshold && (max_gap_s <= 0.0 || l.gap_s.is_none_or(|g| g <= max_gap_s))
        });
        if !joined {
            groups.push(start..i + 1);
            start = i + 1;
        }
    }
    groups.push(start..n);
    groups
}

/// Fichier des seuils par dossier, dans le dossier de données.
const THRESHOLDS: &str = "group-thresholds.tsv";

/// Seuil retenu pour le dossier `dir` (fenêtre de regroupement), s'il y en a un.
pub fn folder_threshold(data: &std::path::Path, dir: &std::path::Path) -> Option<f32> {
    crate::folder::folder_value(data, THRESHOLDS, dir)
}

/// Retient `threshold` pour le dossier `dir` ; `None` : l'oublie (seuil par défaut).
pub fn remember_threshold(
    data: &std::path::Path,
    dir: &std::path::Path,
    threshold: Option<f32>,
) -> std::io::Result<()> {
    crate::folder::remember_folder_value(data, THRESHOLDS, dir, threshold)
}

/// Fichier des couvertures choisies (une image par ligne), dans le dossier de données.
const COVERS: &str = "stack-covers.txt";

/// Couvertures choisies par l'utilisateur (la préférée de chaque pile), toutes dossiers
/// confondus ; séparées des notes, qui restent dans les sidecars.
pub fn chosen_covers(data: &std::path::Path) -> std::collections::HashSet<std::path::PathBuf> {
    std::fs::read_to_string(data.join(COVERS))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

/// Retient (`on`) ou oublie `image` comme couverture choisie ; `others` (les autres
/// images de sa pile) sont oubliées : une seule préférée par pile.
pub fn choose_cover(
    data: &std::path::Path,
    image: &std::path::Path,
    others: &[std::path::PathBuf],
    on: bool,
) -> std::io::Result<()> {
    let mut covers = chosen_covers(data);
    for o in others {
        covers.remove(o);
    }
    if on {
        covers.insert(image.to_path_buf());
    } else {
        covers.remove(image);
    }
    let mut lines: Vec<String> = covers.iter().map(|p| p.display().to_string()).collect();
    lines.sort();
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join(COVERS), lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Image de 32 × 24 : dégradé horizontal (`flip` : inversé), teinte `tint`.
    fn image(flip: bool, tint: [u8; 3]) -> Bitmap {
        let (w, h) = (32u32, 24u32);
        let mut data = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                let v = if flip { w - 1 - x } else { x } * 255 / (w - 1);
                for c in tint {
                    data.push(((v * c as u32) / 255) as u8);
                }
            }
        }
        Bitmap {
            width: w,
            height: h,
            data,
        }
    }

    #[test]
    fn similar_images_are_close_and_different_ones_far() {
        let a = Signature::of(&image(false, [255, 255, 255]), None);
        let b = Signature::of(&image(false, [250, 250, 255]), None);
        let c = Signature::of(&image(true, [255, 120, 40]), None);
        assert!(a.distance(&b, 0.5) < 0.05, "{}", a.distance(&b, 0.5));
        assert!(a.distance(&c, 0.5) > 0.5, "{}", a.distance(&c, 0.5));
        assert_eq!(a.distance(&a, 0.5), 0.0);
    }

    #[test]
    fn groups_cut_on_distance_time_and_unknowns() {
        let link = |d, g| {
            Some(Link {
                distance: d,
                gap_s: g,
            })
        };
        // 0-1-2 proches, 2|3 loin, 3-4 proches mais 90 s d'écart, 4|5 inconnu.
        let links = [
            link(0.1, Some(1.0)),
            link(0.1, Some(1.0)),
            link(0.6, Some(1.0)),
            link(0.1, Some(90.0)),
            None,
        ];
        assert_eq!(group(&links, 0.25, 60.0), [0..3, 3..4, 4..5, 5..6]);
        assert_eq!(
            group(&links, 0.25, 0.0),
            [0..3, 3..5, 5..6],
            "sans limite de temps"
        );
        assert_eq!(group(&links, 1.0, 0.0), [0..5, 5..6]);
        assert_eq!(group(&[], 0.25, 60.0), vec![0..1], "une seule image");
    }

    #[test]
    fn one_chosen_cover_per_stack() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let p = |n: &str| std::path::PathBuf::from(format!("/photos/{n}"));
        choose_cover(data, &p("a1"), &[p("a2")], true).unwrap();
        choose_cover(data, &p("b2"), &[p("b1")], true).unwrap();
        choose_cover(data, &p("a2"), &[p("a1")], true).unwrap();
        let covers = chosen_covers(data);
        assert!(covers.contains(&p("a2")) && !covers.contains(&p("a1")));
        assert!(covers.contains(&p("b2")));
        choose_cover(data, &p("b2"), &[], false).unwrap();
        assert!(!chosen_covers(data).contains(&p("b2")));
    }

    #[test]
    fn thresholds_are_remembered_per_folder() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let (a, b) = (
            std::path::Path::new("/photos/a"),
            std::path::Path::new("/photos/b"),
        );
        assert_eq!(folder_threshold(data, a), None);
        remember_threshold(data, a, Some(0.3)).unwrap();
        remember_threshold(data, b, Some(0.1)).unwrap();
        remember_threshold(data, a, Some(0.35)).unwrap();
        assert_eq!(folder_threshold(data, a), Some(0.35));
        assert_eq!(folder_threshold(data, b), Some(0.1));
        remember_threshold(data, a, None).unwrap();
        assert_eq!(folder_threshold(data, a), None, "oublié : seuil par défaut");
        assert_eq!(folder_threshold(data, b), Some(0.1));
    }

    #[test]
    fn signatures_round_trip_and_times_parse() {
        let s = Signature::of(
            &image(true, [10, 200, 30]),
            parse_time("2026:10:17 16:56:51"),
        );
        assert_eq!(Signature::from_bytes(&s.to_bytes()), Some(s.clone()));
        assert_eq!(Signature::from_bytes(&s.to_bytes()[..10]), None);
        let a = parse_time("2026:10:17 16:56:51").unwrap();
        let b = parse_time("2026-10-18T00:00:00").unwrap();
        assert_eq!(b - a, 7 * 3600 + 3 * 60 + 9);
        assert_eq!(parse_time("1970:01:01 00:00:00"), Some(0));
        assert_eq!(parse_time("pas une date"), None);
    }
}
