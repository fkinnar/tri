//! Exposition d'une image : histogrammes sur les vraies données, pixels saturés et
//! bouchés, masque des zones concernées.
//!
//! Pour un RAW, tout est calculé sur les **données du capteur** ([`crate::raw::Sensor`]),
//! à leur profondeur (12, 14, 16 bits), noir soustrait pixel par pixel, sans dématriçage :
//! c'est la vérité sur l'exposition, quel que soit l'aperçu affiché (JPEG du boîtier ou
//! rendu). Pour une image plate, sur ses valeurs 8 bits. C'est une donnée *dérivée*
//! (cache d'analyse), jamais écrite dans le XMP. Voir `doc/exposure.md`.

use std::path::Path;

use serde::Deserialize;

use crate::bitmap::{Bitmap, Rotation};
use crate::raw::{RawFile, Sensor};

/// Canaux : rouge, vert, bleu.
pub const CHANNELS: usize = 3;

/// Version de l'algorithme, dans la clé du cache d'analyse : à incrémenter quand le calcul
/// change sans que les réglages changent (2 : zones bouchées à la majorité des pixels).
pub const VERSION: u32 = 2;

/// Réglages du calcul (section `[exposure]` du fichier de configuration).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExposureSettings {
    /// Seuil de saturation, en IL sous le blanc du capteur : 0 = seulement les pixels au
    /// blanc ; -0,1 = aussi ceux à moins de 0,1 IL (boîtiers qui saturent un peu sous leur
    /// blanc théorique).
    pub over_ev: f32,
    /// Seuil des ombres bouchées, en IL sous le blanc : -10 = moins de 1/1024 de la plage
    /// (bruit sur un capteur 14 bits).
    pub under_ev: f32,
    /// Nombre de cases de l'histogramme linéaire.
    pub linear_bins: u32,
    /// Étendue de l'histogramme en IL, sous la saturation (tout ce qui est plus sombre va
    /// dans la première case).
    pub ev_range: f32,
    /// Cases par IL de l'histogramme en IL.
    pub ev_bins_per_stop: u32,
    /// Côté long du masque des zones (cases) ; chaque case couvre plusieurs pixels.
    pub mask_size: u32,
    /// Part des pixels d'une case qui doivent être bouchés pour la marquer (0 à 1). Pas
    /// « tous » : près du noir, le bruit du capteur en remonte presque toujours un.
    pub mask_under_share: f32,
}

impl Default for ExposureSettings {
    fn default() -> Self {
        Self {
            over_ev: 0.0,
            under_ev: -10.0,
            linear_bins: 1024,
            ev_range: 16.0,
            ev_bins_per_stop: 8,
            mask_size: 1024,
            mask_under_share: 0.5,
        }
    }
}

/// D'où viennent les valeurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Données du capteur d'un RAW.
    Raw {
        /// Profondeur utile (bits).
        bits: u32,
        /// Plus petit niveau de noir (valeur brute).
        black: u16,
        /// Blanc du capteur (valeur brute).
        white: u16,
    },
    /// Valeurs 8 bits d'une image plate.
    Flat,
}

/// Bits d'une case du masque : saturé dans le canal 0, 1, 2 ; bouché (tous canaux).
pub mod mask {
    /// Rouge saturé.
    pub const OVER_R: u8 = 1;
    /// Vert saturé.
    pub const OVER_G: u8 = 2;
    /// Bleu saturé.
    pub const OVER_B: u8 = 4;
    /// Un canal au moins saturé.
    pub const OVER: u8 = OVER_R | OVER_G | OVER_B;
    /// Bouché : au moins `mask_under_share` des pixels de la case sous le seuil des
    /// ombres.
    pub const UNDER: u8 = 8;
}

/// Masque des zones saturées / bouchées, en cases, déjà dans l'orientation de l'image
/// affichée (rotation du boîtier appliquée).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    /// Largeur (cases).
    pub width: u32,
    /// Hauteur (cases).
    pub height: u32,
    /// Une case par octet, ligne par ligne : bits de [`mask`].
    pub cells: Vec<u8>,
}

