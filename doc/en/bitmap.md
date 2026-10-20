# Module `tri::bitmap`

The engine's common image type, produced by [`raw`](raw.md) (developed RAW, bitmap
preview) and by [`flat`](flat.md) (decoded flat image).

```rust
pub struct Bitmap { pub width: u32, pub height: u32, pub data: Vec<u8> }
```

- interleaved 8-bit RGB pixels (`R G B R G B …`), row by row, **without padding**:
  `data.len() == width * height * 3`;
- **always the right way up**: the orientation (EXIF, HEIF, camera) is applied by the
  module that produced it. Only exception: `raw::Thumbnail::Jpeg`, which is not a
  `Bitmap` but raw JPEG bytes;
- independent of any display or image library: the interface converts it to its own
  format (for egui: `ColorImage::from_rgb`).

## `rotate(self, rotation) -> Bitmap`

Rotates the image clockwise according to a `Rotation` (`None`, `Cw90`, `Cw180`, `Cw270`).
`Cw90` and `Cw270` swap width and height. Used to straighten the embedded previews of
RAW files (see `RawFile::orientation` in [raw.md](raw.md)). Doing the `fit` *before* the
`rotate` is cheaper: fewer pixels to rotate.

## `fit(self, max_side) -> Bitmap`

Shrinks the image so that its longest side is at most `max_side` pixels, keeping the
proportions (rounded up, never less than one pixel). **Lanczos3** filter from the `image`
crate, the sharpest of the available filters, suited to previews.

**Two-step reduction.** The cost of a Lanczos3 grows with the reduction ratio (its kernel
widens accordingly): shrinking 4416 px to 400 px cost ~110 ms. For a strong reduction,
`fit` first does an **average over whole blocks** (`box_downsample`, very fast), as long
as both sides stay at least twice the target, then Lanczos3 for the last step. A test
checks that the result stays within 2 levels of mean difference from a direct Lanczos3.
The final dimensions are always computed from the original image. Edge pixels that do
not fill a whole block are ignored (less than one block, negligible).

- never enlarges: an image that is already small enough (or `max_side == 0`) is returned
  as is, without a copy;
- consumes the `Bitmap` (`self`): the original, often large, is freed as soon as the
  reduction is done.

Measurements (Apple Silicon Mac, release): 4170×6246 → 1710×2560 in ≈ 0.31 s;
2085×3123 → 1710×2560 in ≈ 0.15 s. If this becomes a bottleneck, the
`fast_image_resize` crate (SIMD) would be worth evaluating.

## `pixel(x, y) -> Option<[u8; 3]>`

Colour of a pixel, `None` outside the image. Handy for tests and future analyses; not
meant for walking through a whole image (use `data` directly).
