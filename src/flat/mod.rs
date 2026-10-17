//! Décodage des images « plates » (JPEG, PNG, TIFF, WebP, HEIF/HEIC), sans développement.
//!
//! Le support HEIF dépend de la feature Cargo `heif` (activée par défaut) : HEVC est couvert
//! par des brevets, un binaire distribué doit pouvoir s'en passer (`doc/licensing.md`).
//!
//! Voir `doc/flat.md` pour le détail.

#[cfg(feature = "heif")]
mod heif;

use std::fmt;
use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation};

use crate::bitmap::Bitmap;
use crate::jpeg::{self, JpegError};

/// Extensions HEIF (HEIC iPhone, HEIF, HIF Fujifilm), en minuscules.
pub const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif"];

/// Vrai si ce binaire sait décoder le HEIF (feature `heif` compilée).
pub const HEIF_SUPPORTED: bool = cfg!(feature = "heif");

/// Erreurs du module flat.
#[derive(Debug)]
pub enum FlatError {
    /// Lecture du fichier impossible.
    Io(std::io::Error),
    /// Erreur du crate `image` (format non reconnu, fichier corrompu...).
    Image(image::ImageError),
    /// Erreur de décodage JPEG (libjpeg-turbo puis repli `image`).
    Jpeg(JpegError),
    /// Fichier HEIF alors que ce binaire a été compilé sans la feature `heif`.
    HeifNotCompiled,
    /// Erreur de libheif.
    #[cfg(feature = "heif")]
    Heif(libheif_rs::HeifError),
    /// libheif n'a pas produit le plan RGB 8 bits entrelacé demandé.
    #[cfg(feature = "heif")]
    UnexpectedHeifLayout,
}

impl fmt::Display for FlatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "lecture impossible : {e}"),
            Self::Image(e) => write!(f, "décodage impossible : {e}"),
            Self::Jpeg(e) => write!(f, "décodage impossible : {e}"),
            Self::HeifNotCompiled => write!(f, "HEIF non pris en charge (compilé sans « heif »)"),
            #[cfg(feature = "heif")]
            Self::Heif(e) => write!(f, "décodage HEIF impossible : {e}"),
            #[cfg(feature = "heif")]
            Self::UnexpectedHeifLayout => write!(f, "libheif n'a pas renvoyé de RGB 8 bits"),
        }
    }
}

impl std::error::Error for FlatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Jpeg(e) => Some(e),
            Self::HeifNotCompiled => None,
            #[cfg(feature = "heif")]
            Self::Heif(e) => Some(e),
            #[cfg(feature = "heif")]
            Self::UnexpectedHeifLayout => None,
        }
    }
}

impl From<std::io::Error> for FlatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<JpegError> for FlatError {
    fn from(e: JpegError) -> Self {
        Self::Jpeg(e)
    }
}

impl From<image::ImageError> for FlatError {
    fn from(e: image::ImageError) -> Self {
        Self::Image(e)
    }
}

/// `Result` spécialisé pour le module flat.
pub type Result<T> = std::result::Result<T, FlatError>;

/// Décode une image plate en RGB8, orientation appliquée (EXIF, ou transformations HEIF).
///
/// Le décodeur est choisi par extension pour HEIF ; pour le reste, le format est détecté
/// d'après le contenu (un PNG nommé `.jpg` est décodé correctement). Sans la feature `heif`,
/// un fichier HEIF donne `FlatError::HeifNotCompiled`.
pub fn decode(path: impl AsRef<Path>) -> Result<Bitmap> {
    decode_fit(path, 0)
}

/// Comme [`decode`], mais réduit l'image pour que son plus grand côté fasse au plus
/// `max_side` pixels (`0` = pas de réduction, voir [`Bitmap::fit`]). Pour les formats
/// décodés par `image`, l'orientation EXIF est appliquée *après* la réduction : tourner une
/// vignette coûte bien moins cher que tourner l'image entière.
pub fn decode_fit(path: impl AsRef<Path>, max_side: u32) -> Result<Bitmap> {
    let path = path.as_ref();
    if is_heif(path) {
        decode_heif(path).map(|b| b.fit(max_side))
    } else {
        decode_image(path, max_side)
    }
}

#[cfg(feature = "heif")]
fn decode_heif(path: &Path) -> Result<Bitmap> {
    heif::decode(path)
}

#[cfg(not(feature = "heif"))]
fn decode_heif(_path: &Path) -> Result<Bitmap> {
    Err(FlatError::HeifNotCompiled)
}

/// Vrai si l'extension désigne un fichier HEIF (insensible à la casse).
pub fn is_heif(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| HEIF_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Image RGB 16 bits par canal.
pub type Rgb16 = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;

/// Image plate décodée à sa profondeur d'enregistrement (analyse d'exposition), orientation
/// appliquée.
#[derive(Debug)]
pub enum Deep {
    /// 8 bits par canal (JPEG, PNG / TIFF 8 bits, WebP, HEIF).
    Bits8(Bitmap),
    /// Plus de 8 bits par canal (TIFF, PNG 16 bits ; flottants ramenés sur 16 bits).
    Bits16(Rgb16),
}

/// Comme [`decode`], mais à la profondeur du fichier : un TIFF ou PNG 16 bits donne
/// [`Deep::Bits16`] (65 536 niveaux au lieu de 256), le reste [`Deep::Bits8`]. HEIF : 8 bits
/// pour l'instant (un HEIC 10 bits est ramené à 8 bits par libheif).
pub fn decode_deep(path: impl AsRef<Path>) -> Result<Deep> {
    let path = path.as_ref();
    if is_heif(path) {
        return decode_heif(path).map(Deep::Bits8);
    }
    let bytes = std::fs::read(path)?;
    let reader = ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    if reader.format() == Some(ImageFormat::Jpeg) {
        drop(reader);
        return decode_image(path, 0).map(Deep::Bits8);
    }
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let deep =
        decoder.color_type().bits_per_pixel() / decoder.color_type().channel_count() as u16 > 8;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(if deep {
        Deep::Bits16(img.into_rgb16())
    } else {
        Deep::Bits8(img.into_rgb8().into())
    })
}

fn decode_image(path: &Path, max_side: u32) -> Result<Bitmap> {
    let bytes = std::fs::read(path)?;
    let reader = ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    let is_jpeg = reader.format() == Some(ImageFormat::Jpeg);
    // Le décodeur `image` ne lit ici que les en-têtes (format, balise EXIF).
    let mut decoder = reader.into_decoder()?;
    // Une balise EXIF illisible ne doit pas empêcher l'affichage.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let decoded = if is_jpeg {
        drop(decoder);
        // JPEG : libjpeg-turbo si disponible, réduit dès le décodage.
        jpeg::decode(&bytes, max_side)?
    } else {
        Bitmap::from(DynamicImage::from_decoder(decoder)?.into_rgb8())
    };
    let mut img = DynamicImage::ImageRgb8(decoded.fit(max_side).into());
    img.apply_orientation(orientation);
    Ok(img.into_rgb8().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heif_extensions_case_insensitive() {
        for name in ["a.HEIC", "a.heic", "a.Heif", "a.HIF"] {
            assert!(is_heif(Path::new(name)), "{name}");
        }
        for name in ["a.jpg", "a.heic.jpg", "heic", "a.avif"] {
            assert!(!is_heif(Path::new(name)), "{name}");
        }
    }
}
