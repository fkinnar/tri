# tri

**A fast RAW viewer for culling a photo shoot**, on macOS, Linux and Windows:
thumbnails of a whole folder in a few seconds, full screen in the camera render or a real
RAW rendering, ratings (stars, colours, picked / rejected) written to XMP and read by
darktable, digiKam, Lightroom…, filters, stacks of similar images, sharpness, RAW
histogram, zoom and loupe, memory card offloading. Fully keyboard driven ("classic" or
"vim" key set), mouse too.

> **Alpha version** (`1.0.0-alpha.7`): try it on a **copy** of your photos. Ratings go
> into `.xmp` files next to the images; renaming, moving or deleting acts on the real
> files (undoable with `Ctrl+Z`, or `u` in vim, during the session).

*[Version française](README.fr.md)*

## Install

**Ready-to-install packages**, on the [Releases page](https://github.com/fkinnar/tri/releases)
(without HEIC, see [doc/en/packaging.md](doc/en/packaging.md)). Between versions, the
**`nightly`** pre-release holds the packages of the last night's `main`:

- **macOS** (`.dmg`): drag `tri.app` into Applications. The app is not signed by Apple
  yet: on first launch macOS refuses it; then open **System Settings → Privacy &
  Security → "Open Anyway"**, or remove the quarantine in a terminal:
  `xattr -dr com.apple.quarantine /Applications/tri.app`.
- **Linux** (Ubuntu 24.04 / Mint 22, Debian 13 or newer): **`.deb`** for Debian, Ubuntu
  and Mint (double-click, or `sudo apt install ./tri-….deb`); **AppImage** to try it
  without installing anything (`chmod +x tri-….AppImage`, then run it;
  `./tri-….AppImage tri --help` for the command line); or the **`.tar.gz`** archive:
  unpack, then `./install.sh` (into `~/.local`, no administrator rights; `--uninstall`
  to remove). `tri` appears in the applications menu.
- **Windows** (`.msi`, 64-bit): run the installer; `tri` appears in the Start menu
  (uninstall from Windows Settings). Not signed yet: SmartScreen warns, click **"More
  info" → "Run anyway"**. Or the portable `.zip`: unpack, then run `tri-gui.exe`.

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
- `Enter` full screen, `z` 100 %, `b` camera render or RAW rendering, `/` search.

## Documentation

Everything is in [`doc/en/`](doc/en/README.md) (in French: [`doc/fr/`](doc/fr/README.md)):
one file per module, the reasons behind the choices, the settings
([`doc/config.example.en.toml`](doc/config.example.en.toml), commented), licences and
patents ([`doc/en/licensing.md`](doc/en/licensing.md)). Release history:
[CHANGELOG.md](CHANGELOG.md). What comes next, and how to help:
[ROADMAP.md](ROADMAP.md).

## Licence

GPL-3.0-or-later: see [LICENSE](LICENSE). The LGPL libraries (LibRaw, libheif) are
linked dynamically; the distributed binaries contain no HEVC decoder (see
[doc/en/licensing.md](doc/en/licensing.md)).
