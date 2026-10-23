# Tests

## Organization

- **Unit tests**: in each module, under `#[cfg(test)] mod tests`. They test the internal
  logic, private functions included (e.g. `bitmap_from_processed`), without depending on
  external files.
- **Integration tests**: in `tests/`, one file per module (`tests/raw.rs`…). They use
  only the public API, as the interface would. Temporary files go through the `tempfile`
  crate (deleted automatically).
- **Fixtures**: `tests/fixtures/` only contains small synthetic files, impossible to
  generate from Rust without a heavy dependency. `quadrants.heic` (856 bytes, 64×48, red /
  green / blue / white quadrants) was produced by:
  `heif-enc -q 95 -p chroma=444 quadrants.png -o quadrants.heic`.

## Running the tests

```sh
cargo test                       # everything that needs no external file
TRI_TEST_RAW=/chemin/photo.RAF TRI_TEST_HEIC=/chemin/photo.heic \
  cargo test --release -- --include-ignored                               # everything
```

Both feature configurations must pass:

```sh
cargo test                          # with HEIF (default)
cargo test --no-default-features    # without HEIF
```

HEIF tests are `#[cfg(feature = "heif")]`. Without the feature, a dedicated test checks
that a HEIC gives `HeifNotCompiled`.

Tests that need a real file are marked `#[ignore = "nécessite TRI_TEST_…"]`: no RAW file
is versioned (too heavy, and specific to each camera). `cargo test` reports them as
ignored instead of silently passing them. With `--include-ignored`, they fail clearly if
the variable is not set. `--release` is recommended: RAW development is much slower in
debug.

Tests on a real RAW are **serialized** by a static `Mutex` (`serial()` in
`tests/raw.rs`): each LibRaw render already occupies several cores, and running them in
parallel made them about 50× slower (302 s instead of 5.7 s).