impl Mask {
    /// Case (`x`, `y`).
    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.cells[(y * self.width + x) as usize]
    }

    /// Tourne le masque dans le sens horaire (comme [`Bitmap::rotate`]).
    pub fn rotate(self, rotation: Rotation) -> Mask {
        let (w, h) = (self.width as usize, self.height as usize);
        let src = |x: usize, y: usize| self.cells[y * w + x];
        let (nw, nh, cells): (usize, usize, Vec<u8>) = match rotation {
            Rotation::None => return self,
            Rotation::Cw180 => (w, h, self.cells.iter().rev().copied().collect()),
            Rotation::Cw90 => (
                h,
                w,
                (0..w)
                    .flat_map(|y| (0..h).map(move |x| (x, y)))
                    .map(|(x, y)| src(y, h - 1 - x))
                    .collect(),
            ),
            Rotation::Cw270 => (
                h,
                w,
                (0..w)
                    .flat_map(|y| (0..h).map(move |x| (x, y)))
                    .map(|(x, y)| src(w - 1 - y, x))
                    .collect(),
            ),
        };
        Mask {
            width: nw as u32,
            height: nh as u32,
            cells,
        }
    }
}

/// Exposition d'une image.
#[derive(Clone, Debug, PartialEq)]
pub struct Exposure {
    /// Origine des valeurs.
    pub source: Source,
    /// Plage utile : du noir (0) au blanc (`range`), noir soustrait.
    pub range: u32,
    /// Histogramme linéaire par canal, de 0 à `range`.
    pub linear: [Vec<u32>; CHANNELS],
    /// Histogramme en IL par canal, de `-ev_range` à 0 (saturation), `ev_bins_per_stop`
    /// cases par IL ; ce qui est plus sombre (et le noir pur) est dans la première case.
    pub ev: [Vec<u32>; CHANNELS],
    /// Étendue de l'histogramme en IL.
    pub ev_range: f32,
    /// Pixels par canal.
    pub total: [u64; CHANNELS],
    /// Pixels saturés par canal.
    pub over: [u64; CHANNELS],
    /// Pixels bouchés par canal.
    pub under: [u64; CHANNELS],
    /// Valeur exacte au 99,9e centile par canal (noir soustrait) : les hautes lumières
    /// utiles, sans quelques pixels isolés (reflets, pixels chauds).
    pub high: [u32; CHANNELS],
    /// Zones saturées / bouchées.
    pub mask: Mask,
}

/// Erreurs du calcul.
#[derive(Debug)]
pub enum ExposureError {
    /// Lecture du RAW (fichier, LibRaw, RAW sans mosaïque).
    Raw(crate::raw::RawError),
    /// Données d'un cache illisibles (format inconnu ou tronqué).
    Format,
}

impl std::fmt::Display for ExposureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(e) => write!(f, "{e}"),
            Self::Format => write!(f, "données d'exposition illisibles"),
        }
    }
}

impl std::error::Error for ExposureError {}

impl From<crate::raw::RawError> for ExposureError {
    fn from(e: crate::raw::RawError) -> Self {
        Self::Raw(e)
    }
}

/// Accumulation en un seul passage : histogramme complet (une case par valeur) et
/// masque ; tout le reste en est dérivé.
struct Accumulator {
    range: u32,
    over_level: u32,
    full: [Vec<u32>; CHANNELS],
    mask_w: usize,
    mask_h: usize,
    over_cells: Vec<u8>,
    under_level: u32,
    /// Par case : pixels bouchés, et pixels en tout.
    cell_under: Vec<u32>,
    cell_total: Vec<u32>,
}

