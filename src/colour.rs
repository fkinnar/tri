//! Gestion des couleurs (issue n° 422, décidée le 11/11/2026 avec l'utilisateur).
//!
//! Tout ce que `tri` montre (vignettes, aperçus, 1:1) passe par un **espace de travail**
//! unique : primaires **Rec.2020**, courbe **sRGB**, blanc D65. Large (il contient P3,
//! l'Adobe RGB et les écrans wide gamut qui le dépassent, l'Eizo de l'utilisateur
//! compris), indépendant de l'écran (le cache sert sur tous les écrans), et ses gris sont
//! ceux du sRGB (même courbe, même blanc) : l'interface d'egui, en sRGB, n'en change pas
//! de luminosité.
//!
//! - Chaque source est convertie **une fois, à la mise en cache**, depuis son vrai profil
//!   ([`Origin`] : ICC intégré, sinon EXIF, sinon sRGB) : JPEG boîtier, image plate (à sa
//!   profondeur : un TIFF 16 bits est converti en 16 bits). Le rendu RAW de LibRaw sort
//!   directement dans l'espace de travail ([`crate::raw::OutputSpace::WORKING`]).
//! - Les rendus des logiciels de développement sont gardés tels quels (avec leur ICC : le
//!   profil de sortie de l'historique, jamais imposé par `tri`) et convertis à la lecture
//!   ([`crate::cache::Cache::get`]).
//! - L'affichage convertit de l'espace de travail vers l'écran (macOS : déclaré au
//!   système ; ailleurs, par [`Transform`]).
//! - Les analyses (histogrammes, zones, netteté) restent sur les données réelles.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use moxcms::{
    CicpColorPrimaries, CicpProfile, ColorProfile, Layout, MatrixCoefficients, ProfileText,
    TransferCharacteristics, Transform8BitExecutor, Transform16BitExecutor, TransformOptions,
};

use crate::bitmap::Bitmap;

/// Version des conversions : entre dans la clé de cache des images (une règle changée :
/// les aperçus sont refaits).
pub const VERSION: u32 = 1;

/// Le profil de l'espace de travail (voir le module).
pub fn working() -> &'static ColorProfile {
    static WORKING: OnceLock<ColorProfile> = OnceLock::new();
    WORKING.get_or_init(|| {
        let mut p = ColorProfile::new_bt2020();
        let srgb = ColorProfile::new_srgb();
        p.red_trc = srgb.red_trc.clone();
        p.green_trc = srgb.green_trc.clone();
        p.blue_trc = srgb.blue_trc;
        p.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Bt2020,
            transfer_characteristics: TransferCharacteristics::Srgb,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        p.description = Some(ProfileText::PlainString(
            "tri working space (Rec.2020 primaries, sRGB curve)".into(),
        ));
        p
    })
}

/// Le profil ICC de l'espace de travail (déclaré au système par l'interface, intégré aux
/// fichiers écrits depuis le cache).
pub fn working_icc() -> &'static [u8] {
    static ICC: OnceLock<Vec<u8>> = OnceLock::new();
    ICC.get_or_init(|| working().encode().unwrap_or_default())
}

/// Poids de luminance de l'espace de travail, en lumière linéaire (ceux de Rec.2020 : les
/// primaires fixent la luminance).
pub const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

/// Passage linéaire Rec.2020 → Rec.709 (primaires sRGB), par lignes.
const REC2020_TO_709: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641, -0.072_850],
    [-0.124_550, 1.132_9, -0.008_349],
    [-0.018_151, -0.100_579, 1.118_73],
];

/// Passage linéaire Rec.709 → Rec.2020, par lignes (l'inverse du précédent).
const REC709_TO_2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_283, 0.043_313],
    [0.069_097, 0.919_540, 0.011_362],
    [0.016_391, 0.088_013, 0.895_595],
];

