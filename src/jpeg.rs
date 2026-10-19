//! Décodage et encodage JPEG rapides.
//!
//! Avec la feature `turbojpeg` (défaut) : libjpeg-turbo, y compris le décodage à taille
//! réduite (1/2, 1/4, 1/8) directement par l'IDCT. Sans elle, ou si libjpeg-turbo refuse un
//! fichier (JPEG CMJN, par exemple) : repli sur le crate `image`. Aucune orientation n'est
//! appliquée ici : c'est à l'appelant de le faire (EXIF ou métadonnées du RAW).
//! Voir `doc/jpeg.md`.

use std::fmt;

use crate::bitmap::Bitmap;

/// Vrai si ce binaire utilise libjpeg-turbo (feature `turbojpeg`).
pub const TURBO: bool = cfg!(feature = "turbojpeg");

/// Sous-échantillonnage de la chrominance à l'encodage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    /// 4:4:4 : couleur à pleine résolution (rendus 1:1).
    Full,
    /// 4:2:0 : couleur à demi-résolution, fichiers plus petits (vignettes, aperçus).
    Half,
}

/// Erreur de décodage ou d'encodage JPEG.
#[derive(Debug)]
pub struct JpegError(String);

impl fmt::Display for JpegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JPEG : {}", self.0)
    }
}

impl std::error::Error for JpegError {}

/// Décode un JPEG en RGB8. Si `min_side > 0`, le décodeur réduit l'image pendant le
/// décodage, mais jamais en dessous de `min_side` pour le plus grand côté : l'appelant fait
/// ensuite la réduction exacte avec [`Bitmap::fit`]. `min_side == 0` : pleine taille.
pub fn decode(bytes: &[u8], min_side: u32) -> Result<Bitmap, JpegError> {
    #[cfg(feature = "turbojpeg")]
    if let Ok(bitmap) = turbo::decode(bytes, min_side) {
        return Ok(bitmap);
    }
    let _ = min_side; // sans turbojpeg, décodage pleine taille
    decode_with_image(bytes)
}

/// Encode un bitmap RGB8 en JPEG. Sans la feature `turbojpeg`, `chroma` est ignoré (le
/// crate `image` choisit lui-même).
pub fn encode(bitmap: &Bitmap, quality: u8, chroma: Chroma) -> Result<Vec<u8>, JpegError> {
    #[cfg(feature = "turbojpeg")]
    return turbo::encode(bitmap, quality, chroma);
    #[cfg(not(feature = "turbojpeg"))]
    {
        let _ = chroma;
        encode_with_image(bitmap, quality)
    }
}

fn decode_with_image(bytes: &[u8]) -> Result<Bitmap, JpegError> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
        .map(|img| img.into_rgb8().into())
        .map_err(|e| JpegError(e.to_string()))
}

#[cfg(not(feature = "turbojpeg"))]
fn encode_with_image(bitmap: &Bitmap, quality: u8) -> Result<Vec<u8>, JpegError> {
    use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder};
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(
            &bitmap.data,
            bitmap.width,
            bitmap.height,
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| JpegError(e.to_string()))?;
    Ok(out)
}

/// Plus forte réduction à l'IDCT (parmi 1/8, 1/4, 1/2, 1) qui garde le plus grand côté
/// au moins égal à `min_side`. Renvoie le dénominateur.
#[cfg(any(feature = "turbojpeg", test))]
fn scale_denominator(width: u32, height: u32, min_side: u32) -> u32 {
    let long = width.max(height);
    if min_side == 0 {
        return 1;
    }
    [8, 4, 2]
        .into_iter()
        .find(|&d| long.div_ceil(d) >= min_side)
        .unwrap_or(1)
}

#[cfg(feature = "turbojpeg")]
mod turbo {
    use turbojpeg::{
        Compressor, Decompressor, Image, OutputBuf, PixelFormat, ScalingFactor, Subsamp,
    };

    use super::{Chroma, JpegError, scale_denominator};
    use crate::bitmap::Bitmap;

    fn err(e: turbojpeg::Error) -> JpegError {
        JpegError(e.to_string())
    }

    pub(super) fn decode(bytes: &[u8], min_side: u32) -> Result<Bitmap, JpegError> {
        let mut d = Decompressor::new().map_err(err)?;
        let header = d.read_header(bytes).map_err(err)?;
        let denom = scale_denominator(header.width as u32, header.height as u32, min_side);
        let factor = ScalingFactor::new(1, denom as usize);
        d.set_scaling_factor(factor).map_err(err)?;
        let scaled = header.scaled(factor);
        let mut pixels = vec![0u8; scaled.width * scaled.height * 3];
        d.decompress(
            bytes,
            Image {
                pixels: pixels.as_mut_slice(),
                width: scaled.width,
                pitch: scaled.width * 3,
                height: scaled.height,
                format: PixelFormat::RGB,
            },
        )
        .map_err(err)?;
        Ok(Bitmap {
            width: scaled.width as u32,
            height: scaled.height as u32,
            data: pixels,
        })
    }

