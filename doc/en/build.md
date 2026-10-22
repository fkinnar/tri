# Building and platforms

## Prerequisites

To install **before the first build** (a single command per platform):

| Platform | Installation |
|---|---|
| Ubuntu ≥ 24.04 / Mint ≥ 22 | `sudo apt install libraw-dev libheif-dev libheif-plugin-libde265 zlib1g-dev cmake nasm pkg-config build-essential` |
| macOS (Homebrew) | `brew install libraw libheif cmake pkg-config` (+ Xcode command line tools: `xcode-select --install`) |
| Windows (untested) | `vcpkg install libraw:x64-windows libheif:x64-windows`, CMake and NASM, then `LIBRAW_DIR` (see below) |

Plus Rust itself ([rustup](https://rustup.rs/), edition 2024). The script
[`prerequisites.sh`](../../prerequisites.sh) recalls these commands. Linux command checked on
Mint 22.3 (2026-10-16). macOS command checked on 2026-10-17: `cmake` is indeed
needed since libjpeg-turbo is built with the project (on Apple Silicon, NASM
is not required: the ARM SIMD optimisations do not need it).

### Why these packages

`tri` links two system libraries:

- [LibRaw](https://www.libraw.org/) (C API), for RAWs;
- [libheif](https://github.com/strukturag/libheif) **≥ 1.17**, for HEIC/HEIF, with its
  HEVC decoding plugin (libde265).

libjpeg-turbo (`turbojpeg` feature) is **not** taken from the system: it is built
from the sources embedded by the `turbojpeg-sys` crate (see below), which requires
CMake and NASM.

A C compiler is also needed, for the small shim `csrc/libraw_shim.c`.

`zlib1g-dev`: on Ubuntu/Mint, `libheif.pc` declares zlib in `Requires.private`;
without this package, pkg-config rejects libheif ("Package 'zlib', required by 'libheif', not
found") even though `libheif-dev` is installed.

### libheif API version

`libheif-rs` 3 targets the very latest API by default (libheif ≥ 1.23), absent from
LTS distributions (Mint 22 / Ubuntu 24.04: 1.17.6). `Cargo.toml` therefore pins the feature
`v1_17`, the minimum required by `tri`. Raise it only if a more recent libheif function
is used.

### libheif too old

Debian 12 and Ubuntu 22.04 provide a libheif < 1.17, rejected at build time. In that
case, `libheif-rs` can build libheif itself (CMake required), by enabling its
`embedded-libheif` feature in `Cargo.toml`:

```toml
libheif-rs = { version = "3", default-features = false, features = ["v1_17", "embedded-libheif"], optional = true }
```

For a **distributed** binary, see [licensing.md](licensing.md): Homebrew's libheif
is linked to x265 (GPL) and HEVC is patented.

libheif is located by the `libheif-sys` crate itself (pkg-config, or vcpkg on Windows
MSVC). `tri`'s `build.rs` only deals with LibRaw.

## Workspace

The repository is a Cargo workspace with two packages: `tri` (engine + command-line demo, at
the root) and `tri-gui` (egui interface, in `gui/`). The engine does not depend on egui.

```sh
cargo build --release --workspace             # everything
cargo run --release -p tri-gui -- <folder>    # the interface
cargo test --workspace                        # all tests
```

`tri-gui` forwards the `heif` and `turbojpeg` features to the engine (`--no-default-features`
therefore applies to both).

On Linux, the interface (eframe: winit + wgpu) needs **no additional development
package**: the X11/Wayland/xkbcommon libraries and the
Vulkan/OpenGL driver are loaded at runtime, not linked (checked on Mint 22.3 without
`libxkbcommon-dev`, `libwayland-dev` or `libxcb-*-dev`). A normal graphical session is enough.

## What `build.rs` does

1. **Locates LibRaw.**
   - If `LIBRAW_DIR` is set: we use `$LIBRAW_DIR/include` and `$LIBRAW_DIR/lib`, and
     the `raw` lib (name overridable with `LIBRAW_LIB`, e.g. `libraw`). Outside MSVC, the
     C++ standard library is added.
   - Otherwise: `pkg-config libraw`.
2. **Compiles the C shim** (`csrc/libraw_shim.c`) with the `cc` crate, using the same
   header paths: the `libraw_data_t` struct is thus always seen with the right layout.
3. **Emits the link directives**, *after* the shim: with `ld` (Linux), a static
   library must appear before those it depends on.

### macOS specifics

`pkg-config` returns `-lstdc++` (GNU libstdc++), whereas macOS uses `libc++`.
`build.rs` replaces `stdc++` with `c++` when the target is macOS. On Linux, `stdc++` is
correct as is.

### Windows

No pkg-config in practice. Prerequisites: Rust (with the Visual Studio C++ Build
Tools that rustup asks for), Git, CMake, NASM (libjpeg-turbo SIMD), and LibRaw through
vcpkg. In `cmd`:

```
winget install NASM.NASM
git clone https://github.com/microsoft/vcpkg C:\vcpkg
C:\vcpkg\bootstrap-vcpkg.bat
C:\vcpkg\vcpkg install libraw:x64-windows
set PATH=%PATH%;C:\Program Files\NASM;C:\vcpkg\installed\x64-windows\bin
set LIBRAW_DIR=C:\vcpkg\installed\x64-windows
set LIBRAW_LIB=raw_r
cargo run --release -p tri-gui --no-default-features --features tri/turbojpeg,tri-gui/turbojpeg
```

`LIBRAW_LIB`: the name of the library in `lib\` (`raw_r.lib` with LibRaw 0.22 from
vcpkg, otherwise `raw`). Without HEIC (`--no-default-features`): libheif would also
have to come from vcpkg. Plan 30 to 40 GB of disk (Build Tools, vcpkg, `target\`).
Easier for a tester: the `.msi` built by the CI ([packaging.md](packaging.md)).

The LibRaw DLL must be findable at runtime (in the `PATH` or next to the executable).
File paths are passed to LibRaw in UTF-16 (`libraw_open_wfile`), which handles
non-ASCII names.

## Cargo features

| Feature | Default | Effect |
|---|---|---|
| `turbojpeg` | enabled | JPEG via libjpeg-turbo, built from its sources and linked statically. Disabled, falls back to the pure Rust decoder/encoder of the `image` crate, slower: see [jpeg.md](jpeg.md) |
| `heif` | enabled | HEIC/HEIF decoding via libheif. Disabled (`--no-default-features`), libheif is no longer required nor linked: see [flat.md](flat.md) and [licensing.md](licensing.md) |

With `--no-default-features`, only LibRaw remains needed as a system library.

### libjpeg-turbo built with the project

The `turbojpeg` 1.x crate uses the TurboJPEG 3 API and requires **libjpeg-turbo ≥ 3.0**. LTS
distributions still provide 2.1 (Mint 22 / Ubuntu 24.04: 2.1.5), rejected at
build time. Hence the choice (2026-10-16) to enable the `cmake` feature of `turbojpeg-sys` on
all platforms: it builds the sources it embeds (libjpeg-turbo 3.1.0) as a
**static** library. Same version everywhere, no libjpeg-turbo package to install,
Windows included. Cost: CMake and NASM (x86 SIMD extensions; `require-simd` makes the
build fail rather than silently produce a slow version), and ~20 s of compilation
the first time. Permissive licences, static linking with no constraint other than
attribution (see [licensing.md](licensing.md)).

To use a system libjpeg-turbo ≥ 3.0 anyway, `turbojpeg-sys` reads
environment variables:

```sh
TURBOJPEG_SOURCE=pkg-config cargo build --release        # via pkg-config (libturbojpeg)
TURBOJPEG_SOURCE=explicit TURBOJPEG_LIB_DIR=… TURBOJPEG_INCLUDE_DIR=… cargo build --release
```

## Environment variables

| Variable | Role |
|---|---|
| `LIBRAW_DIR` | Root of a LibRaw installation (`include/`, `lib/`); bypasses pkg-config |
| `LIBRAW_LIB` | Name of the library to link with `LIBRAW_DIR` (default: `raw`) |
| `TRI_TEST_RAW` | RAW file used by the integration tests (see [tests.md](tests.md)) |
| `TRI_TEST_HEIC` | Real HEIC (iPhone) used by an optional integration test |

## Checking the code for Windows from macOS or Linux (2026-10-19)

Without a Windows machine, we at least check that all the Rust code (engine, interface,
tests) compiles and passes clippy for Windows:

```sh
rustup target add x86_64-pc-windows-msvc        # once
TRI_CHECK_ONLY=1 CARGO_FEATURE_PURE=1 cargo clippy --workspace --all-targets \
  --no-default-features --target x86_64-pc-windows-msvc --target-dir /tmp/tri-win
```

- `cargo check` / `clippy` link nothing: LibRaw is not needed;
- `TRI_CHECK_ONLY`: `build.rs` skips the LibRaw C shim (it would need the target's
  compiler);
- `CARGO_FEATURE_PURE`: blake3 without its assembly (same reason), for the
  check only;
- `--no-default-features`: without libjpeg-turbo or libheif, which are compiled in C.

First pass on 2026-10-19: everything compiles, clippy clean (one test function
reserved for Unix marked as such). Still to check on a real machine: building
the C libraries (LibRaw via `LIBRAW_DIR`, libjpeg-turbo, libheif) and
running (paths, drive letters, ejection, trash).

## Builds on macOS, Linux and Windows: GitHub Actions (2026-10-19)

`.github/workflows/ci.yml`: on all three systems, dependencies (apt, Homebrew, vcpkg
for LibRaw on Windows), `cargo fmt --check`, clippy (`-D warnings`), tests, release
binaries, uploaded as artifacts with `LICENSE`, `README.md`, `CHANGELOG.md`, `CHANGELOG.fr.md`. Without HEIC
(`heif` feature removed: HEVC patents, see `licensing.md`), with libjpeg-turbo.

Run by hand (Actions tab, or `gh workflow run ci.yml`), on each version
(`git tag v1.0.0-alpha.5 && git push origin v1.0.0-alpha.5`: Release with the packages,
[packaging.md](packaging.md)), each night if `main` changed ("nightly" Release), and on
each pull request to `main`. On a pull request the packages are skipped (checks and tests
only, faster), and a new push cancels the previous run of the same pull request.

### Branches and pull requests (2026-10-22, from 1.0.0-alpha.5 on)

No direct push to `main`: it stays releasable. One branch per topic, `fix/<issue or
topic>` for a fix, `feat/<topic>` for a feature (or documentation), pushed, then a pull
request to `main` ("Fixes #n" in its description closes the issue on merge). `main` is
protected: merging through a pull request only, once the three systems (`ubuntu-24.04`,
`macos-14`, `windows-2022` jobs) pass. Before pushing, as usual: `cargo fmt`, clippy
and the tests around the changed code.