fn multiply(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// Un pixel de l'espace de travail en lumière linéaire, exprimé avec les primaires sRGB
/// (valeurs négatives ou au-delà de 1 hors du gamut sRGB) : les réglages de visualisation
/// (balance, mélangeur du noir et blanc) restent définis sur des canaux sRGB, comme avant
/// la gestion des couleurs.
pub fn working_to_srgb_linear(rgb: [f32; 3]) -> [f32; 3] {
    multiply(&REC2020_TO_709, rgb)
}

/// L'inverse de [`working_to_srgb_linear`].
pub fn srgb_linear_to_working(rgb: [f32; 3]) -> [f32; 3] {
    multiply(&REC709_TO_2020, rgb)
}

/// Espace couleur d'une image déjà développée (JPEG boîtier, image plate, rendu d'un
/// logiciel) : remarque de l'utilisateur (19/10/2026), un JPEG boîtier peut être en Adobe
/// RGB (réglage du boîtier), il ne faut pas le supposer sRGB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// sRGB (aussi : rien d'indiqué).
    Srgb,
    /// Adobe RGB (1998).
    AdobeRgb,
    /// Profil ICC intégré (autre que sRGB ou Adobe RGB reconnus).
    Icc(Vec<u8>),
}

impl Origin {
    /// D'un JPEG : son profil ICC intégré, sinon l'EXIF (Adobe RGB selon la norme DCF :
    /// `InteroperabilityIndex` « R03 », ou `ColorSpace` 2 de certains boîtiers), sinon sRGB
    /// (l'espace par défaut de l'EXIF).
    pub fn of_jpeg(bytes: &[u8]) -> Origin {
        if let Some(icc) = crate::jpeg::icc_profile(bytes) {
            return Origin::of_icc(icc);
        }
        let exif = exif::Reader::new().read_from_container(&mut std::io::Cursor::new(bytes));
        let adobe = exif.is_ok_and(|e| {
            let interop = e
                .get_field(exif::Tag::InteroperabilityIndex, exif::In::PRIMARY)
                .is_some_and(|f| f.display_value().to_string().contains("R03"));
            let space = e
                .get_field(exif::Tag::ColorSpace, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0));
            interop || space == Some(2)
        });
        if adobe {
            Origin::AdobeRgb
        } else {
            Origin::Srgb
        }
    }

    /// D'un profil ICC : sRGB et Adobe RGB reconnus à leur description (ASCII ou UTF-16).
    /// Un profil « linéaire » n'est jamais pris pour l'un d'eux (sa courbe diffère).
    pub fn of_icc(icc: Vec<u8>) -> Origin {
        if icc.as_slice() == working_icc() {
            return Origin::Icc(icc);
        }
        let has = |text: &str| {
            let ascii = text.as_bytes();
            let wide: Vec<u8> = text.bytes().flat_map(|b| [0, b]).collect();
            icc.windows(ascii.len()).any(|w| w == ascii)
                || icc.windows(wide.len()).any(|w| w == wide.as_slice())
        };
        if has("inear") || has("scRGB") {
            Origin::Icc(icc)
        } else if has("sRGB") {
            Origin::Srgb
        } else if has("Adobe RGB") {
            Origin::AdobeRgb
        } else {
            Origin::Icc(icc)
        }
    }

    /// D'un fichier image (plate) : JPEG comme ci-dessus ; autre format : son profil ICC
    /// intégré, sinon sRGB.
    pub fn of_file(bytes: &[u8]) -> Origin {
        if crate::jpeg::dimensions(bytes).is_some() {
            return Origin::of_jpeg(bytes);
        }
        use image::ImageDecoder;
        image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_decoder().ok())
            .and_then(|mut d| d.icc_profile().ok().flatten())
            .map_or(Origin::Srgb, Origin::of_icc)
    }

    /// Son profil ; `None` : ICC illisible (l'image est alors laissée telle quelle).
    pub fn profile(&self) -> Option<ColorProfile> {
        match self {
            Origin::Srgb => Some(ColorProfile::new_srgb()),
            Origin::AdobeRgb => Some(ColorProfile::new_adobe_rgb()),
            Origin::Icc(icc) => ColorProfile::new_from_slice(icc).ok(),
        }
    }

    /// Profil ICC à intégrer quand on garde cet espace.
    pub fn icc(&self) -> Option<Vec<u8>> {
        match self {
            Origin::Icc(icc) => Some(icc.clone()),
            other => other.profile()?.encode().ok(),
        }
    }

    /// Clé de la conversion (table des conversions déjà construites).
    fn id(&self) -> Id {
        match self {
            Origin::Srgb => Id::Srgb,
            Origin::AdobeRgb => Id::AdobeRgb,
            Origin::Icc(icc) => {
                if icc.as_slice() == working_icc() {
                    Id::Working
                } else {
                    Id::Icc(*blake3::hash(icc).as_bytes())
                }
            }
        }
    }

    /// Vrai si c'est déjà l'espace de travail (rien à convertir).
    pub fn is_working(&self) -> bool {
        self.id() == Id::Working
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Id {
    Srgb,
    AdobeRgb,
    Working,
    Icc([u8; 32]),
    /// Profil d'un écran (affichage hors macOS).
    Screen([u8; 32]),
}

/// Une conversion d'un espace vers un autre, construite une fois et gardée.
#[derive(Clone)]
pub struct Transform {
    bits8: Arc<Transform8BitExecutor>,
    bits16: Option<Arc<Transform16BitExecutor>>,
}

impl std::fmt::Debug for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Transform")
    }
}