impl Accumulator {
    fn new(range: u32, width: usize, height: usize, settings: &ExposureSettings) -> Self {
        let long = settings.mask_size.max(1) as usize;
        let (mask_w, mask_h) = if width >= height {
            (
                long.min(width),
                (long * height / width.max(1)).clamp(1, height.max(1)),
            )
        } else {
            (
                (long * width / height.max(1)).clamp(1, width.max(1)),
                long.min(height),
            )
        };
        let full = std::array::from_fn(|_| vec![0u32; range as usize + 1]);
        Accumulator {
            range,
            over_level: level(range, settings.over_ev).max(1),
            full,
            mask_w,
            mask_h,
            over_cells: vec![0; mask_w * mask_h],
            under_level: level(range, settings.under_ev),
            cell_under: vec![0; mask_w * mask_h],
            cell_total: vec![0; mask_w * mask_h],
        }
    }

    /// Case du masque pour chaque colonne de l'image.
    fn column_cells(&self, width: usize) -> Vec<usize> {
        (0..width).map(|col| col * self.mask_w / width).collect()
    }

    #[inline]
    fn add(&mut self, channel: usize, value: u32, cell: usize) {
        let v = value.min(self.range);
        self.full[channel][v as usize] += 1;
        if v >= self.over_level {
            self.over_cells[cell] |= 1 << channel;
        }
        self.cell_total[cell] += 1;
        if v <= self.under_level {
            self.cell_under[cell] += 1;
        }
    }

    fn finish(self, source: Source, rotation: Rotation, settings: &ExposureSettings) -> Exposure {
        let range = self.range;
        let under_level = self.under_level;
        let lin_bins = settings.linear_bins.max(1) as usize;
        let per_stop = settings.ev_bins_per_stop.max(1);
        let ev_bins = ((settings.ev_range.max(1.0) * per_stop as f32).ceil() as usize).max(1);
        let mut linear: [Vec<u32>; CHANNELS] = std::array::from_fn(|_| vec![0; lin_bins]);
        let mut ev: [Vec<u32>; CHANNELS] = std::array::from_fn(|_| vec![0; ev_bins]);
        let (mut total, mut over, mut under) = ([0u64; 3], [0u64; 3], [0u64; 3]);
        let mut high = [0u32; CHANNELS];
        // Case en IL de chaque valeur : une seule fois par valeur, pas par pixel.
        let ev_bin = |v: u32| -> usize {
            if v == 0 {
                return 0;
            }
            let stops = (v as f32 / range as f32).log2() + settings.ev_range;
            ((stops * per_stop as f32).floor().max(0.0) as usize).min(ev_bins - 1)
        };
        for c in 0..CHANNELS {
            for (v, &n) in self.full[c].iter().enumerate() {
                if n == 0 {
                    continue;
                }
                let v32 = v as u32;
                linear[c][(v * lin_bins / (range as usize + 1)).min(lin_bins - 1)] += n;
                ev[c][ev_bin(v32)] += n;
                total[c] += n as u64;
                if v32 >= self.over_level {
                    over[c] += n as u64;
                }
                if v32 <= under_level {
                    under[c] += n as u64;
                }
            }
            let target = (total[c] as f64 * 0.999).ceil() as u64;
            let mut seen = 0u64;
            for (v, &n) in self.full[c].iter().enumerate() {
                seen += n as u64;
                if seen >= target && n > 0 {
                    high[c] = v as u32;
                    break;
                }
            }
        }
        let share = settings.mask_under_share.clamp(0.0, 1.0) as f64;
        let cells = self
            .over_cells
            .iter()
            .zip(self.cell_under.iter().zip(&self.cell_total))
            .map(|(&o, (&u, &t))| {
                let under = t > 0 && u as f64 >= share * t as f64;
                o | if under { mask::UNDER } else { 0 }
            })
            .collect();
        let mask = Mask {
            width: self.mask_w as u32,
            height: self.mask_h as u32,
            cells,
        }
        .rotate(rotation);
        Exposure {
            source,
            range,
            linear,
            ev,
            ev_range: settings.ev_range,
            total,
            over,
            under,
            high,
            mask,
        }
    }
}

/// Valeur (noir soustrait) à `stops` IL sous `range` : `range × 2^stops`.
fn level(range: u32, stops: f32) -> u32 {
    (range as f64 * 2f64.powf(stops as f64)).floor() as u32
}

