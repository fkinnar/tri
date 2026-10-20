# Module `tri::flat`

Decoding of "flat" images — already developed by the camera or by software — into an
RGB8 [`Bitmap`](bitmap.md). No development: the pixels are simply read.

| Format | Extensions | Decoder |
|---|---|---|
| JPEG | `jpg`, `jpeg`, `jpe` | libjpeg-turbo (feature `turbojpeg`), otherwise the `image` crate — see [jpeg.md](jpeg.md) |
| PNG | `png` | `image` crate |
| TIFF | `tif`, `tiff` | `image` crate |
| WebP | `webp` | `image` crate |
| HEIF / HEIC | `heic`, `heif`, `hif` | libheif, via the `libheif-rs` crate |

`heic`: iPhone. `hif`: HEIF extension used by recent Fujifilm cameras.

## Feature `heif`

HEIF support is a **Cargo feature**, enabled by default:

```sh
cargo build --release                         # with HEIF (libheif required)
cargo build --release --no-default-features   # without HEIF: no link to libheif / HEVC
```

Why: HEVC is patented and Homebrew's libheif embeds x265, under the GPL (see
[licensing.md](licensing.md)). A distributed binary must be able to do without it.

All the libheif code is in `src/flat/heif.rs`, compiled only with the feature. Without
it:

- `decode` on a HEIF file returns `FlatError::HeifNotCompiled` ("HEIF not supported"),
  without even reading the file;
- the `FlatError::Heif` and `UnexpectedHeifLayout` variants do not exist;
- `HEIF_SUPPORTED` is `false`, which lets the interface adapt its message;
- `media::scan` **still lists** HEIC files. They exist, and the interface must be able
  to say "present, but not decodable by this binary" rather than hide them.

## API

### `decode_deep(path) -> Result<Deep>`

Like `decode`, but **at the file's bit depth**, for exposure analysis:
`Deep::Bits16(Rgb16)` for a TIFF or a PNG with more than 8 bits per channel (floats
brought down to 16 bits), `Deep::Bits8(Bitmap)` otherwise. Depth read from the header
before decoding; JPEG through the same path as `decode`. HEIF: 8 bits for now. Display,
for its part, stays in 8 bits (`decode`).

### `decode(path) -> Result<Bitmap>`

Decodes the file and **applies the orientation**: the returned `Bitmap` is the right way
up, ready to display.

- **HEIF** (chosen by extension, case-insensitive): the file is read into memory then
  passed to libheif (`HeifContext::read_from_bytes`). This avoids `read_from_file`,
  which only accepts UTF-8 paths. libheif applies by itself the transformations stored in
  the file (rotation `irot`, mirror `imir`, crop `clap`). An interleaved 8-bit RGB plane
  is requested; 10-bit sources are converted by libheif.
- **Other formats**: the format is **detected from the content**, not from the extension
  (a PNG named `.jpg` is decoded correctly). The EXIF orientation is read then applied.
  An unreadable EXIF tag is ignored (image shown without rotation) rather than making
  decoding fail.

### `decode_fit(path, max_side) -> Result<Bitmap>`

Like `decode`, but scales the image down to `max_side` (see `Bitmap::fit`; `0` = no
scaling). For formats decoded by `image`, the EXIF orientation is applied *after* the
scaling: rotating a thumbnail costs much less than rotating a 26 Mpx image.
`decode(path)` is equivalent to `decode_fit(path, 0)`. Used for thumbnails and standard
previews of flat images.

### `is_heif(path) -> bool`

True if the extension is one of `HEIF_EXTENSIONS`, whether the feature is compiled or
not.

### `HEIF_SUPPORTED: bool`

`true` if this binary was compiled with the `heif` feature.

### `FlatError` and `Result<T>`

| Variant | When |
|---|---|
| `Io(io::Error)` | File missing or unreadable |
| `Image(image::ImageError)` | Unrecognized format, corrupted file (other than HEIF) |
| `HeifNotCompiled` | HEIF file, binary compiled without the `heif` feature |
| `Heif(HeifError)` | *(feature `heif`)* libheif error: corrupted file, missing codec… |
| `UnexpectedHeifLayout` | *(feature `heif`)* libheif did not provide interleaved 8-bit RGB (should not happen) |

`FlatError` implements `std::error::Error`, with `source()` returning the original
error.

## Internal mechanisms

- **`heif::decode`** (`src/flat/heif.rs`): read into memory, libheif context, primary
  image, decoding into interleaved RGB.
- **`heif::pack_rows(data, width, height, stride)`**: libheif may pad the end of each
  row for memory alignment (`stride` > `width * 3`). This pure function copies the rows
  without the padding, and rejects an inconsistent stride or data that is too short.

## Measurements

Apple Silicon Mac, release build:

| Image | Time |
|---|---|
| iPhone HEIC 3024×4032 | ≈ 0.2 s |
| Fujifilm JPEG 4160×6240 (orientation applied) | ≈ 0.35 s |

For the grid, decoding the whole image and then scaling it down is expensive. The
options, to be measured before choosing: the EXIF thumbnail embedded in JPEGs, the HEIF
thumbnail (`thumbnail_ids`), or a reduced-resolution JPEG decode.

## Licenses and patents

libheif and the HEVC decoder libde265 are under the LGPL, linked dynamically. **HEVC is
covered by patents**: no consequence for personal use, but to be dealt with before
distributing binaries. Homebrew's libheif is also linked to the x265 encoder, under the
GPL. Hence the `heif` feature, which can be disabled. The details and the other options
(system decoder on macOS or Windows…) are in [licensing.md](licensing.md).