fn options() -> TransformOptions {
    TransformOptions::default()
}

impl Transform {
    fn new(from: &ColorProfile, to: &ColorProfile, deep: bool) -> Option<Transform> {
        let bits8 = from
            .create_transform_8bit(Layout::Rgb, to, Layout::Rgb, options())
            .ok()?;
        let bits16 = if deep {
            Some(
                from.create_transform_16bit(Layout::Rgb, to, Layout::Rgb, options())
                    .ok()?,
            )
        } else {
            None
        };
        Some(Transform { bits8, bits16 })
    }

    /// Convertit `bitmap` (8 bits) sur place.
    pub fn apply(&self, bitmap: &mut Bitmap) {
        let src = std::mem::take(&mut bitmap.data);
        let mut dst = vec![0; src.len()];
        if self.bits8.transform(&src, &mut dst).is_ok() {
            bitmap.data = dst;
        } else {
            bitmap.data = src;
        }
    }

    /// Convertit des pixels 16 bits (RGB entrelacés) en une image 8 bits.
    fn apply16(&self, width: u32, height: u32, src: &[u16]) -> Option<Bitmap> {
        let mut dst = vec![0u16; src.len()];
        self.bits16.as_ref()?.transform(src, &mut dst).ok()?;
        let data = dst
            .iter()
            .map(|&v| ((u32::from(v) + 128) / 257) as u8)
            .collect();
        Some(Bitmap {
            width,
            height,
            data,
        })
    }
}

type Table = Mutex<HashMap<(Id, Id, bool), Option<Transform>>>;

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// La conversion de `origin` vers l'espace de travail ; `None` : rien à faire (déjà
/// l'espace de travail) ou profil illisible.
pub fn to_working(origin: &Origin, deep: bool) -> Option<Transform> {
    let id = origin.id();
    if id == Id::Working {
        return None;
    }
    let mut table = table().lock().unwrap_or_else(|e| e.into_inner());
    table
        .entry((id, Id::Working, deep))
        .or_insert_with(|| Transform::new(&origin.profile()?, working(), deep))
        .clone()
}

