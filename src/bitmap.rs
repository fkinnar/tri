//! Type d'image commun produit par le moteur (RAW développé ou image plate décodée).

impl From<image::RgbImage> for Bitmap {
    fn from(img: image::RgbImage) -> Self {
        Bitmap {
            width: img.width(),
            height: img.height(),
            data: img.into_raw(),
        }
    }
}

impl From<Bitmap> for image::RgbImage {
    fn from(bmp: Bitmap) -> Self {
        image::RgbImage::from_raw(bmp.width, bmp.height, bmp.data)
            .expect("Bitmap incohérent : data.len() != width * height * 3")
    }
}

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
    /// gardant les proportions. Une image déjà assez petite (ou `max_side == 0`) est renvoyée
    /// telle quelle : on n'agrandit jamais.
    ///
    /// Pour une forte réduction, une moyenne par blocs (rapide) ramène d'abord l'image à
    /// environ deux fois la taille cible, puis Lanczos3 fait la dernière étape : même rendu
    /// qu'un Lanczos3 direct, pour une fraction du coût.
    pub fn fit(self, max_side: u32) -> Bitmap {
        let long = self.width.max(self.height);
        if long <= max_side || max_side == 0 {
            return self;
        }
        // Dimensions finales calculées sur l'original : pas de cumul d'arrondis.
        let scale = |side: u32| {
            ((u64::from(side) * u64::from(max_side)).div_ceil(u64::from(long)) as u32).max(1)
        };
        let (width, height) = (scale(self.width), scale(self.height));
        // Moyenne par blocs tant que les deux côtés restent au moins au double de la cible.
        let factor = (self.width / (2 * width)).min(self.height / (2 * height));
        let src = if factor >= 2 {
            self.box_downsample(factor)
        } else {
            self
        };
        let src = image::RgbImage::from_raw(src.width, src.height, src.data)
            .expect("Bitmap incohérent : data.len() != width * height * 3");
        image::imageops::resize(&src, width, height, image::imageops::FilterType::Lanczos3).into()
    }

    /// Moyenne de chaque bloc `factor`×`factor`. Les pixels de bord qui ne remplissent pas un
    /// bloc entier sont ignorés (moins de `factor` pixels, négligeable avant une réduction).
    fn box_downsample(self, factor: u32) -> Bitmap {
        let (w, k) = (self.width as usize, factor as usize);
        let (width, height) = (self.width / factor, self.height / factor);
        let mut data = Vec::with_capacity(width as usize * height as usize * 3);
        let mut sums = vec![0u32; width as usize * 3];
        for by in 0..height as usize {
            sums.fill(0);
            for row in by * k..(by + 1) * k {
                let line = &self.data[row * w * 3..(row * w + width as usize * k) * 3];
                for (bx, block) in line.chunks_exact(k * 3).enumerate() {
                    for px in block.as_chunks::<3>().0 {
                        sums[bx * 3] += u32::from(px[0]);
                        sums[bx * 3 + 1] += u32::from(px[1]);
                        sums[bx * 3 + 2] += u32::from(px[2]);
                    }
                }
            }
            let n = (k * k) as u32;
            data.extend(sums.iter().map(|&s| ((s + n / 2) / n) as u8));
        }
        Bitmap {
            width,
            height,
            data,
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
    fn box_downsample_averages_blocks() {
        // 4×2, blocs 2×2 : moyenne de (0, 10, 40, 50) et (20, 30, 60, 70), bords ignorés.
        let bmp = Bitmap {
            width: 5,
            height: 2,
            data: [0u8, 10, 20, 30, 99, 40, 50, 60, 70, 99]
                .iter()
                .flat_map(|&v| [v, v, v])
                .collect(),
        };
        let small = bmp.box_downsample(2);
        assert_eq!((small.width, small.height), (2, 1));
        assert_eq!(small.data, [25, 25, 25, 45, 45, 45]);
    }

    #[test]
    fn fit_large_reduction_matches_direct_lanczos() {
        // Dégradé 2000×1500 : la réduction en deux temps doit rester proche d'un Lanczos3
        // direct (écart moyen < 2 niveaux).
        let (w, h) = (2000u32, 1500u32);
        let bmp = Bitmap {
            width: w,
            height: h,
            data: (0..h)
                .flat_map(|y| (0..w).flat_map(move |x| [(x / 8) as u8, (y / 6) as u8, 100]))
                .collect(),
        };
        let direct: Bitmap = image::imageops::resize(
            &image::RgbImage::from_raw(w, h, bmp.data.clone()).unwrap(),
            200,
            150,
            image::imageops::FilterType::Lanczos3,
        )
        .into();
        let fast = bmp.fit(200);
        assert_eq!((fast.width, fast.height), (200, 150));
        let diff: u64 = fast
            .data
            .iter()
            .zip(&direct.data)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert!(
            diff / (fast.data.len() as u64) < 2,
            "écart moyen {}",
            diff / fast.data.len() as u64
        );
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
