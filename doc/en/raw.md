# Module `tri::raw`

Reading RAW files through LibRaw: extraction of the embedded JPEG preview (fast, for
thumbnails) and full development of the sensor data (for full-screen display).

## Why LibRaw's C API

LibRaw is written in C++, but the C++ ABI is not stable and Rust cannot call it directly.
LibRaw provides a C API (`libraw_*`) that wraps it; that is what we use through FFI.

## Organisation

- **`ffi.rs`** (private): `unsafe extern "C"` declarations of the LibRaw functions used, and
  the two types that cross the boundary:
  - `LibRawData`: opaque handle (`libraw_data_t*`), never dereferenced on the Rust side;
  - `ProcessedImage`: `#[repr(C)]` mirror of `libraw_processed_image_t`, a struct ending
    with a variable-size array (`data[1]` in C) that holds the pixels or the JPEG.
- **`mod.rs`**: the safe API, described below. All the `unsafe` is confined here.
- **`csrc/libraw_shim.c`**: two C setters (`tri_libraw_set_use_camera_wb`,
  `tri_libraw_set_half_size`) for `params` fields that the C API does not expose.

## Public API

### `RawFile`

An open RAW file, which owns a LibRaw instance.

```rust
let mut raw = RawFile::open("DSCF7171.RAF")?;
let thumb = raw.thumbnail()?;                      // embedded preview
let bitmap = raw.render(&RenderOptions::default())?; // full development
```

- `open(path)`: `libraw_init`, then the file is opened. The path is passed as raw bytes on
  Unix (non-UTF-8 names work), as UTF-16 on Windows. Only the format is parsed at this
  stage (~1 to 5 ms): the sensor data is not read yet.
- `thumbnail()`: `libraw_unpack_thumb` → `libraw_dcraw_make_mem_thumb`. Returns the preview
  produced by the camera, without any computation. ~10 ms on an X-T4 RAF.
- `render(&options)`: sets the parameters, then `libraw_unpack` → `libraw_dcraw_process` →
  `libraw_dcraw_make_mem_image`. Returns an 8-bit RGB `Bitmap`, **already rotated**
  according to the camera orientation.
- `thumbnail()` then `render()` can be called on the same `RawFile`.
- **Release**: `Drop` calls `libraw_close`, even if a step failed.
- `orientation()`: rotation to apply to the **embedded preview** (`Rotation::Cw90`…), read
  from `sizes.flip`. The render, for its part, is already rotated. The RAW metadata is
  authoritative: the JPEG preview is decoded *without* taking its own EXIF tag into
  account, then this rotation is applied. Otherwise, a preview that also carries the tag
  would be rotated twice.
- `size()`: dimensions of the full-resolution render, *before* rotation, available as soon
  as the file is opened. They are used to decide whether `half_size` is enough for the
  requested preview.
- `render_cancellable(&options, &cancel)`: like `render`, but returns
  `RawError::Cancelled` if the `AtomicBool` flag turns `true` (see "Cancellation").
