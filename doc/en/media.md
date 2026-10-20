# Module `tri::media`

Scanning a folder and classifying each image file as **RAW** (to be developed via LibRaw)
or **flat image** (JPEG, TIFF, PNG… already developed, simply decoded).

## Types

### `MediaKind`

```rust
pub enum MediaKind { Raw, Flat }
```

The nature of a file, deduced from its extension. It determines the processing: embedded
preview + `raw::RawFile::render` for `Raw`, direct decoding for `Flat`.

### `MediaFile`

```rust
pub struct MediaFile { pub path: PathBuf, pub kind: MediaKind }
```

A file found by `scan`. `path` includes the root of the scan. It is the unit handled by
the filters (see [filter.md](filter.md)).

## Functions

### `kind_of(path) -> Option<MediaKind>`

Classifies a path by its extension, **case-insensitively** (`.RAF`, `.raf`, `.Raf`).
Returns `None` for:

- unsupported extensions (`.xmp`, `.mov`, `.txt`…) and files without an extension;
- **hidden files** (name starting with `.`). On an SD card read by macOS, each photo may
  have a `._DSCF0001.RAF` double (an AppleDouble metadata file, a few KB) that has the
  same extension but is not an image.

The extension lists are public: `RAW_EXTENSIONS` (photo formats supported by LibRaw:
`raf`, `cr2`, `cr3`, `nef`, `arw`, `dng`, `orf`, `rw2`…) and `FLAT_EXTENSIONS` (`jpg`,
`jpeg`, `jpe`, `png`, `tif`, `tiff`, `webp`, `heic`, `heif`, `hif`). They are lowercase and disjoint (checked
by a test).

RAW video formats (`.r3d`, `.braw`) are deliberately excluded. AVIF is not listed:
depending on the distribution, libheif does not always have an AV1 decoder. Decoding of
flat images is described in [flat.md](flat.md).

### `scan(root, depth) -> io::Result<Vec<MediaFile>>`

Lists the images in `root` (via `walkdir`):

- `depth`: `1` = the folder alone, `2` = one level of subfolders, etc.; `0` = nothing;
  `usize::MAX` = no limit (e.g. a whole card `DCIM/100_FUJI/…`).
- only files are kept (a folder named `x.RAF` is ignored); symbolic links are not
  followed;
- **sorting**: by path without extension, then RAW before flat image. The RAW+JPEG pairs
  of a single shot therefore always follow each other in the same order (`DSCF0001.RAF`,
  `DSCF0001.JPG`, `DSCF0002.RAF`…);
- **errors**: `Err` if `root` does not exist, is unreadable or is not a folder
  (`ErrorKind::NotADirectory`). Unreadable entries *inside* (permissions, broken link)
  are ignored: one problematic file must not prevent displaying the rest.

The scan is done entirely and in memory (no lazy iterator): sorting requires it, and a
few thousand entries cost a negligible amount.

## Demonstration

The `tri` binary lists a folder, with an optional filter:

```sh
cargo run --release -- ~/Pictures/Import/2026-09-21            # everything
cargo run --release -- ~/Pictures/Import/2026-09-21 --raw      # RAW only
cargo run --release -- ~/Pictures/Import/2026-09-21 --flat --depth 3
```
