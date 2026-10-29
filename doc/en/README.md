# `tri` documentation

`tri` is a fast RAW viewer for culling after a photo shoot (a free equivalent of
FastRawViewer), for Linux, macOS and, if possible, Windows. The context and architecture
decisions are in [`CLAUDE.md`](../../CLAUDE.md); this folder documents the code as it
exists.

## Contents

| Document | Content |
|---|---|
| [manual.md](manual.md) ([français](../fr/manual.md)) | First steps for testers (screenshots, deliberately short); screenshots produced by `scripts/manual-screenshots.sh` |
| [gui.md](gui.md) | egui interface: grid, full screen, actions and shortcuts |
| [build.md](build.md) | Prerequisites, build per platform, environment variables |
| [packaging.md](packaging.md) | Ready-to-install packages (`.dmg`, `.tar.gz`), shipped licenses, macOS signing |
| [config_edit.md](config_edit.md) | Module `tri::config_edit`: list of settings taken from the example, editing the file without losing anything (settings window) |
| [settings.md](settings.md) | Configuration file (TOML): location, sections, errors; module `tri::settings` |
| [config.example.toml](../config.example.toml) | All keys with their default value, commented |
| [media.md](media.md) | Module `tri::media`: folder scanning, RAW / flat images |
| [filter.md](filter.md) | Module `tri::filter`: `Filter` trait, groups, `KindFilter` |
| [cache.md](cache.md) | Module `tri::cache`: persistent cache of thumbnails and previews |
| [preview.md](preview.md) | Module `tri::preview`: computing thumbnails and previews |
| [scheduler.md](scheduler.md) | Module `tri::scheduler`: background work queue |
| [jpeg.md](jpeg.md) | Module `tri::jpeg`: fast JPEG (libjpeg-turbo, pure Rust fallback) |
| [exposure.md](exposure.md) | Module `tri::exposure`: histograms on the sensor data, clipped / blocked, mask |
| [i18n.md](i18n.md) | Module `tri::i18n`: languages (Fluent), `.ftl` catalogs, plurals, `t!` |
| [similarity.md](similarity.md) | Module `tri::similarity`: fingerprints, distances, grouping into stacks (bursts, series) |
| [sort.md](sort.md) | Module `tri::sort`: order of the images (name, shooting date, modification date) |
| [history.md](history.md) | Module `tri::history`: undo, redo, history list (session) |
| [files.md](files.md) | Module `tri::files`: rename, trash, reveal, external editor (pairs and sidecars follow) |
| [sharpness.md](sharpness.md) | Module `tri::sharpness`: sharpness map (focus peaking) at 1:1 |
| [selection.md](selection.md) | Module `tri::selection`: multiple selection (marks, visual mode, action targets) |
| [folder.md](folder.md) | Module `tri::folder`: open folder (images, notes, filtered list) |
| [offload.md](offload.md) | Module `tri::offload`: card offloading (verified copy, history, deletion) |
| [develop.md](develop.md) | Module `tri::develop`: rendering by darktable (plugin driving `darktable-cli`) |
| [versions.md](versions.md) | Module `tri::versions`: crops of a photo (versions), stored in the XMP |
| [export.md](export.md) | Module `tri::export`: JPEG / TIFF / PNG export with presets (darktable), no ProPhoto in 8 bits; `tri::export_log`: export report |
| [fuzzy.md](fuzzy.md) | Module `tri::fuzzy`: fuzzy search (filter by name, folders, help) |
| [overlay.md](overlay.md) | Module `tri::overlay`: overlays (grids, text, logo, format), plugins written in Rust or TOML files |
| [send.md](send.md) | Module `tri::send`: send plugins (AirDrop) |
| [update.md](update.md) | Module `tri::update`: checking for a new version, on demand |
| [watch.md](watch.md) | Module `tri::watch`: watching the open folder (images added, XMP modified elsewhere) |
| [servers.md](servers.md) | Module `tri::servers`: network servers joined by their address (`\\nas\photo`, `smb://`, `ftp://`, `sftp://`, WebDAV, NFS) |
| [remote.md](remote.md) | Module `tri::remote`: local copy of a server the system cannot mount (FTP, SFTP) |
| [places.md](places.md) | Module `tri::places`: places (volumes, home folder, root) and tree |
| [process.md](process.md) | Module `tri::process`: external programs, without a console window on Windows |
| [metadata.md](metadata.md) | Module `tri::metadata`: capture metadata (LibRaw, EXIF), lines to display |
| [notes.md](notes.md) | Module `tri::notes`: a folder's notes in memory, editing, filters on notes |
| [xmp.md](xmp.md) | Module `tri::xmp`: notes (stars, colors, accepted/rejected) in XMP sidecars |
| [bitmap.md](bitmap.md) | Module `tri::bitmap`: common RGB8 image type |
| [flat.md](flat.md) | Module `tri::flat`: decoding JPEG, PNG, TIFF, WebP, HEIC |
| [raw.md](raw.md) | Module `tri::raw`: reading and developing RAWs via LibRaw |
| [tests.md](tests.md) | Organization and running of the tests |
| [licensing.md](licensing.md) | Dependency licenses, HEVC patents, distribution options |

## Code overview

```
tri/
├── build.rs              locates LibRaw, compiles the C shim, emits the link directives
├── csrc/libraw_shim.c    access to LibRaw parameters with no setter in the C API
├── src/
│   ├── lib.rs            library crate (the "engine"): declares the modules
│   ├── main.rs           command-line demo: listing, background previews, cache
│   ├── media.rs          folder scanning, RAW / flat classification
│   ├── filter.rs         Filter trait, groups, KindFilter
│   ├── files.rs          rename, trash, reveal, external editor
│   ├── i18n.rs           languages: Fluent catalogs (locales/, gui/locales/), t!
│   ├── bitmap.rs         Bitmap: common RGB8 image
│   ├── cache.rs          persistent on-disk cache (BLAKE3 keys, JPEG)
│   ├── preview.rs        computing thumbnails, previews and 1:1 (computation settings)
│   ├── scheduler.rs      background work queue: priorities, progress, cancellation
│   ├── selection.rs      multiple selection: marks, visual mode, targets
│   ├── sharpness.rs      sharpness map (focus peaking)
│   ├── settings.rs       configuration file: engine sections, reading
│   ├── jpeg.rs           fast JPEG: libjpeg-turbo (feature) or image crate
│   ├── flat/
│   │   ├── mod.rs        decoding flat images (image crate)
│   │   └── heif.rs       HEIC/HEIF via libheif (feature `heif`)
│   └── raw/
│       ├── ffi.rs        raw declarations of the LibRaw C API (private)
│       └── mod.rs        safe API: RawFile, Thumbnail, RenderOptions, RawError
├── gui/                  tri-gui package: the egui interface (see gui.md)
├── examples/decode.rs    command-line tool: RAW preview / render, flat decoding
└── tests/                integration tests (one file per module)
    └── fixtures/         small synthetic test files (HEIC)
```

The engine (`src/lib.rs` and its modules) depends on no display library: it produces data
(`Bitmap`, file lists) that the interface consumes.

## Generated documentation

Every public item also carries a `///` comment (enforced by `#![warn(missing_docs)]`). The
browsable API documentation is obtained with:

```sh
cargo doc --no-deps --open
```

These Markdown files explain the *why* and how things fit together; `cargo doc` remains
the reference for exact signatures.