/// La conversion de l'espace de travail vers un écran de profil `screen` (ICC) ; `None`
/// pour l'écran sRGB par défaut quand `screen` est `None`. Hors macOS, l'affichage la
/// passe sur chaque image avant d'en faire une texture.
pub fn to_screen(screen: Option<&[u8]>) -> Option<Transform> {
    let (id, profile) = match screen {
        Some(icc) => (
            Id::Screen(*blake3::hash(icc).as_bytes()),
            ColorProfile::new_from_slice(icc).ok()?,
        ),
        None => (Id::Srgb, ColorProfile::new_srgb()),
    };
    let mut table = table().lock().unwrap_or_else(|e| e.into_inner());
    table
        .entry((Id::Working, id, false))
        .or_insert_with(|| Transform::new(working(), &profile, false))
        .clone()
}

/// Ramène `bitmap` (dans l'espace `origin`) à l'espace de travail.
pub fn bitmap_to_working(bitmap: &mut Bitmap, origin: &Origin) {
    if let Some(t) = to_working(origin, false) {
        t.apply(bitmap);
    }
}

/// Image 16 bits (dans l'espace `origin`) ramenée à l'espace de travail, en 8 bits : la
/// conversion se fait en 16 bits, l'arrondi à 8 bits seulement à la fin (pas de cassures
/// dans les dégradés d'un TIFF ProPhoto).
pub fn deep_to_working(width: u32, height: u32, pixels: &[u16], origin: &Origin) -> Bitmap {
    if let Some(bitmap) = to_working(origin, true).and_then(|t| t.apply16(width, height, pixels)) {
        return bitmap;
    }
    // Déjà l'espace de travail, ou profil illisible : seulement ramené à 8 bits.
    Bitmap {
        width,
        height,
        data: pixels
            .iter()
            .map(|&v| ((u32::from(v) + 128) / 257) as u8)
            .collect(),
    }
}

