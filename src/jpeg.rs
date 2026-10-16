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
}
