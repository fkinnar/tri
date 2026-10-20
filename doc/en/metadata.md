# Module `tri::metadata`

Shooting metadata: camera, lens, exposure, compensation, mode, focal length, flash, date,
dimensions, file size. Read on demand (a few ms), not cached.

## Sources

- **RAW**: LibRaw as soon as the file is opened, without decompressing
  (`RawFile::shooting_info`, shim `tri_libraw_metadata`): make, model, lens, ISO, shutter
  speed, aperture, focal length and 35 mm equivalent, date (formatted in local time by the
  shim: `localtime` + `strftime`, no date dependency), dimensions. LibRaw does not expose
  **exposure compensation** or the mode: they come from the **EXIF of the embedded JPEG**
  (`thumbnail()`, ~10 ms), which only fills gaps (LibRaw is authoritative).
  Measured: 2 to 8 ms per X-T4 RAF.
- **Flat images**: their own EXIF, read by `kamadak-exif` (JPEG, TIFF, HEIF, PNG,
  WebP); dimensions read from the header (`image::image_dimensions`) when there is no
  EXIF, then swapped if the EXIF orientation is a quarter turn (5 to 8).
- An image without EXIF gives empty fields, not an error.

Choice of `kamadak-exif` (2026-10-17): BSD-2-Clause, a single dependency
(`mutate_once`, BSD-2-Clause), the most used (3.7 M recent downloads), last commit
October 2025, reads all our flat image containers. Rejected: `rexif` (JPEG and TIFF only,
licence not identified on GitHub), `nom-exif` (heavier: chrono, regex, nom…; "non
standard" licence on crates.io).

## Display

`Metadata::rows()`: (label, value) rows in the interface language, in order, unknown
fields omitted — ready for any interface: "Exposure: 1/250 s · f/2.8 · ISO 400",
"Compensation: +0.7 EV", "Focal length: 5.7 mm (35 mm equiv.: 26 mm)",
"Date: 2026-10-16 14:32:05", "Dimensions: 4160 × 6240 (26 MP)". Helpers:
`camera()` (without repeating the make), `shutter_text()`, `exposure_text()`.

`Metadata::embedded_size` (2026-10-21): for a RAW, the dimensions of the embedded JPEG,
read from its header only (the RAW's orientation applied); the resolution of the
"camera JPEG" in full screen.

## Command line

```
tri <dossier> --metadata [--raw|--flat] [--depth N]
```

Verified (2026-10-17): X-T4 RAF (Sigma lens, manual mode), exported JPEG, iPhone 13 Pro
HEIC (focal length 5.7 mm, portrait orientation).

## Focus point (`focus_point`, 2026-10-17)

Relative coordinates (0 to 1) in the **displayed** image (orientation applied), to draw
the focus point marker. Read from the **maker note** in the EXIF of the embedded JPEG
(`MakerNote`, which `kamadak-exif` returns raw):

- **Fujifilm**: `FUJIFILM` header, IFD position (32 bits), little-endian IFD, tag
  **0x1023** `FocusPixel` (two 16-bit integers x, y, in pixels of the JPEG image before
  orientation: we divide by `PixelXDimension` / `PixelYDimension`), then the EXIF
  orientation (1 to 8) is applied. Verified on an X-T4 RAF held vertically (ExifTool:
  `FocusPixel 2517 1317`, JPEG 4416 × 2944, orientation 6): (0.553; 0.570), right on the
  post where the camera had focused. (A first version read tag 0x1031: wrong point,
  caught by the test on a real file.)
- Other makes: not yet (`None`); each has its own maker note format.

Tests: synthetic note (read, other make rejected, truncated note rejected), orientations;
`tests/metadata.rs` on a real RAF (`TRI_TEST_RAW`, `#[ignore]`).
