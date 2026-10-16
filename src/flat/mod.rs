//! Décodage des images « plates » (JPEG, PNG, TIFF, WebP, HEIF/HEIC), sans développement.
//!
//! Le support HEIF dépend de la feature Cargo `heif` (activée par défaut) : HEVC est couvert
//! par des brevets, un binaire distribué doit pouvoir s'en passer (`doc/licensing.md`).
//!
//! Voir `doc/flat.md` pour le détail.

#[cfg(feature = "heif")]
mod heif;

use std::fmt;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader, metadata::Orientation};

use crate::bitmap::Bitmap;

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
    let path = path.as_ref();
    if is_heif(path) {
        decode_heif(path)
    } else {
        decode_image(path)
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

fn decode_image(path: &Path) -> Result<Bitmap> {
    let mut decoder = ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    // Une balise EXIF illisible ne doit pas empêcher l'affichage.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)?;
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
