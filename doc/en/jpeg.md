# Module `tri::jpeg`

Fast JPEG decoding and encoding, used wherever the engine handles JPEG:

- embedded previews of RAW files (thumbnails);
- flat JPEG images (`flat::decode_fit`);
- reading and writing [cache](cache.md) entries.

## Two implementations

| | `turbojpeg` feature (default) | Without the feature |
|---|---|---|
| Library | [libjpeg-turbo](https://libjpeg-turbo.org/) via the `turbojpeg` crate | `image` crate (zune-jpeg decoder, pure Rust) |
| Decoding scaled down at the IDCT (1/2, 1/4, 1/8) | yes | no (full size) |
| Choice of chroma subsampling when encoding | yes | no |

If libjpeg-turbo rejects a file (CMYK JPEG, exotic variant), `decode` retries with the
`image` crate: a readable JPEG stays readable whatever the configuration.

No orientation is applied here: `flat` reads the EXIF tag, and for a RAW file the RAW's
`flip` metadata is authoritative.

## API

- `decode(bytes, min_side) -> Result<Bitmap, JpegError>`: decodes to RGB8. With
  `min_side > 0`, picks the strongest IDCT reduction (1/8, 1/4, 1/2) that keeps the longest
  side **≥ `min_side`**; the caller then does the exact reduction with `Bitmap::fit`.
  `0` = full size.
- `encode(bitmap, quality, chroma) -> Result<Vec<u8>, JpegError>`: `Chroma::Full` (4:4:4,
  full-resolution colour, for 1:1) or `Chroma::Half` (4:2:0, thumbnails and previews).
- `TURBO: bool`: true if the feature is compiled in.

## Measurements

Apple Silicon Mac, release build:

| Operation | `image` | turbojpeg |
|---|---|---|
| Decoding a RAF embedded preview 4416×2944 | 117 ms | 63 ms (1/1), 46 ms (1/8) |
| Decoding a camera JPEG 6240×4160 | 224 ms | 172 ms (1/1), 132 ms (1/8) |
| Encoding a 1710×2560 preview, quality 90 | 41 ms | 10 ms |
| Decoding a 1710×2560 preview (cache read) | 18 ms | 11 ms |

Scaled-down decoding saves less than one might hope: in a high-quality JPEG, most of the
time goes into entropy decoding (Huffman), which cannot be reduced; the reduced scale only
saves the IDCT and the colour conversion.

Effect on a real folder (472 images, 236 RAF + 236 JPEG, empty cache): full grid in
**14–19 s instead of 29 s**, cache 20 % smaller (libjpeg-turbo's 4:2:0 encoding). Details
in [scheduler.md](scheduler.md#measurements).

## Licence

libjpeg-turbo: IJG AND BSD-3-Clause AND Zlib, permissive, compiled from source and linked
statically (see [build.md](build.md#libjpeg-turbo-built-with-the-project)); `turbojpeg` and
`turbojpeg-sys` crates: Unlicense OR MIT. No active patent on baseline JPEG. See
[licensing.md](licensing.md).

## Without decoding: dimensions and orientation

For export "without recompression" (2026-10-19, see [export.md](export.md)):

- `dimensions(bytes)` reads the size from the header (SOF segment), without decoding;
- `set_orientation(bytes, value)` sets the EXIF orientation tag (1 to 8) without touching
  the compressed data: the IFD0 tag is modified in place if it exists; otherwise, a
  minimal EXIF segment is added after the JFIF header, and replaces an EXIF segment
  without orientation (two EXIF segments would be invalid).