/// Un JPEG d'un logiciel de développement (son espace : son ICC, sinon sRGB), décodé et
/// ramené à l'espace de travail.
pub fn decode_to_working(bytes: &[u8]) -> Option<Bitmap> {
    let mut bitmap = crate::jpeg::decode(bytes, 0).ok()?;
    bitmap_to_working(&mut bitmap, &Origin::of_jpeg(bytes));
    Some(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use moxcms::ToneReprCurve;

    fn convert(origin: &Origin, rgb: [u8; 3]) -> [u8; 3] {
        let mut b = Bitmap {
            width: 1,
            height: 1,
            data: rgb.to_vec(),
        };
        bitmap_to_working(&mut b, origin);
        [b.data[0], b.data[1], b.data[2]]
    }

    fn close(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
        a.iter()
            .zip(b)
            .all(|(&x, y)| (i32::from(x) - i32::from(y)).abs() <= tol)
    }

    #[test]
    fn greys_are_unchanged() {
        // Même courbe et même blanc que le sRGB : les gris ne bougent pas.
        for v in [0u8, 18, 64, 128, 200, 255] {
            assert!(
                close(convert(&Origin::Srgb, [v, v, v]), [v, v, v], 1),
                "{v}"
            );
        }
    }

    #[test]
    fn srgb_red_becomes_less_saturated_in_rec2020() {
        let red = convert(&Origin::Srgb, [255, 0, 0]);
        // Le rouge sRGB est à l'intérieur du gamut Rec.2020 : moins de rouge, un peu de vert.
        assert!(red[0] < 255 && red[1] > 0, "{red:?}");
    }

    #[test]
    fn same_colour_from_different_spaces_agrees() {
        // Une même couleur, exprimée en sRGB, en Adobe RGB et en Display P3, arrive au même
        // endroit de l'espace de travail.
        let p3 = ColorProfile::new_display_p3().encode().unwrap();
        let adobe = ColorProfile::new_adobe_rgb();
        let srgb = ColorProfile::new_srgb();
        let colour = [200u8, 120, 60];
        let to = |p: &ColorProfile| {
            let t = srgb
                .create_transform_8bit(Layout::Rgb, p, Layout::Rgb, options())
                .unwrap();
            let mut dst = [0u8; 3];
            t.transform(&colour, &mut dst).unwrap();
            dst
        };
        let reference = convert(&Origin::Srgb, colour);
        let from_adobe = convert(&Origin::AdobeRgb, to(&adobe));
        let from_p3 = convert(
            &Origin::Icc(p3.clone()),
            to(&ColorProfile::new_from_slice(&p3).unwrap()),
        );
        assert!(
            close(from_adobe, reference, 2),
            "{from_adobe:?} {reference:?}"
        );
        assert!(close(from_p3, reference, 2), "{from_p3:?} {reference:?}");
    }

    #[test]
    fn working_icc_is_recognised() {
        let origin = Origin::of_icc(working_icc().to_vec());
        assert!(origin.is_working());
        assert!(to_working(&origin, false).is_none());
    }

    #[test]
    fn linear_profiles_are_not_taken_for_srgb() {
        let mut p = ColorProfile::new_srgb();
        p.description = Some(ProfileText::PlainString("Linear sRGB".into()));
        p.red_trc = Some(ToneReprCurve::Lut(Vec::new()));
        p.green_trc = Some(ToneReprCurve::Lut(Vec::new()));
        p.blue_trc = Some(ToneReprCurve::Lut(Vec::new()));
        let icc = p.encode().unwrap();
        assert!(matches!(Origin::of_icc(icc), Origin::Icc(_)));
    }

    #[test]
    fn primaries_matrices_are_inverse_and_give_rec2020_luma() {
        for v in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.2, 0.5, 0.9]] {
            let back = srgb_linear_to_working(working_to_srgb_linear(v));
            for (a, b) in back.iter().zip(v) {
                assert!((a - b).abs() < 1e-4, "{back:?} {v:?}");
            }
        }
        // La luminance BT.709 des canaux sRGB est celle de Rec.2020 des canaux de travail.
        for (i, luma) in LUMA.iter().enumerate() {
            let mut v = [0.0; 3];
            v[i] = 1.0;
            let s = working_to_srgb_linear(v);
            let y = 0.2126 * s[0] + 0.7152 * s[1] + 0.0722 * s[2];
            assert!((y - luma).abs() < 2e-3, "{i}: {y}");
        }
    }

    #[test]
    fn eight_bit_round_trip_stays_within_two_levels() {
        // Cassures en 8 bits : un dégradé sRGB (sur fond gris) converti vers l'espace de
        // travail puis revenu en sRGB (deux arrondis, le cas de Linux / Windows) ne s'écarte
        // jamais de plus de 2 niveaux (mesuré le 11/11/2026 : ±2 au plus, dans les ombres
        // saturées, ±1 presque partout).
        let back = to_screen(None).unwrap();
        for channel in 0..3 {
            let mut b = Bitmap {
                width: 256,
                height: 1,
                data: (0..256u32)
                    .flat_map(|v| {
                        let mut px = [40u8, 40, 40];
                        px[channel] = v as u8;
                        px
                    })
                    .collect(),
            };
            bitmap_to_working(&mut b, &Origin::Srgb);
            back.apply(&mut b);
            for (v, p) in b.data.chunks(3).enumerate() {
                let error = i32::from(p[channel]) - v as i32;
                assert!(error.abs() <= 2, "canal {channel}, valeur {v} : {error}");
            }
        }
    }

    #[test]
    fn deep_conversion_keeps_precision() {
        // Un dégradé 16 bits ProPhoto : converti en 16 bits puis arrondi, aucun saut.
        let pro = ColorProfile::new_pro_photo_rgb().encode().unwrap();
        let pixels: Vec<u16> = (0..1024u32)
            .flat_map(|i| {
                let v = (i * 64) as u16;
                [v, v / 2, v / 4]
            })
            .collect();
        let b = deep_to_working(1024, 1, &pixels, &Origin::Icc(pro));
        let greens: Vec<i32> = b.data.chunks(3).map(|p| i32::from(p[1])).collect();
        for w in greens.windows(2) {
            assert!((w[1] - w[0]).abs() <= 1, "{w:?}");
        }
    }
}