impl Exposure {
    /// Exposition d'un RAW, sur les données du capteur. Décompresse le fichier
    /// (`unpack`, ~0,4 s sur un RAF de 26 Mpx) : à appeler hors du fil de l'interface.
    pub fn from_raw(path: &Path, settings: &ExposureSettings) -> Result<Exposure, ExposureError> {
        let mut raw = RawFile::open(path)?;
        let rotation = raw.orientation();
        let sensor = raw.sensor()?;
        Ok(Exposure::from_sensor(&sensor, rotation, settings))
    }

    /// Exposition calculée sur des données capteur ; `rotation` : orientation de l'image
    /// affichée, appliquée au masque.
    pub fn from_sensor(s: &Sensor, rotation: Rotation, settings: &ExposureSettings) -> Exposure {
        let black = s.min_black();
        let range = s.white.saturating_sub(black).max(1) as u32;
        let mut acc = Accumulator::new(range, s.width, s.height, settings);
        let cols = acc.column_cells(s.width);
        for row in 0..s.height {
            let base = row * acc.mask_h / s.height * acc.mask_w;
            let (channels, blacks) = s.pattern(row);
            for (col, &v) in s.row(row).iter().enumerate() {
                let i = col % channels.len();
                let value = v.saturating_sub(blacks[i]) as u32;
                acc.add(channels[i] as usize, value, base + cols[col]);
            }
        }
        let source = Source::Raw {
            bits: s.bits(),
            black,
            white: s.white,
        };
        acc.finish(source, rotation, settings)
    }

    /// Exposition d'une image plate (RGB 8 bits, déjà dans son orientation).
    pub fn from_bitmap(b: &Bitmap, settings: &ExposureSettings) -> Exposure {
        let (w, h) = (b.width as usize, b.height as usize);
        let mut acc = Accumulator::new(255, w, h, settings);
        let cols = acc.column_cells(w);
        for (row, line) in b.data.chunks_exact(w * 3).enumerate() {
            let base = row * acc.mask_h / h * acc.mask_w;
            for (col, px) in line.as_chunks::<3>().0.iter().enumerate() {
                for (c, &v) in px.iter().enumerate() {
                    acc.add(c, v as u32, base + cols[col]);
                }
            }
        }
        acc.finish(Source::Flat, Rotation::None, settings)
    }

    /// Part des pixels saturés dans le canal le plus touché (0 à 1).
    pub fn over_fraction(&self) -> f64 {
        fraction(&self.over, &self.total)
    }

    /// Part des pixels bouchés dans le canal le plus touché (0 à 1).
    pub fn under_fraction(&self) -> f64 {
        fraction(&self.under, &self.total)
    }

    /// Marge avant saturation, en IL, du canal le plus exposé : position de son 99,9e
    /// centile sous le blanc (0 : déjà au blanc ; indication « exposer à droite »).
    pub fn headroom_ev(&self) -> f32 {
        let top = self.high.iter().copied().max().unwrap_or(0);
        if top == 0 {
            return -self.ev_range;
        }
        ((top as f64 / self.range as f64).log2() as f32).clamp(-self.ev_range, 0.0)
    }
}

fn fraction(counts: &[u64; CHANNELS], total: &[u64; CHANNELS]) -> f64 {
    (0..CHANNELS)
        .filter(|&c| total[c] > 0)
        .map(|c| counts[c] as f64 / total[c] as f64)
        .fold(0.0, f64::max)
}

// Format binaire du cache d'analyse : petit-boutiste, entête versionné, masque compressé
// par plages (il est presque toujours vide). Changer le format = changer `MAGIC`.
const MAGIC: &[u8; 8] = b"TRIEXP01";

