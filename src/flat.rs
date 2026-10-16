//! Décodage des images « plates » (JPEG, PNG, TIFF, WebP, HEIF/HEIC), sans développement.
//!
//! Voir `doc/flat.md` pour le détail.

use std::fmt;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader, metadata::Orientation};
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

use crate::bitmap::Bitmap;

/// Extensions décodées par libheif (HEIC iPhone, HEIF, HIF Fujifilm), en minuscules.
pub const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif"];

/// Erreurs du module flat.
#[derive(Debug)]
pub enum FlatError {
    /// Lecture du fichier impossible.
    Io(std::io::Error),
    /// Erreur du crate `image` (format non reconnu, fichier corrompu...).
    Image(image::ImageError),
    /// Erreur de libheif.
    Heif(libheif_rs::HeifError),
    /// libheif n'a pas produit le plan RGB 8 bits entrelacé demandé.
    UnexpectedHeifLayout,
}

impl fmt::Display for FlatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "lecture impossible : {e}"),
            Self::Image(e) => write!(f, "décodage impossible : {e}"),
            Self::Heif(e) => write!(f, "décodage HEIF impossible : {e}"),
            Self::UnexpectedHeifLayout => write!(f, "libheif n'a pas renvoyé de RGB 8 bits"),
        }
    }
}

impl std::error::Error for FlatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Heif(e) => Some(e),
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

impl From<libheif_rs::HeifError> for FlatError {
    fn from(e: libheif_rs::HeifError) -> Self {
        Self::Heif(e)
    }
}

/// `Result` spécialisé pour le module flat.
pub type Result<T> = std::result::Result<T, FlatError>;

/// Décode une image plate en RGB8, orientation appliquée (EXIF, ou transformations HEIF).
///
/// Le décodeur est choisi par extension pour HEIF ; pour le reste, le format est détecté
/// d'après le contenu (un PNG nommé `.jpg` est décodé correctement).
pub fn decode(path: impl AsRef<Path>) -> Result<Bitmap> {
    let path = path.as_ref();
    if is_heif(path) {
        decode_heif(path)
    } else {
        decode_image(path)
    }
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
    let rgb = img.into_rgb8();
    Ok(Bitmap {
        width: rgb.width(),
        height: rgb.height(),
        data: rgb.into_raw(),
    })
}

fn decode_heif(path: &Path) -> Result<Bitmap> {
    // Lecture en mémoire : read_from_file n'accepte que des chemins UTF-8.
    let bytes = std::fs::read(path)?;
    let ctx = HeifContext::read_from_bytes(&bytes)?;
    let handle = ctx.primary_image_handle()?;
    // Les transformations (rotation, miroir, recadrage) sont appliquées par défaut.
    let image = LibHeif::new().decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?;
    let plane = image
        .planes()
        .interleaved
        .filter(|p| p.storage_bits_per_pixel == 24)
        .ok_or(FlatError::UnexpectedHeifLayout)?;
    let data = pack_rows(plane.data, plane.width, plane.height, plane.stride)
        .ok_or(FlatError::UnexpectedHeifLayout)?;
    Ok(Bitmap {
        width: plane.width,
        height: plane.height,
        data,
    })
}

/// Retire le remplissage de fin de ligne (`stride` ≥ `width * 3`) d'un plan RGB8.
/// `None` si les données sont trop courtes ou le stride incohérent.
fn pack_rows(data: &[u8], width: u32, height: u32, stride: usize) -> Option<Vec<u8>> {
    let row = width as usize * 3;
    if stride < row {
        return None;
    }
    let mut out = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        out.extend_from_slice(data.get(y * stride..y * stride + row)?);
    }
    Some(out)
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

    #[test]
    fn pack_rows_removes_padding() {
        // 2×2 pixels, stride de 8 (6 octets utiles + 2 de remplissage).
        let data = [1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0];
        assert_eq!(
            pack_rows(&data, 2, 2, 8).unwrap(),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
    }

    #[test]
    fn pack_rows_without_padding_is_identity() {
        let data: Vec<u8> = (0..12).collect();
        assert_eq!(pack_rows(&data, 2, 2, 6).unwrap(), data);
    }

    #[test]
    fn pack_rows_last_row_may_omit_padding() {
        let data = [1, 2, 3, 0, 4, 5, 6];
        assert_eq!(pack_rows(&data, 1, 2, 4).unwrap(), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn pack_rows_rejects_short_data_and_small_stride() {
        assert!(pack_rows(&[0; 11], 2, 2, 6).is_none());
        assert!(pack_rows(&[0; 12], 2, 2, 5).is_none());
    }
}
