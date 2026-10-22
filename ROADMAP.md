# tri roadmap

*[Version française](ROADMAP.fr.md)*

> **Frozen on 2026-10-21.** The beta 1 list no longer grows: any new idea goes to "after
> V1", except bugs. Feedback goes through the repository's *issues*.

## Already there (alpha 5)

Grid and full screen (camera render, RAW render, 1:1, continuous zoom, loupe), XMP
ratings compatible with darktable / Lightroom / digiKam, filters (ratings, sharpness,
exposure, invert, name search), stacks of similar photos, sharpness (score, map),
histogram and exposure zones, comparison, multiple selection, files and folders (rename,
trash, copy / move, drag and drop, RAW+JPEG pairs optional), undo, card offload, export
presets, cropped versions, darktable development, overlays, settings in the interface,
classic and vim shortcuts, mouse selection (Shift+click, Ctrl+click), update check and
issue report, sort by name or date, named favourites,
French and English. Packages for macOS (`.dmg`), Linux (`.tar.gz`) and Windows (`.msi`,
`.zip`), and nightly builds ("nightly" Release).

## Beta 1 (public)

| Topic | Details |
|---|---|
| Windows testers' feedback | Tries on real machines (paths, drives, eject, trash, AZERTY keyboard, high-density screens), bugs found |
| Testers' manual | Screenshots redone with real photos (RAW, focus point, histogram) |
| Large folders on a slow disk | Folder read in the background with visible progress (the window froze while listing thousands of files and their sidecars on an external hard drive) |
| Sort by rating, colour | To design with the stacks and the sharpness judgement, which rely on the shooting order |

The beta is **not signed**: macOS and Windows warn on first launch, the steps are in the
README.

## V1

| Topic | Details |
| --- | --- |
| Signing | macOS: Developer ID and notarisation; Windows: SignPath Foundation (free for open source). No more warning on first launch |
| Other brands' RAWs | Tests with Canon, Nikon, Sony, OM, Panasonic, DNG (raw.pixls.us); focus point for each |
| Ratings in other software | Check digiKam, RawTherapee, ART; Lightroom and Capture One (testers) |
| Sharpness in comparison | Sharpness map in side-by-side comparison |

## After V1

| Topic | Details |
| --- | --- |
| Keywords | Hierarchical, compatible with other software; interface to be mocked up first |
| Folder merge | Move a folder into a folder of the same name |
| Crop to development software | Write a version's crop into the sidecar, in the format of Lightroom (`crs:Crop…`), darktable, Capture One |
| Offload | Folder and file name templates, multi-day cards |
| Viewing adjustments | Exposure, black and white, contrast, exposure matching (non-destructive) |
| Complex filters | Dedicated window or panel |
| EXIF filters | Camera, lens, focal length, aperture, shutter speed, ISO…: at least what the metadata panel shows (issue #4) |
| Fuller vim | Counts before actions (`5j`, `3x`) |
| AI sharpness | Faces, sharp eyes on a portrait |
| Icon set | Drawn icons (SVG) instead of simple shapes |
| Target architecture | LibRaw as a dynamic library, terminal interface (TUI) |
| Other development software | RawTherapee (`rawtherapee-cli`), ART, vkdt (`vkdt-cli`): render and export as with darktable |
| darktable style | Apply a darktable style to an image (`darktable-cli --style`) |
| External plugins | A separate program, in any language (Python for AI…), exchanging JSON with `tri`: analyses, filters, sending. **Example provided: step-by-step rating**, two shortcuts for everything: rating +1 / −1, next / previous colour (configurable order), rejected → neutral → accepted, and the stars / colour filter one notch up / down; nothing moves at the end of the sequence |
| HEIC through the system | macOS (ImageIO) and Windows (HEVC extension) decoders: nothing patented shipped with `tri` |
| Automatic update | Download and install the new version (after signing) |
| Other packages | AppImage or Flatpak (Flathub), winget / Microsoft Store, Homebrew: more systems, more visible |
| Video offload | Ignored on the card today |
| Image analysis | Luminance histogram of flat images, 10-bit HEIC, exposure computed during the RAW render |

## How to help

- **Test**: especially on Windows, and with camera brands other than Fujifilm. Packages
  on the [Releases page](https://github.com/fkinnar/tri/releases), on a **copy** of your
  photos.
- **Report**: through the repository's [*issues*](https://github.com/fkinnar/tri/issues)
  (system, camera, what was expected and what happened).
- **Contribute**: building in [doc/en/build.md](doc/en/build.md), architecture and
  modules in [doc/en/](doc/en/README.md). Rust, egui, LibRaw. A `fix/…` or `feat/…`
  branch and a pull request to `main`, checked by the CI on the three systems.