impl Exposure {
    /// Sérialise pour le cache d'analyse (quelques Ko).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        let (kind, bits, black, white) = match self.source {
            Source::Raw { bits, black, white } => (1u8, bits, black, white),
            Source::Flat => (0u8, 8, 0, 255),
        };
        out.push(kind);
        put_u32(&mut out, bits);
        put_u32(&mut out, black as u32);
        put_u32(&mut out, white as u32);
        put_u32(&mut out, self.range);
        out.extend_from_slice(&self.ev_range.to_le_bytes());
        for c in 0..CHANNELS {
            put_vec(&mut out, &self.linear[c]);
            put_vec(&mut out, &self.ev[c]);
            for n in [self.total[c], self.over[c], self.under[c]] {
                out.extend_from_slice(&n.to_le_bytes());
            }
            put_u32(&mut out, self.high[c]);
        }
        put_u32(&mut out, self.mask.width);
        put_u32(&mut out, self.mask.height);
        let mut runs: Vec<(u8, u32)> = Vec::new();
        for &cell in &self.mask.cells {
            match runs.last_mut() {
                Some((value, n)) if *value == cell => *n += 1,
                _ => runs.push((cell, 1)),
            }
        }
        put_u32(&mut out, runs.len() as u32);
        for (value, n) in runs {
            out.push(value);
            put_u32(&mut out, n);
        }
        out
    }

    /// Relit des données de [`Exposure::to_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Exposure, ExposureError> {
        let mut r = Reader(bytes);
        if r.take(MAGIC.len())? != MAGIC {
            return Err(ExposureError::Format);
        }
        let kind = r.take(1)?[0];
        let bits = r.u32()?;
        let black = r.u32()? as u16;
        let white = r.u32()? as u16;
        let source = match kind {
            1 => Source::Raw { bits, black, white },
            _ => Source::Flat,
        };
        let range = r.u32()?;
        let ev_range = f32::from_le_bytes(r.take(4)?.try_into().unwrap());
        let mut linear: [Vec<u32>; CHANNELS] = Default::default();
        let mut ev: [Vec<u32>; CHANNELS] = Default::default();
        let (mut total, mut over, mut under) = ([0u64; 3], [0u64; 3], [0u64; 3]);
        let mut high = [0u32; CHANNELS];
        for c in 0..CHANNELS {
            linear[c] = r.vec()?;
            ev[c] = r.vec()?;
            total[c] = r.u64()?;
            over[c] = r.u64()?;
            under[c] = r.u64()?;
            high[c] = r.u32()?;
        }
        let width = r.u32()?;
        let height = r.u32()?;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for _ in 0..r.u32()? {
            let value = r.take(1)?[0];
            let n = r.u32()? as usize;
            cells.extend(std::iter::repeat_n(value, n));
        }
        if cells.len() != (width * height) as usize {
            return Err(ExposureError::Format);
        }
        Ok(Exposure {
            source,
            range,
            linear,
            ev,
            ev_range,
            total,
            over,
            under,
            high,
            mask: Mask {
                width,
                height,
                cells,
            },
        })
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_vec(out: &mut Vec<u8>, v: &[u32]) {
    put_u32(out, v.len() as u32);
    for n in v {
        put_u32(out, *n);
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ExposureError> {
        if self.0.len() < n {
            return Err(ExposureError::Format);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, ExposureError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, ExposureError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn vec(&mut self) -> Result<Vec<u32>, ExposureError> {
        let n = self.u32()? as usize;
        if self.0.len() < n * 4 {
            return Err(ExposureError::Format);
        }
        (0..n).map(|_| self.u32()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> ExposureSettings {
        ExposureSettings {
            mask_size: 4,
            ..ExposureSettings::default()
        }
    }

    /// Image 4 × 2 : en haut à gauche blanc pur (saturé), en bas à droite noir (bouché).
    fn sample() -> Bitmap {
        let mut data = vec![128u8; 4 * 2 * 3];
        data[..3].copy_from_slice(&[255, 255, 255]);
        let last = data.len() - 3;
        data[last..].copy_from_slice(&[0, 0, 0]);
        Bitmap {
            width: 4,
            height: 2,
            data,
        }
    }

    #[test]
    fn flat_image_counts_and_mask() {
        let e = Exposure::from_bitmap(&sample(), &settings());
        assert_eq!(e.source, Source::Flat);
        assert_eq!(e.range, 255);
        assert_eq!(e.total, [8, 8, 8]);
        assert_eq!(e.over, [1, 1, 1]);
        assert_eq!(e.under, [1, 1, 1], "-10 IL sous 255 : seul 0 est bouché");
        assert_eq!((e.mask.width, e.mask.height), (4, 2));
        assert_eq!(e.mask.get(0, 0), mask::OVER);
        assert_eq!(e.mask.get(3, 1), mask::UNDER);
        assert_eq!(e.mask.get(1, 0), 0);
        assert!((e.over_fraction() - 0.125).abs() < 1e-9);
        // Histogrammes : chaque canal compte tous ses pixels.
        for c in 0..CHANNELS {
            assert_eq!(e.linear[c].iter().map(|&n| n as u64).sum::<u64>(), 8);
            assert_eq!(e.ev[c].iter().map(|&n| n as u64).sum::<u64>(), 8);
        }
        // 128 / 255 = -0,994 IL : juste au-dessus de -1 IL, dans la case qui commence à
        // -1 IL (la première de la dernière IL).
        let per_stop = 8;
        let bins = e.ev[0].len();
        assert_eq!(bins, 16 * per_stop);
        let mid = e.ev[0].iter().position(|&n| n == 6).unwrap();
        assert_eq!(mid, bins - per_stop);
        assert_eq!(e.ev[0][bins - 1], 1, "le blanc dans la dernière case");
        assert_eq!(e.ev[0][0], 1, "le noir dans la première");
    }

    #[test]
    fn over_threshold_in_stops() {
        let loose = ExposureSettings {
            over_ev: -1.0,
            ..settings()
        };
        // 128 ≥ 255 × 2^-1 = 127 : saturé avec une marge d'un IL.
        assert_eq!(Exposure::from_bitmap(&sample(), &loose).over, [7, 7, 7]);
    }

    #[test]
    fn mask_rotations() {
        // 2 × 1 : [A, B]
        let m = Mask {
            width: 2,
            height: 1,
            cells: vec![1, 2],
        };
        let cw = m.clone().rotate(Rotation::Cw90);
        assert_eq!((cw.width, cw.height, cw.cells.clone()), (1, 2, vec![1, 2]));
        let ccw = m.clone().rotate(Rotation::Cw270);
        assert_eq!((ccw.width, ccw.height, ccw.cells), (1, 2, vec![2, 1]));
        assert_eq!(m.clone().rotate(Rotation::Cw180).cells, vec![2, 1]);
        // Une rotation de 90° puis 270° revient au départ, sur un masque non carré.
        let big = Mask {
            width: 3,
            height: 2,
            cells: (0..6).collect(),
        };
        assert_eq!(
            big.clone().rotate(Rotation::Cw90).rotate(Rotation::Cw270),
            big
        );
        // Coin haut gauche après 90° horaire : l'ancien coin bas gauche.
        assert_eq!(big.clone().rotate(Rotation::Cw90).get(0, 0), 3);
    }

    #[test]
    fn bytes_round_trip_and_reject_garbage() {
        let e = Exposure::from_bitmap(&sample(), &settings());
        let bytes = e.to_bytes();
        assert_eq!(Exposure::from_bytes(&bytes).unwrap(), e);
        assert!(Exposure::from_bytes(b"pas un cache").is_err());
        assert!(
            Exposure::from_bytes(&bytes[..bytes.len() - 3]).is_err(),
            "tronqué"
        );
    }

    #[test]
    fn headroom_of_a_mid_grey_image() {
        let grey = Bitmap {
            width: 2,
            height: 2,
            data: vec![64; 12],
        };
        let e = Exposure::from_bitmap(&grey, &settings());
        // 64 / 255 : environ 2 IL sous le blanc.
        assert!((e.headroom_ev() + 2.0).abs() < 0.05, "{}", e.headroom_ev());
    }
}
