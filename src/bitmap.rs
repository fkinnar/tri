//! Type d'image commun produit par le moteur (RAW développé ou image plate décodée).

/// Rotation horaire d'une image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rotation {
    /// Aucune.
    #[default]
    None,
    /// 90° dans le sens horaire.
    Cw90,
    /// 180°.
    Cw180,
    /// 270° dans le sens horaire (90° anti-horaire).
    Cw270,
}

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
    /// Tourne l'image dans le sens horaire. `Cw90` et `Cw270` échangent largeur et hauteur.
    pub fn rotate(self, rotation: Rotation) -> Bitmap {
        let (w, h) = (self.width as usize, self.height as usize);
        let (width, height) = match rotation {
            Rotation::None => return self,
            Rotation::Cw180 => (w, h),
            Rotation::Cw90 | Rotation::Cw270 => (h, w),
        };
        let mut data = Vec::with_capacity(self.data.len());
        for y in 0..height {
            for x in 0..width {
                // Pixel (x, y) de la destination ← pixel (sx, sy) de la source.
                let (sx, sy) = match rotation {
                    Rotation::Cw90 => (y, h - 1 - x),
                    Rotation::Cw180 => (w - 1 - x, h - 1 - y),
                    Rotation::Cw270 => (w - 1 - y, x),
                    Rotation::None => unreachable!(),
                };
                let i = (sy * w + sx) * 3;
                data.extend_from_slice(&self.data[i..i + 3]);
            }
        }
        Bitmap {
            width: width as u32,
            height: height as u32,
            data,
        }
    }

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

    /// 3×2, pixels numérotés 0..6 (valeur répétée sur les 3 canaux) :
    /// ```text
    /// 0 1 2
    /// 3 4 5
    /// ```
    fn numbered() -> Bitmap {
        Bitmap {
            width: 3,
            height: 2,
            data: (0..6u8).flat_map(|v| [v, v, v]).collect(),
        }
    }

    fn values(bmp: &Bitmap) -> Vec<u8> {
        bmp.data.chunks(3).map(|p| p[0]).collect()
    }

    #[test]
    fn rotate_clockwise() {
        // 90° horaire : la colonne de gauche (0, 3) devient la ligne du haut, de droite à gauche.
        let r = numbered().rotate(Rotation::Cw90);
        assert_eq!((r.width, r.height), (2, 3));
        assert_eq!(values(&r), [3, 0, 4, 1, 5, 2]);

        let r = numbered().rotate(Rotation::Cw180);
        assert_eq!((r.width, r.height), (3, 2));
        assert_eq!(values(&r), [5, 4, 3, 2, 1, 0]);

        let r = numbered().rotate(Rotation::Cw270);
        assert_eq!((r.width, r.height), (2, 3));
        assert_eq!(values(&r), [2, 5, 1, 4, 0, 3]);

        assert_eq!(numbered().rotate(Rotation::None), numbered());
    }

    #[test]
    fn four_quarter_turns_are_identity() {
        let mut r = numbered();
        for _ in 0..4 {
            r = r.rotate(Rotation::Cw90);
        }
        assert_eq!(r, numbered());
        assert_eq!(
            numbered().rotate(Rotation::Cw90).rotate(Rotation::Cw270),
            numbered()
        );
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
