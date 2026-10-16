//! Décodage HEIF/HEIC via libheif. Compilé seulement avec la feature `heif`.

use std::path::Path;

use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

use super::{FlatError, Result};
use crate::bitmap::Bitmap;

impl From<libheif_rs::HeifError> for FlatError {
    fn from(e: libheif_rs::HeifError) -> Self {
        Self::Heif(e)
    }
}

pub(super) fn decode(path: &Path) -> Result<Bitmap> {
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
