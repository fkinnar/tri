# Module `tri::preview`

Synchronous computation of the cached images (see `CLAUDE.md`, "Aperçus et cache").

- `Sizes { thumbnail: 400, standard: 2560 }`: longest side of each reduced level.
- `Settings`: everything that tunes the computation, read from the configuration file
  (see [settings.md](settings.md)): `sizes`, `render` (`RenderOptions`: white balance,
  brightness, automatic brightness; `half_size` is ignored there, chosen per level),
  `half_size_margin` (1.0), `embedded_min_ratio` (0.5), `min_embedded_full` (1024 px).
- `compute(file, level, settings, cancel)`:
  - RAW thumbnail: embedded preview decoded *without* its EXIF tag, shrunk, then rotated
    according to `RawFile::orientation()`; without a preview, a `half_size` render;
  - RAW standard preview: `half_size` if `half_size_suffices(size, side, margin)` (half
    of the long side ≥ `half_size_margin` × requested side), otherwise full render, then
    `fit`;
  - RAW camera preview (`Embedded`): embedded JPEG decoded (libjpeg-turbo, reduced at the
    IDCT without going below the standard size), shrunk to the standard size,
    straightened. Refused (`PreviewError::NoEmbeddedPreview`) if it is smaller than
    `embedded_min_ratio` (half) of the requested size: some cameras embed only a 160 px
    thumbnail. For a flat image, this level is identical to the standard preview (the
    scheduler does not request it);
  - 1:1 of the camera JPEG (`EmbeddedFull`): embedded JPEG at its original size,
    straightened; refused below `min_embedded_full` (1024 px);
  - RAW 1:1: cancellable full render;
  - flat images: `flat::decode` then `fit` (except for 1:1).
- `params(level, settings)`: stable string included in the cache key (`RULES_VERSION` and
  only the settings that influence this level: size, render, thresholds); changing a
  setting recomputes the levels concerned, and only those (tested by
  `params_follow_only_the_settings_that_matter`).
- `ensure(cache, …)`: computes only if the entry is missing; returns `Cached` or
  `Computed`. For `Level::Exposure`, computes the analysis (`compute_exposure`: sensor
  data for a RAW, decoded image for a flat image) and writes it in binary (`put_bytes`).
- `exposure(cache, file, settings)`: analysis read from the cache (`None` if missing; an
  unreadable entry is deleted so it gets recomputed). Its key contains all the
  `[exposure]` settings.
- `camera_exposure(cache, file, settings)` (2026-10-27): exposure of a RAW's **camera
  JPEG**, from its cached camera preview (`Level::Embedded`); `None` for a flat image or
  while the preview is missing. No cache of its own (a few tens of ms): the interface
  keeps it in memory and overlays it on the sensor histogram.
- `PreviewError`: `Raw`, `Flat`, `Io`, `Cancelled`, `NotAnImage` (`compute` on
  `Level::Exposure`).