The whole workspace: `cargo test --workspace` (and `--no-default-features`). The
interface tests are described in [gui.md](gui.md#tests).

## What is covered

### `media` (unit, `src/media.rs`)

- RAW and flat extensions recognized regardless of case; other files ignored;
- hidden and AppleDouble files (`._x.RAF`) ignored;
- extension lists in lowercase and disjoint;
- sorting: the RAW comes before its JPEG twin.

### `bitmap` (unit, `src/bitmap.rs`)

- reading a pixel, `None` outside the image;
- `rotate`: position of each pixel for 90°, 180° and 270°, four quarter turns = identity;
- `box_downsample`: exact average of the blocks, edges ignored;
- two-step `fit`: mean difference < 2 levels compared with a direct Lanczos3;
- `fit`: proportions kept (landscape and portrait), never enlarged (`0` = no limit),
  solid color preserved, tiny side rounded to at least 1 pixel.

### `flat` (unit, `src/flat/mod.rs` and `src/flat/heif.rs`)

- HEIF extensions recognized regardless of case;
- *(feature `heif`)* `pack_rows`: padding removal, case without padding, last row
  without padding, rejection of too-short data and inconsistent strides.

### `flat` (integration, `tests/flat.rs`)

- PNG, TIFF, WebP (lossless) generated on the fly: decoded to the exact pixel;
- format detected from the content (PNG named `.jpg`);
- JPEG with EXIF tag `Orientation = 6`: dimensions and colors rotated, also with
  `decode_fit` (downsizing then orientation);
- HEIC (fixture): dimensions and color of each quadrant; uppercase extension and
  non-ASCII path;
- missing file → `Io`; invalid content → `Image` or `Heif`;
- `decode_deep`: 16-bit PNG and TIFF read back identically (two distinct neighboring
  values), 8-bit PNG as `Bits8`;
- without the `heif` feature: the HEIC fixture gives `HeifNotCompiled`;
- with `TRI_TEST_HEIC`: a real iPhone HEIC decodes to consistent RGB8.

### `cache` (unit, `src/cache.rs`)

- default location per OS (HOME, absolute or relative XDG_CACHE_HOME, LOCALAPPDATA),
  missing variables;
- key: depends on each field, deterministic, unambiguous fields, **frozen value**
  (`key_hash_is_stable`, to be changed only together with `FORMAT_VERSION`);
- unique temporary files, in the target's folder;
- entries of one folder of photos share a subfolder.

### `cache` (integration, `tests/cache.rs`)

- JPEG round trip (dimensions, small mean difference), overwrite, removal;
- key changed by a new modification date or a new size;
- key depending on level and parameters, identical for relative and absolute paths;
- missing source → error;
- isolated levels, clearing one level or everything, cache usable after clearing;
- clearing one folder: its entries whatever their parameters, one level or all, not its
  subfolders; previous formats deleted in the background;
- `usage` ignores temporaries; `evict` removes the least recently used (read = use);
- corrupted entry removed by `get`;
- 4 threads writing and reading the same key: never a partial entry;
- JPEG quality clamped between 1 and 100, and low quality ⇒ smaller entry.

### `preview` (unit, `src/preview.rs`)

- `half_size` rule (exact threshold, orientation irrelevant, margin); distinct key
  parameters per level and per size; each setting changes the key only for the levels
  it influences; cancellation before start.

### `settings` (unit `src/settings.rs`, integration `tests/settings.rs`)

- default location per OS (absolute or relative XDG, missing variables);
- empty file = default settings, equal to those of the engine types; partial file;
- `Auto`: `"auto"` or a positive integer, rejection of `"automatique"`, `-1`, `1.5`,
  `true`, of a too-large integer; 0 workers brought up to 1;
- unknown key reported with its line, wrong type reported, `[gui]` section ignored by the
  engine;
- valid file read; invalid or missing file: default settings and a warning naming the
  file and the key; cache folder and quality applied;
- **`doc/config.example.toml` equal to the default values** (engine here, whole file in
  `gui/src/settings.rs`).

### `scheduler` (unit and integration)

See [scheduler.md](scheduler.md#tests). The concurrency tests were rerun in a loop (300
to 500 times) after each fix: a test that passes one time in ten proves nothing.

- Exposure analysis (2026-10-17): background levels with and without analysis, heavy
  worker for a RAW only, priority order (fitted, analysis, 1:1, then neighbors), in the
  background after the camera previews and before all RAW renders (fake producer, 30
  runs without failure).
### `jpeg` (unit, `src/jpeg.rs`)

- choice of the reduction factor (never below `min_side`, `0` = full size, image already
  small, orientation irrelevant);
- encode/decode round trip in 4:4:4 and 4:2:0; reduced decoding (1/4 with turbojpeg,
  full size without); invalid content rejected.
- These tests pass in both configurations: they also check the fallback.

### `filter` (unit, `src/filter.rs`)

- `ungrouped`, `KindFilter` (all / RAW / flat), removal of emptied groups, default
  value, `apply_all` chaining.

### `xmp` (unit, `src/xmp.rs`)

- `Notes`: stars capped without touching the flag, one color (replace / remove),
  several colors (sorted).
- Rewrite without change identical byte for byte (realistic darktable sidecar); a single
  changed value modifies only it; full update keeping history, keywords and unknown
  fields; rejection that remembers and restores the stars; several colors (darktable
  only); `xap:` prefix and element form; values written by other software (Lightroom,
  digiKam, "3.0"); empty `rdf:Description`; malformed XML rejected; new packet; sidecar
  names for both conventions.

### `xmp` (integration, `tests/xmp.rs`)

In a temporary folder: new sidecar according to the convention, most recent sidecar
chosen, uppercase `.XMP` extension, unchanged notes ⇒ file not rewritten (date intact),
malformed sidecar never overwritten, Adobe sidecar shared by a RAF and its JPG, no
temporary file left behind. Conversion: darktable → Adobe (dry run without writing,
source intact, no empty sidecar), conflict within a pair (the RAW wins), Adobe →
darktable (a shared sidecar copied for both images), up-to-date target made the most
recent, malformed source reported.

### `metadata` (unit `src/metadata.rs`, integration `tests/metadata.rs`)

- Rows in French (exposure, signed compensation, focal length and equivalent, date, Mpx,
  MB / kB), unknown fields omitted, manufacturer not repeated, exposure time, short focal
  length with decimal, 35 mm equivalent not repeated if identical.
- Fixture `tests/fixtures/exif.jpg` (16 × 12, tags set by ExifTool): all fields,
  orientation 6 → swapped dimensions; PNG without EXIF (empty fields, dimensions from
  the header); with `TRI_TEST_RAW`: camera, exposure, dimensions.

### `offload` (unit `src/offload.rs`, integration `tests/offload.rs`)

- UTC dates (leap year), destinations kept under the root (`..`, absolute, empty
  rejected; empty suffix removed).
- Fake card (two camera folders, one video): inventory (video ignored), proposed name,
  flat copy without temporary file, content and original date preserved, history read
  back from disk (second pass, other destination: everything skipped); name taken by
  another file → suffix, nothing overwritten, identical copy skipped; deletion of only
  the copied originals (video intact); verified duplicates deleted too, not those known
  only to the global history; same card in two places (copied twice) and twice in the
  same place (duplicates recognized); cancellation: nothing copied.
- `settings`: data folder per OS, `~` expanded.

### `notes` (unit `src/notes.rs`, integration `tests/notes.rs`)

- Star rules, color rules (including "no color"), combination of criteria, empty rule;
  unknown image.
- In a temporary folder: loading (malformed sidecar reported without stopping the rest),
  change written then read back from disk, RAW+JPEG pair in the Adobe convention (both
  refreshed) and darktable (same note in both sidecars, case ignored, `IMG_01` separate),
  mismatched pair realigned, malformed sidecar in the pair reported without blocking the
  rest, malformed sidecar never modified,
  `reload` after an external change, filters on a folder scanned by `media::scan`.

### `places` (unit `src/places.rs`, integration `tests/places.rs`)

- Volume folders per OS (macOS, Linux with and without `USER`, Windows), home folder,
  parents to expand, hidden names.
- In a temporary folder: subfolders sorted case-insensitively, hidden ones excluded or
  not, files left out, link to a folder followed, missing folder as an error; places of
  the machine (home, root, absolute paths); tree expanded on demand, collapsed then
  re-expanded (the sub-level stays expanded), `reveal`, `reload` after creation,
  unreadable folder reported; Time Machine volumes recognized by their content (APFS,
  HFS, option disabled), exclusion by name or path, `*` and `?` patterns; monitoring:
  volume "plugged in" then removed in a temporary folder, reported only once, zero
  interval without a thread; fixed volumes not ejectable (case ignored, only volumes can
  be ejected, `Row::ejectable`, `Tree::can_eject`); device of a mount point read from `/proc/mounts` (spaces).
- Named favourites (name cleaned, kept when another one is added or removed, follows a
  rename, empty: removed, old file read); a favourite unfolds independently of the same
  folder in the tree, both ways.

### `sort` (unit `src/sort.rs`, integration `tests/sort.rs`)

- By name ascending and descending (RAW+JPEG pairs kept, RAW first); by shooting date
  (two cameras, unknown date last in both directions); by modification date read on the
  disk; next key.

### `exposure` (unit `src/exposure.rs`, integration `tests/exposure.rs`)

- Synthetic image: counts per channel, clipped / blocked, histogram bins in EV
  (128/255 = -0.994 EV), mask (clipped at top left, blocked at bottom right), saturation
  threshold in EV, mask rotations (round trip on a non-square mask), headroom before
  saturation, binary round trip and rejection of an unreadable or truncated cache.
- Flat image written to disk then decoded: half clipped, mask, zero headroom.
- 16-bit PNG: analyzed in 16 bits (range 65,535, more than 15 EV), a near-black not
  blocked in 16 bits is blocked once brought down to 8 bits; binary round trip.
- With `TRI_TEST_RAW`: sensor data (12 to 16 bits, white > black, range = white −
  black), mask within the requested size, binary round trip.

### `i18n` (unit `src/i18n.rs`, integration `tests/i18n.rs`, interface `gui/src/i18n.rs`)

See [i18n.md](i18n.md#tests): each identifier used in the code exists in every language,
identical catalogs, plurals.

### `sharpness` (unit `src/sharpness.rs`, interface `gui/src/app.rs`)

See [sharpness.md](sharpness.md#tests).

### `selection` (unit `src/selection.rs`, interface `gui/src/app.rs`)

See [selection.md](selection.md#tests).

### `files` (unit `src/files.rs`, integration `tests/files.rs`)

See [files.md](files.md#tests): groups (pairs, shared sidecars), renaming with notes read
back, taken name rejected without changing anything, permanent deletion; the real trash
only on demand (`#[ignore]`). Interface tests always delete permanently, in their
temporary folder (`[files] trash = false`): never the user's trash.

### `folder` (unit `src/folder.rs`, integration `tests/folder.rs`)

Last folder remembered then forgotten if it disappears; no default folder; opening (absolute path, images, notes), type and
note filters combined, note read back after reopening, missing folder as an error.

### `media` + `filter` (integration, `tests/media.rs`)

On a temporary fake SD card (RAW+JPEG pairs, mixed case, AppleDouble, `.xmp`,
subfolders, folder named `x.RAF`): exact content and order of `scan`, effect of `depth`
(0, 1, 2, 3, unlimited), empty folder, missing folder, file passed as root, type filter
on the card.

### `raw` (unit, `src/raw/mod.rs`)

- RGB8 conversion, grayscale → RGB, extra bytes ignored;
- rejection of truncated data, of JPEG, of 16 bits, of an unexpected number of channels;
- translation of LibRaw error codes;
- `RenderOptions` default values, LibRaw version.

### `raw` (integration, `tests/raw.rs`)

Without a RAW file: missing file, non-RAW file, non-ASCII path passed through intact.

With `TRI_TEST_RAW`: valid JPEG preview (`FF D8` marker), consistent non-black RGB8
render, preview then render on the same handle, real effect of the white balance option
(checks the C shim), `half_size` halves the dimensions, render in another thread,
`size()` consistent with the render, embedded preview rotated by `orientation()` in the
same direction as the render, cancellation (flag already raised, then raised from
another thread during a full render).