    pub(super) fn encode(
        bitmap: &Bitmap,
        quality: u8,
        chroma: Chroma,
    ) -> Result<Vec<u8>, JpegError> {
        let mut c = Compressor::new().map_err(err)?;
        c.set_quality(i32::from(quality)).map_err(err)?;
        c.set_subsamp(match chroma {
            Chroma::Full => Subsamp::None,
            Chroma::Half => Subsamp::Sub2x2,
        })
        .map_err(err)?;
        let mut out = OutputBuf::new_owned();
        c.compress(
            Image {
                pixels: bitmap.data.as_slice(),
                width: bitmap.width as usize,
                pitch: bitmap.width as usize * 3,
                height: bitmap.height as usize,
                format: PixelFormat::RGB,
            },
            &mut out,
        )
        .map_err(err)?;
        Ok(out.to_vec())
    }
}

/// Segments d'un JPEG avant les données compressées : (marqueur, début du segment, fin).
/// `None` si ce n'est pas un JPEG lisible.
fn segments(bytes: &[u8]) -> Option<Vec<(u8, usize, usize)>> {
    if bytes.get(..2) != Some(&[0xFF, 0xD8]) {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 2;
    loop {
        // Octets de remplissage 0xFF possibles entre deux segments.
        while bytes.get(at) == Some(&0xFF) && bytes.get(at + 1) == Some(&0xFF) {
            at += 1;
        }
        if bytes.get(at) != Some(&0xFF) {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        let len = usize::from(u16::from_be_bytes([
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]));
        let end = at + 2 + len;
        if len < 2 || end > bytes.len() {
            return None;
        }
        out.push((marker, at, end));
        if marker == 0xDA {
            return Some(out);
        }
        at = end;
    }
}

/// Dimensions (largeur, hauteur) d'un JPEG, lues dans son en-tête, sans le décoder.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let (_, start, _) = segments(bytes)?
        .into_iter()
        .find(|&(m, _, _)| matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF))?;
    let h = u16::from_be_bytes([bytes[start + 5], bytes[start + 6]]);
    let w = u16::from_be_bytes([bytes[start + 7], bytes[start + 8]]);
    Some((u32::from(w), u32::from(h)))
}

/// Le même JPEG avec la balise EXIF d'orientation `value` (1 à 8), **sans recompresser** :
/// l'export « sans recompression » d'un JPEG boîtier, dont l'orientation est celle du RAW.
/// Balise existante : modifiée sur place ; sinon un segment EXIF minimal est ajouté (après
/// l'en-tête JFIF), à la place d'un segment EXIF sans orientation. `None` : JPEG illisible.
pub fn set_orientation(bytes: &[u8], value: u16) -> Option<Vec<u8>> {
    let segs = segments(bytes)?;
    let exif = segs
        .iter()
        .find(|&&(m, s, _)| m == 0xE1 && bytes.get(s + 4..s + 10) == Some(b"Exif\0\0"));
    if let Some(&(_, start, end)) = exif
        && let Some(at) = orientation_offset(&bytes[start + 10..end])
    {
        let mut out = bytes.to_vec();
        let tiff = start + 10;
        let big = &bytes[tiff..tiff + 2] == b"MM";
        let v = if big {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };
        out[tiff + at..tiff + at + 2].copy_from_slice(&v);
        return Some(out);
    }
    // Segment EXIF minimal (TIFF gros-boutiste, IFD0 d'une entrée : 0x0112, SHORT).
    let mut app1 = vec![0xFF, 0xE1, 0, 34];
    app1.extend_from_slice(b"Exif\0\0MM\0\x2A\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01");
    app1.extend_from_slice(&value.to_be_bytes());
    app1.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let after_jfif = segs
        .first()
        .filter(|&&(m, _, _)| m == 0xE0)
        .map_or(2, |&(_, _, end)| end);
    let mut out = bytes[..after_jfif].to_vec();
    out.extend_from_slice(&app1);
    let mut rest = after_jfif;
    if let Some(&(_, start, end)) = exif {
        // L'ancien segment EXIF (sans orientation) est remplacé : deux seraient invalides.
        out.extend_from_slice(&bytes[after_jfif..start]);
        rest = end;
    }
    out.extend_from_slice(&bytes[rest..]);
    Some(out)
}

