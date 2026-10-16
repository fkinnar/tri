//! Type d'image commun produit par le moteur (RAW développé ou image plate décodée).

/// Image RGB 8 bits, entrelacée (R, G, B, R, G, B...), ligne par ligne, sans remplissage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    /// Largeur en pixels.
    pub width: u32,
    /// Hauteur en pixels.
    pub height: u32,
    /// `width * height * 3` octets.
    pub data: Vec<u8>,
}

impl Bitmap {
    /// Couleur `[r, g, b]` du pixel `(x, y)`, `None` hors de l'image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 3;
        self.data.get(i..i + 3)?.try_into().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_reads_row_major_rgb() {
        let bmp = Bitmap {
            width: 2,
            height: 2,
            data: (0..12).collect(),
        };
        assert_eq!(bmp.pixel(0, 0), Some([0, 1, 2]));
        assert_eq!(bmp.pixel(1, 0), Some([3, 4, 5]));
        assert_eq!(bmp.pixel(0, 1), Some([6, 7, 8]));
        assert_eq!(bmp.pixel(2, 0), None);
        assert_eq!(bmp.pixel(0, 2), None);
    }
}
