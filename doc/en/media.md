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

### darktable clones: one thumbnail each

A darktable clone ("duplicate") has no file of its own, only its sidecar: `IMG_01.RAF.xmp`
for clone no. 1 of `IMG.RAF`. The scan lists it as an image `IMG_01.RAF` (user's choice,
2026-10-29: one thumbnail per clone, next to the original), with its own notes and its own
development, since its sidecar is exactly the one `tri` reads for that name. The scan
recognises it by names alone, with no extra disk access: a sidecar `NAME_NN.EXT.xmp`
without an image `NAME_NN.EXT`, whose image `NAME.EXT` is in the same folder. A real image
with that name (`IMG_0001.CR2` from a Canon) stays an image.

The pixels are the original's: `source(path)` gives the file to read (the original for a
clone, `path` otherwise). LibRaw (`RawFile::open`), flat images (`flat::decode*`),
metadata, the cache stamp (`cache::stamp`) and the darktable render (the original with the
clone's sidecar) go through it. `is_clone(path)` tells whether `path` is a clone,
`clone_name("IMG_01.RAF")` gives `("IMG.RAF", 1)`. Known clones are recorded during the
scan (in-memory registry, clone path → original).

## Demonstration

The `tri` binary lists a folder, with an optional filter:

```sh
cargo run --release -- ~/Pictures/Import/2026-09-21            # everything
cargo run --release -- ~/Pictures/Import/2026-09-21 --raw      # RAW only
cargo run --release -- ~/Pictures/Import/2026-09-21 --flat --depth 3
```
