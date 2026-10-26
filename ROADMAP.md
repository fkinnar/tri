# tri roadmap

*[Version française](ROADMAP.fr.md)*

> **Frozen on 2026-10-21.** The beta 1 list no longer grows: any new idea goes to "after
> V1", except bugs. Feedback goes through the repository's *issues*;
> each topic below has its own, linked next to its name.

## Already there (alpha 7)

Grid and full screen (camera render, RAW render, 1:1, continuous zoom, loupe), XMP
ratings compatible with darktable / Lightroom / digiKam, filters (ratings, sharpness,
exposure, invert, name search), stacks of similar photos, sharpness (score, map),
histogram and exposure zones, comparison, multiple selection, files and folders (rename,
trash, copy / move, drag and drop, RAW+JPEG pairs optional), undo, card offload, export
presets, cropped versions, darktable development, overlays, settings in the interface,
classic and vim shortcuts, mouse selection (Shift+click, Ctrl+click), update check and
issue report, sort by name or date, named favourites,
French and English. Packages for macOS (`.dmg`), Linux (`.tar.gz`, `.deb`, AppImage) and Windows (`.msi`,
`.zip`), and nightly builds ("nightly" Release).

## Beta 1 (public)

| Topic | Details |
|---|---|
| Windows testers' feedback [#124](https://github.com/fkinnar/tri/issues/124) | Tries on real machines (paths, drives, eject, trash, AZERTY keyboard, high-density screens), bugs found |
| Testers' manual [#125](https://github.com/fkinnar/tri/issues/125) | Screenshots redone with real photos (RAW, focus point, histogram) |
| Large folders on a slow disk [#126](https://github.com/fkinnar/tri/issues/126) | Folder read in the background with visible progress (the window froze while listing thousands of files and their sidecars on an external hard drive) |
| Sort by rating, colour [#127](https://github.com/fkinnar/tri/issues/127) | To design with the stacks and the sharpness judgement, which rely on the shooting order |
| Clearing the cache in the interface [#128](https://github.com/fkinnar/tri/issues/128) | Whole cache, one level, or the open folder (today: `tri <folder> --clear-cache`), with the space used |
| RapidRAW development [#114](https://github.com/fkinnar/tri/issues/114) | Render and export through `rapidraw export` (command line since RapidRAW 1.6), with the edits of its `.rrdata` sidecars, as with darktable |

The beta is **not signed**: macOS and Windows warn on first launch, the steps are in the
README.

## V1

| Topic | Details |
| --- | --- |
| Signing [#129](https://github.com/fkinnar/tri/issues/129) | macOS: Developer ID and notarisation; Windows: SignPath Foundation (free for open source). No more warning on first launch |
| Focus point for other brands [#130](https://github.com/fkinnar/tri/issues/130) | Every RAW LibRaw reads is already supported (display, histogram, metadata); the focus point marker is Fujifilm only for now. To add: Canon, Nikon, Sony, OM, Panasonic, checked on samples from raw.pixls.us |
| Ratings in other software [#131](https://github.com/fkinnar/tri/issues/131) | Check digiKam, RawTherapee, ART; Lightroom and Capture One (testers) |
| Sharpness in comparison [#132](https://github.com/fkinnar/tri/issues/132) | Sharpness map in side-by-side comparison |

## After V1

| Topic | Details |
| --- | --- |
| Keywords [#133](https://github.com/fkinnar/tri/issues/133) | Hierarchical, compatible with other software; interface to be mocked up first |
| Folder merge [#134](https://github.com/fkinnar/tri/issues/134) | Move a folder into a folder of the same name |
| Crop to development software [#135](https://github.com/fkinnar/tri/issues/135) | Write a version's crop into the sidecar, in the format of Lightroom (`crs:Crop…`), darktable, Capture One |
| Offload [#136](https://github.com/fkinnar/tri/issues/136) | Folder and file name templates, multi-day cards |
| Viewing adjustments [#137](https://github.com/fkinnar/tri/issues/137) | Exposure, black and white, contrast, exposure matching (non-destructive) |
| Complex filters [#138](https://github.com/fkinnar/tri/issues/138) | Dedicated window or panel |
| EXIF filters [#4](https://github.com/fkinnar/tri/issues/4) | Camera, lens, focal length, aperture, shutter speed, ISO…: at least what the metadata panel shows (issue #4) |
| Fuller vim [#139](https://github.com/fkinnar/tri/issues/139) | Counts before actions (`5j`, `3x`) |
| AI sharpness [#140](https://github.com/fkinnar/tri/issues/140) | Faces, sharp eyes on a portrait |
| Icon set [#141](https://github.com/fkinnar/tri/issues/141) | Drawn icons (SVG) instead of simple shapes |
| Target architecture [#142](https://github.com/fkinnar/tri/issues/142) | LibRaw as a dynamic library, terminal interface (TUI) |
| Other development software [#115](https://github.com/fkinnar/tri/issues/115), [#143](https://github.com/fkinnar/tri/issues/143) | RawTherapee (`rawtherapee-cli`), ART, vkdt (`vkdt-cli`): render and export as with darktable |
| darktable style [#144](https://github.com/fkinnar/tri/issues/144) | Apply a darktable style to an image (`darktable-cli --style`) |
| External plugins [#145](https://github.com/fkinnar/tri/issues/145) | A separate program, in any language (Python for AI…), exchanging JSON with `tri`: analyses, filters, sending. **Example provided: step-by-step rating**, two shortcuts for everything: rating +1 / −1, next / previous colour (configurable order), rejected → neutral → accepted, and the stars / colour filter one notch up / down; nothing moves at the end of the sequence |
| HEIC through the system [#146](https://github.com/fkinnar/tri/issues/146) | macOS (ImageIO) and Windows (HEVC extension) decoders: nothing patented shipped with `tri` |
| Automatic update [#147](https://github.com/fkinnar/tri/issues/147) | Download and install the new version (after signing) |
| Other packages [#148](https://github.com/fkinnar/tri/issues/148) | Flatpak (Flathub), winget / Microsoft Store, Homebrew: more systems, more visible |
| Video offload [#149](https://github.com/fkinnar/tri/issues/149) | Ignored on the card today |
| Image analysis [#150](https://github.com/fkinnar/tri/issues/150) | Luminance histogram of flat images, 10-bit HEIC, exposure computed during the RAW render |
| Ideas from the issues | Import preview [#42](https://github.com/fkinnar/tri/issues/42), group from an image [#45](https://github.com/fkinnar/tri/issues/45), exclude similar images from the sharpness / exposure filters [#46](https://github.com/fkinnar/tri/issues/46), several screens [#53](https://github.com/fkinnar/tri/issues/53), viewer mode [#67](https://github.com/fkinnar/tri/issues/67), slideshow [#68](https://github.com/fkinnar/tri/issues/68), collections and optional catalogues [#117](https://github.com/fkinnar/tri/issues/117), MCP server [#121](https://github.com/fkinnar/tri/issues/121), REST API [#122](https://github.com/fkinnar/tri/issues/122) |

## How to help

- **Test**: especially on Windows, and with camera brands other than Fujifilm. Packages
  on the [Releases page](https://github.com/fkinnar/tri/releases), on a **copy** of your
  photos.
- **Report**: through the repository's [*issues*](https://github.com/fkinnar/tri/issues)
  (system, camera, what was expected and what happened).
- **Contribute**: building in [doc/en/build.md](doc/en/build.md), architecture and
  modules in [doc/en/](doc/en/README.md). Rust, egui, LibRaw. A `fix/…` or `feat/…`
  branch and a pull request to `main`, checked by the CI on the three systems.
