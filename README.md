# tri

**A fast RAW viewer for culling a photo shoot**, on macOS and Linux (Windows in progress):
thumbnails of a whole folder in a few seconds, full screen in the camera JPEG or a real
RAW rendering, ratings (stars, colours, picked / rejected) written to XMP and read by
darktable, digiKam, Lightroom…, filters, stacks of similar images, sharpness, RAW
histogram, zoom and loupe, memory card offloading. Fully keyboard driven ("classic" or
"vim" key set), mouse too.

> **Alpha version** (`1.0.0-alpha.2`): try it on a **copy** of your photos. Ratings go
> into `.xmp` files next to the images; renaming, moving or deleting acts on the real
> files (undoable with `Ctrl+Z`, or `u` in vim, during the session).

*[Version française](README.fr.md)*

## Install

**Ready-to-install packages**, on the [Releases page](https://github.com/fkinnar/tri/releases)
(without HEIC, see [doc/en/packaging.md](doc/en/packaging.md)):

- **macOS** (`.dmg`): drag `tri.app` into Applications. The app is not signed by Apple
  yet: on first launch macOS refuses it; then open **System Settings → Privacy &
  Security → "Open Anyway"**, or remove the quarantine in a terminal:
  `xattr -dr com.apple.quarantine /Applications/tri.app`.
- **Linux** (`.tar.gz`, Ubuntu 24.04 / Mint 22 or newer): unpack, then `./install.sh`
  (into `~/.local`, no administrator rights; `--uninstall` to remove). `tri` appears in
  the applications menu.
- **Windows**: coming.

**Build from source**:

1. Rust (recent stable, 2024 edition): <https://rustup.rs>
2. The system libraries, see [`prerequisites.sh`](prerequisites.sh):
   - macOS: `brew install libraw libheif cmake pkg-config`
   - Linux Mint 22 / Ubuntu 24.04: `sudo apt install libraw-dev libheif-dev
     libheif-plugin-libde265 zlib1g-dev cmake nasm pkg-config build-essential`
3. `cargo build --release -p tri-gui`, then `target/release/tri-gui [folder]`.

Details (Windows, options, HEIC): [doc/en/build.md](doc/en/build.md).

## First steps

- [First steps](doc/en/manual.md) (short on purpose: the rest is yours to discover),
  [premiers pas](doc/fr/manual.md) in French.
- `F1` or `?`: every key, with a search. `Ctrl+,`: the settings (key set, "neutral grey"
  theme, language…).
- Ratings: `0`–`5` stars, `6`–`9` colours, `Shift+P` / `Shift+X` picked / rejected.
- `Enter` full screen, `z` 100 %, `b` camera JPEG or RAW rendering, `/` search.

## Documentation

Everything is in [`doc/en/`](doc/en/README.md) (in French: [`doc/fr/`](doc/fr/README.md)):
one file per module, the reasons behind the choices, the settings
([`doc/config.example.toml`](doc/config.example.toml), commented, in French for now),
licences and patents ([`doc/en/licensing.md`](doc/en/licensing.md)). Release history:
[CHANGELOG.md](CHANGELOG.md) (in French).

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). The LGPL libraries (LibRaw, libheif) are
linked dynamically; the distributed binaries contain no HEVC decoder (see
[doc/en/licensing.md](doc/en/licensing.md)).
