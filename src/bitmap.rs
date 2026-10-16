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
    /// Réduit l'image pour que son plus grand côté fasse au plus `max_side` pixels, en
    /// gardant les proportions (filtre Lanczos3). Une image déjà assez petite est renvoyée
    /// telle quelle : on n'agrandit jamais.
    pub fn fit(self, max_side: u32) -> Bitmap {
        let long = self.width.max(self.height);
        if long <= max_side || max_side == 0 {
            return self;
        }
        let scale = |side: u32| {
            ((u64::from(side) * u64::from(max_side)).div_ceil(u64::from(long)) as u32).max(1)
        };
        let (width, height) = (scale(self.width), scale(self.height));
        let src = image::RgbImage::from_raw(self.width, self.height, self.data)
            .expect("Bitmap incohérent : data.len() != width * height * 3");
        let dst =
            image::imageops::resize(&src, width, height, image::imageops::FilterType::Lanczos3);
        Bitmap {
            width,
            height,
            data: dst.into_raw(),
        }
    }

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

    fn solid(width: u32, height: u32, rgb: [u8; 3]) -> Bitmap {
        Bitmap {
            width,
            height,
            data: rgb.repeat((width * height) as usize),
        }
    }

    #[test]
    fn fit_keeps_aspect_ratio_on_long_side() {
        let landscape = solid(400, 300, [10, 20, 30]).fit(100);
        assert_eq!((landscape.width, landscape.height), (100, 75));
        let portrait = solid(300, 400, [10, 20, 30]).fit(100);
        assert_eq!((portrait.width, portrait.height), (75, 100));
        assert_eq!(portrait.data.len(), 75 * 100 * 3);
    }

    #[test]
    fn fit_never_upscales() {
        let small = solid(50, 40, [1, 2, 3]);
        assert_eq!(small.clone().fit(100), small);
        assert_eq!(small.clone().fit(50), small);
        assert_eq!(small.clone().fit(0), small, "0 = pas de limite");
    }

    #[test]
    fn fit_preserves_solid_color_and_tiny_sides() {
        let bmp = solid(1000, 3, [200, 100, 50]).fit(100);
        assert_eq!((bmp.width, bmp.height), (100, 1));
        assert!(bmp.data.chunks(3).all(|p| p == [200, 100, 50]));
    }

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