- `sensor()`: `libraw_unpack` alone (no demosaicing), then a `Sensor` view on the sensor
  mosaic, borrowed from LibRaw (valid as long as the `RawFile` is not modified): rows of
  the visible area (`row`), filter channel (`channel`, 0 red, 1 green, 2 blue; the second
  Bayer green is mapped to green), **per-pixel black** (`black`: general black + channel
  black + the camera's black pattern, Fuji: 6 × 6 at ~1022), white (`white`), bit depth
  (`bits`). Channels and blacks are precomputed in a 24 × 24 table (common period of
  Bayer 8 × 2 and X-Trans 6 × 6, given row by row by `pattern(row)`): no call to LibRaw per
  pixel. The fields come from the shim (`tri_libraw_raw_info`, levels read from
  `rawdata.color`, the copy made by `unpack`) and from `libraw_COLOR`. Error
  `RawError::NoMosaic` for a RAW without a mosaic (linear DNG, Foveon). Basis of the RAW
  histogram ([exposure.md](exposure.md)).
- **Threads**: `RawFile` is `Send` (a LibRaw instance is not tied to any thread, it can be
  moved to a worker thread) but not `Sync` (two threads must not use it at the same time).
  Distinct instances can work in parallel.

### `Thumbnail`

```rust
pub enum Thumbnail { Jpeg(Vec<u8>), Bitmap(Bitmap) }
```

Almost all cameras embed a JPEG (`Jpeg`, bytes of the JPEG file as is). Some older formats
provide a raw bitmap, converted into a `Bitmap`.

**Warning:** the embedded preview is *not* rotated. A portrait shows up as landscape, with
an EXIF `Orientation` tag (often 6 or 8) that has to be applied at display time.

### `Bitmap`

Common type defined in [`tri::bitmap`](bitmap.md): interleaved 8-bit RGB, without padding.

### `RenderOptions`

| Field | Default | Effect |
|---|---|---|
| `camera_wb` | `true` | White balance recorded by the camera. Otherwise, LibRaw's "daylight" multipliers (often too warm or too cold a render). |
| `half_size` | `false` | Half resolution: each block of photosites becomes one pixel, without demosaicing. About 10× faster. |
| `bright` | `1.0` | Brightness applied by LibRaw. |
| `auto_bright` | `false` | LibRaw's automatic brightness (brightens until 1 % of the pixels clip). **Disabled**: it "corrected" underexposures (DSCF0130: mean luminance 39 without, 129 with, camera JPEG 65) and thus hid the shooting mistake one is looking for when culling. Without it, the RAW render shows the real exposure, often darker than the camera JPEG (which applies its tone curve). |

The output is always 8 bits per channel (`libraw_set_output_bps(8)`).

### `RawError` and `Result<T>`

| Variant | When |
|---|---|
| `Init` | `libraw_init` returned a null pointer (memory) |
| `InvalidPath(path)` | Path containing a null byte, impossible to pass to C |
| `LibRaw { code, message }` | LibRaw error code, with its message (`libraw_strerror`): missing file, unsupported format, corrupt file… |
| `EmptyImage` | LibRaw returned a null image without an error code |
| `UnsupportedImage { kind, colors, bits }` | Image produced in a format that `Bitmap` does not handle |

`RawError` implements `Display` and `std::error::Error`: it works with `?` into
`Box<dyn Error>`.

### `version()`

Version of the LibRaw actually loaded (e.g. `0.22.2-Release`).

## Internal mechanisms

- **`check(code)`**: turns a LibRaw return code (0 = success) into a `Result`.
- **`MemImage`**: wraps a `ProcessedImage` allocated by LibRaw; its `Drop` calls
  `libraw_dcraw_clear_mem`. The data is copied into a Rust `Vec` before release.
- **`bitmap_from_processed(header, bytes)`**: pure conversion (no FFI, hence testable) of a
  LibRaw image into a `Bitmap`. Accepts 8-bit RGB and 8-bit grayscale (expanded to RGB);
  rejects the rest and truncated data.

## Measurements

X-T4 (26 Mpx RAF, X-Trans sensor), Apple Silicon Mac, release build:

| Operation | Time |
|---|---|
| `open` | 1 to 5 ms |
| `thumbnail` (4416×2944 JPEG) | ≈ 10 ms |
| `render`, full resolution (4170×6246) | ≈ 3.8 s |
| `render`, `half_size` (2085×3123) | ≈ 0.35 s |

X-Trans demosaicing is particularly expensive; Bayer sensors are faster.

**A render is already parallel internally** (≈ 440 % CPU on a single `render`) and a
full-resolution instance takes ~200 MB. Running several full renders at the same time does
not save time, quite the opposite: nine renders take 5.7 s in series, 18 s on 4 threads and
302 s on one thread per core (CPU and memory contention). Extracting embedded previews
does not have this problem: it does no computation.

## Cancellation

LibRaw calls a *progress callback* (`libraw_set_progress_handler`) between processing
steps; if it returns a non-zero value, processing stops with the code
`LIBRAW_CANCELLED_BY_CALLBACK`. `render_cancellable` hooks a simple read of the flag into
it. The handler is removed when the function exits, whatever happens (`Drop` guard), so as
never to leave LibRaw with a pointer to a flag that no longer exists.

**Measured granularity** (X-T4 RAF, full render):

| Step | Time of the callback |
|---|---|
| data loading (`LOAD_RAW`) | 0 → 0.23 s |
| colour scaling (`SCALE_COLORS`), pre-interpolation | 0.25 → 0.27 s |
| *X-Trans demosaicing: no callback* | 0.27 → 3.30 s |
| RGB conversion (`CONVERT_RGB`) | 3.30 s |

A cancellation requested during demosaicing therefore takes effect at its end, about 3 s
later (the integration test shows it: cancellation requested at 300 ms, effective at
3.3–3.4 s). This does not matter with `half_size` (no demosaicing, ~0.3 s in total); for an
abandoned 1:1 render, the worker simply finishes the current block. Lead to measure if
needed: LibRaw can demosaic X-Trans in 1 pass instead of 3 (quality parameter), probably
much faster.

## Standard preview: half_size is enough

At the size of a full-screen preview, a downscaled `half_size` render is visually identical
to a downscaled full render. Measured on two X-T4 RAFs downscaled to 2560 px: PSNR of 40 to
43 dB between the two, no visible difference on 1:1 crops (eye, beard). Cost: 0.31 s +
0.15 s of downscaling, versus ≈ 3.8 s + 0.31 s. The cache therefore uses `half_size` as
long as half of the RAW's long side covers the requested size (see `CLAUDE.md`, "Aperçus et
cache").

## Test tool

```sh
cargo run --release --example decode -- thumb  photo.RAF aperçu.jpg
cargo run --release --example decode -- render photo.RAF rendu.png [--half] [--daylight] [--bright X] [--fit N]
```

Prints the LibRaw version, the dimensions and the timing of each step.
