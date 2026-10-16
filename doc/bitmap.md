# Module `tri::bitmap`

Le type d'image commun du moteur, produit par [`raw`](raw.md) (RAW développé, aperçu bitmap)
et par [`flat`](flat.md) (image plate décodée).

```rust
pub struct Bitmap { pub width: u32, pub height: u32, pub data: Vec<u8> }
```

- pixels RGB 8 bits entrelacés (`R G B R G B …`), ligne par ligne, **sans remplissage** :
  `data.len() == width * height * 3` ;
- **toujours dans le bon sens** : l'orientation (EXIF, HEIF, boîtier) est appliquée par le
  module qui l'a produit. Seule exception : `raw::Thumbnail::Jpeg`, qui n'est pas un `Bitmap`
  mais des octets JPEG bruts ;
- indépendant de toute bibliothèque d'affichage ou d'image : l'interface le convertit dans son
  propre format (pour egui : `ColorImage::from_rgb`).

## `fit(self, max_side) -> Bitmap`

Réduit l'image pour que son plus grand côté fasse au plus `max_side` pixels, en gardant les
proportions (arrondi vers le haut, jamais moins d'un pixel). Filtre **Lanczos3** du crate
`image`, le plus net des filtres disponibles, adapté aux aperçus.

- n'agrandit jamais : une image déjà assez petite (ou `max_side == 0`) est renvoyée telle
  quelle, sans copie ;
- consomme le `Bitmap` (`self`) : l'original, souvent volumineux, est libéré dès la fin de la
  réduction.

Mesures (Mac Apple Silicon, release) : 4170×6246 → 1710×2560 en ≈ 0,31 s ;
2085×3123 → 1710×2560 en ≈ 0,15 s. Si ça devient un goulot d'étranglement, le crate
`fast_image_resize` (SIMD) serait à évaluer.

## `pixel(x, y) -> Option<[u8; 3]>`

Couleur d'un pixel, `None` hors de l'image. Pratique pour les tests et les futures analyses ;
pas pensé pour parcourir une image entière (utiliser `data` directement).