/// Profil ICC intégré d'un JPEG (segments APP2 `ICC_PROFILE`, remis bout à bout dans
/// l'ordre), s'il y en a un.
pub fn icc_profile(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut parts: Vec<(u8, &[u8])> = segments(bytes)?
        .into_iter()
        .filter(|&(m, s, _)| m == 0xE2 && bytes.get(s + 4..s + 16) == Some(b"ICC_PROFILE\0"))
        .filter_map(|(_, s, e)| Some((*bytes.get(s + 16)?, bytes.get(s + 18..e)?)))
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.sort_by_key(|&(n, _)| n);
    Some(
        parts
            .into_iter()
            .flat_map(|(_, d)| d.iter().copied())
            .collect(),
    )
}

/// Position, dans les données TIFF d'un EXIF, de la valeur de la balise d'orientation de
/// l'IFD0 (type SHORT), s'il y en a une.
fn orientation_offset(tiff: &[u8]) -> Option<usize> {
    let big = match tiff.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let b = [*tiff.get(at)?, *tiff.get(at + 1)?];
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b = [
            *tiff.get(at)?,
            *tiff.get(at + 1)?,
            *tiff.get(at + 2)?,
            *tiff.get(at + 3)?,
        ];
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = usize::from(u16_at(ifd)?);
    (0..count)
        .map(|i| ifd + 2 + i * 12)
        .find(|&e| u16_at(e) == Some(0x0112) && u16_at(e + 2) == Some(3))
        .map(|e| e + 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_never_goes_below_min_side() {
        assert_eq!(scale_denominator(4416, 2944, 400), 8); // 552 ≥ 400
        assert_eq!(scale_denominator(4416, 2944, 600), 4); // 1104
        assert_eq!(scale_denominator(6240, 4160, 2560), 2); // 3120
        assert_eq!(scale_denominator(6240, 4160, 3200), 1);
        assert_eq!(scale_denominator(6240, 4160, 0), 1, "0 = pleine taille");
        assert_eq!(scale_denominator(100, 50, 400), 1, "image déjà petite");
        assert_eq!(
            scale_denominator(2944, 4416, 400),
            8,
            "orientation indifférente"
        );
    }

    fn gradient(w: u32, h: u32) -> Bitmap {
        Bitmap {
            width: w,
            height: h,
            data: (0..h)
                .flat_map(|y| (0..w).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, 90]))
                .collect(),
        }
    }

    fn mean_diff(a: &Bitmap, b: &Bitmap) -> u64 {
        let sum: u64 = a
            .data
            .iter()
            .zip(&b.data)
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum();
        sum / a.data.len() as u64
    }

    #[test]
    fn roundtrip_full_size() {
        for chroma in [Chroma::Full, Chroma::Half] {
            let src = gradient(120, 80);
            let back = decode(&encode(&src, 95, chroma).unwrap(), 0).unwrap();
            assert_eq!((back.width, back.height), (120, 80));
            assert!(mean_diff(&src, &back) < 3, "{chroma:?}");
        }
    }

    #[test]
    fn reduced_decode_respects_min_side() {
        let jpeg = encode(&gradient(800, 400), 90, Chroma::Half).unwrap();
        let bmp = decode(&jpeg, 150).unwrap();
        if TURBO {
            assert_eq!((bmp.width, bmp.height), (200, 100), "1/4 : 200 ≥ 150");
        } else {
            assert_eq!((bmp.width, bmp.height), (800, 400), "repli : pleine taille");
        }
        assert!(bmp.width.max(bmp.height) >= 150);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode(b"pas un jpeg", 0).is_err());
    }

    #[test]
    fn dimensions_and_orientation_without_recompressing() {
        let bmp = Bitmap {
            width: 64,
            height: 40,
            data: vec![120; 64 * 40 * 3],
        };
        let bytes = encode(&bmp, 90, Chroma::Half).unwrap();
        assert_eq!(dimensions(&bytes), Some((64, 40)));
        assert_eq!(dimensions(b"pas un jpeg"), None);
        // Sans EXIF : segment ajouté ; les données compressées restent identiques.
        let turned = set_orientation(&bytes, 6).unwrap();
        assert!(turned.ends_with(&bytes[bytes.len() - 100..]));
        let read = |b: &[u8]| {
            exif::Reader::new()
                .read_from_container(&mut std::io::Cursor::new(b))
                .unwrap()
                .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        };
        assert_eq!(read(&turned), Some(6));
        // Balise existante : modifiée sur place (même taille).
        let again = set_orientation(&turned, 8).unwrap();
        assert_eq!(again.len(), turned.len());
        assert_eq!(read(&again), Some(8));
        assert_eq!(dimensions(&again), Some((64, 40)));
        // Toujours décodable.
        assert_eq!(decode(&again, 0).unwrap().width, 64);
    }
}
