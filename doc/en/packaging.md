# Ready-to-install packages

Decided on 2026-10-19: packages nobody has to compile, for the alpha testers and then the
public beta. A single script, `scripts/package.py`, run on a machine of the target
system (no cross-compilation); the CI runs it on macOS and Linux.

```sh
python3 scripts/package.py              # build (target/package) then package
python3 scripts/package.py --no-build   # reuse the last build
python3 scripts/package.py --no-archive # folder only, no .dmg / .tar.gz
python3 scripts/package.py --sign "Developer ID Application: …"   # macOS, signed
```

Result in `target/dist/`: `tri-<version>-macos-<arch>.dmg`,
`tri-<version>-linux-<arch>.tar.gz`, or `tri-<version>-windows-x64.msi` and `.zip`.
Prerequisites: those of the build
([build.md](build.md)) and [`cargo-about`](https://github.com/EmbarkStudios/cargo-about)
(`cargo install cargo-about --locked --features cli`).

## Nightly builds

Decided on 2026-10-22: while in alpha, the CI (`.github/workflows/ci.yml`) also runs
**every night** (1:17 UTC, not on the hour: GitHub delays scheduled jobs there by hours) when `main` has changed since the last one (the `nightly` tag
is compared with the current commit): tests on the three systems, packages, then the
**`nightly` Release** (pre-release) replaced with the day's packages and the last commits
as notes. Its tag is not a version number: the update check ignores it. These binaries
carry `TRI_BUILD_CHANNEL=nightly` (`release` on a `v…` tag) and say so in the help
([update.md](update.md)); so do their packages, in their name
(`tri-1.0.0-alpha.7-nightly-20261025-d16c32f-macos-arm64.dmg`, `package_version` in
`scripts/package.py`, 2026-10-25: the nightly `.dmg` had the published version's name).
For the beta (planned): a `candidate` branch for release
candidates and a nightly build branch.

## What is shipped, and why

- **Without HEIC** (`--no-default-features`, with libjpeg-turbo): HEVC patents, see
  [licensing.md](licensing.md).
- **Separate build** (`target/package`): the development binary, with HEIC, is not
  overwritten.
- **LibRaw and its dependencies shipped with `tri`**, as separate libraries (`.dylib`,
  `.so`): nobody has to install Homebrew or the right version of LibRaw, and the LGPL is
  respected (replaceable library). Their licenses are copied from the build system
  (Homebrew formulae, `/usr/share/doc/<paquet>/copyright` on Debian / Ubuntu) into
  `licenses/`, with a `README.md` giving the versions and the address of the **LibRaw
  source code** (an LGPL obligation for a shipped library).
- **Rust crate notices**: `licenses/rust-crates.html`, produced by `cargo about`
  (accepted licenses in `about.toml`, template `scripts/about.hbs`).
- `LICENSE` (GPL-3.0-or-later), `README.md`, `CHANGELOG.md`, `CHANGELOG.fr.md`.

## macOS: `tri.app` in a `.dmg`

- `Contents/MacOS/tri-gui` (the interface) and `Contents/MacOS/tri` (command line).
- `Contents/Frameworks/`: LibRaw, little-cms2, libomp, libjpeg (LibRaw's).
  Every reference is rewritten as `@rpath/…` (`install_name_tool`), with the binaries'
  `@rpath` pointing to `../Frameworks`: checked with `DYLD_PRINT_LIBRARIES`, nothing is
  loaded from Homebrew.
- `Info.plist` (identifier `io.github.fkinnar.tri`, photography category), icon
  `tri.icns` made from `gui/assets/icon-1024.png` (`sips`, `iconutil`).
- The `.dmg` contains the app, a link to Applications and the license files.
- Command line from the app: `/Applications/tri.app/Contents/MacOS/tri`.

### Signing and notarization

On Apple Silicon, every binary must be signed; modifying the libraries
(`install_name_tool`) invalidates their signature. By default the script therefore signs
**"ad hoc"** (without an identity). That is enough to launch the app, but a downloaded
`.dmg` is quarantined: on first launch macOS refuses, then **System Settings → Privacy &
Security → "Open Anyway"** (right-click → Open is no longer enough since macOS 15), or in
a terminal `xattr -dr com.apple.quarantine /Applications/tri.app` (removes only the
quarantine; `xattr -cr` clears all extended attributes, broader than needed). Acceptable
for the alpha (testers are warned).

For the **public beta**, signing and notarization will be needed:

1. Apple Developer Program (99 USD a year), **Developer ID Application** certificate.
2. `package.py --sign "Developer ID Application: Nom (ÉQUIPE)"`: signing with the
   hardened runtime (`--options runtime`) and the timestamp, both required by
   notarization. The libraries are signed with the same identity (no library validation
   exception to request).
3. Notarization of the `.dmg`: `xcrun notarytool submit <dmg> --keychain-profile <profil>
   --wait`, then `xcrun stapler staple <dmg>` (the ticket travels with the file).
4. In CI: `.p12` certificate and password as secrets, imported into a temporary
   keychain; notarization credentials as secrets too.

## Linux: `.tar.gz` archive and `install.sh`

```
tri-<version>-linux-<arch>/
  bin/tri-gui, bin/tri
  lib/          LibRaw and dependencies (libraw, liblcms2, libjpeg, libgomp)
  share/applications/tri.desktop, share/icons/hicolor/256x256/apps/tri.png
  licenses/, install.sh, LICENSE, README.md, CHANGELOG.md, CHANGELOG.fr.md
```

- The binaries look for their libraries in `../lib` (`$ORIGIN/../lib`, set at build
  time), then in the system. As **`RPATH`** and not `RUNPATH`
  (`--disable-new-dtags`): `RUNPATH` only applies to direct dependencies, and LibRaw's
  dependencies (libjpeg, lcms2) were not found on a distribution without LibRaw. The
  script checks the `RPATH` (`readelf -d`).
- **Checked on 2026-10-19** (archive built in an Ubuntu 24.04 container, arm64):
  installed with `install.sh` on **Debian 13** and **Fedora 42** without LibRaw, all
  libraries loaded from the package, RAF read (`tri <dossier> --metadata`).
  The interface remains to be tried on a real desktop (Mint 22, x86_64, CI archive).
- **Why ship LibRaw**: its library number changes from one distribution to another
  (`libraw.so.23` on Ubuntu 24.04, `.so.24`/`.25` elsewhere); a binary linked against one
  does not start with the other. The less universal dependencies (lcms2, libjpeg,
  libgomp) follow it; the rest (glibc, libstdc++, X11 / Wayland, Vulkan / OpenGL) comes
  from the system. List in the script's `LINUX_BUNDLED`; the script checks with `ldd`
  that nothing is missing.
- **Target distributions**: built on Ubuntu 24.04, the archive needs a glibc at least as
  recent (2.39): Ubuntu 24.04 and later, Linux Mint 22, Debian 13, recent Fedora. For
  older distributions: build on an older one.
- `install.sh` copies everything into `~/.local/lib/tri` (or `--prefix DOSSIER`), creates
  the links `~/.local/bin/tri-gui` and `tri`, the launcher (`tri.desktop`, whose `Exec`
  points to the installation) and the icon; `install.sh --uninstall` removes it all and
  updates the icon cache in `~/.local` (otherwise it would still list the removed icon and
  hide that of a `tri` installed elsewhere). No administrator rights needed. `tri`'s data
  (settings, cache) is never touched.
- **Moving from the archive to the `.deb`**: first `~/.local/lib/tri/install.sh
  --uninstall`, otherwise the launcher in `~/.local` takes precedence over the package's.
  If the icon is gone after an uninstall by an older `install.sh`:
  `gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor`, then log out and back in.
- The window declares the application id `tri` (Wayland), tied to the `tri.desktop`
  launcher for the icon and name.

### `.deb` and AppImage (2026-10-25)

Made from the same tree as the archive (same minimum glibc, same bundled libraries),
right after it:

- **`.deb`** (`package_deb`, `dpkg-deb`): everything in `/opt/tri`, links
  `/usr/bin/tri-gui` and `/usr/bin/tri`, launcher and icon in `/usr/share`; the RPATH
  `$ORIGIN/../lib` follows the link. Debian version: `1.0.0~alpha.7` (`~`: before the
  final version), nightly `1.0.0~alpha.7+nightly.20261025.d16c32f`. Depends on glibc 2.39
  and the C++ runtime; display (OpenGL, X11, Wayland) only recommended, loaded by winit
  at run time.
- **AppImage** (`package_appimage`, `appimagetool`, `APPIMAGETOOL` or the `PATH`; without
  it, skipped): the tree in `usr/`, `AppRun` (the interface; `tri` as first argument: the
  command line), launcher and icon at the root. The CI downloads `appimagetool` 1.9.1;
  `APPIMAGE_EXTRACT_AND_RUN=1`: no FUSE needed.
- Checked on 2026-10-25 in an Ubuntu 24.04 container (arm64) with dummy binaries:
  `dpkg -i` (commands in `/usr/bin`, launcher, icon), `dpkg -r`, AppImage run (interface
  and command line).

## Windows

Added on 2026-10-21 (so that "a normal person" can install it): an **`.msi`
installer** and a **portable `.zip`**, built by the CI (`windows-latest`).

```
tri-<version>-windows-x64/
  tri-gui.exe, tri.exe
  raw_r.dll and its vcpkg dependencies, Visual C++ runtime (msvcp140.dll, vcruntime140*.dll…)
  tri.ico, licenses/, LICENSE, README.md, CHANGELOG.md, CHANGELOG.fr.md
```

- **Installer** (WiX 5, `dotnet tool install --global wix --version 5.0.2`; the WiX
  licence, MS-RL, only covers the tool): installs into `Program Files\tri` (per machine,
  administrator confirmation), **Start menu** shortcut with the icon, uninstall from
  Windows Settings. Fixed `UpgradeCode`: a new version replaces the previous one. The MSI
  version is numeric (`1.0.0` for `1.0.0-alpha.2`); `AllowSameVersionUpgrades` still
  replaces it.
- **DLLs**: found by reading the import tables of the `.exe` and of each DLL (small PE
  reader in the script, no `dumpbin`), looked up in `LIBRAW_DIR\bin` (vcpkg) and in the
  **Visual C++ redistributable** folders of Visual Studio (`vswhere`); system DLLs
  (`System32`, `api-ms-*`) stay out. The runtime is shipped next to the executable
  ("app-local" deployment, allowed by Microsoft for these files): many PCs do not have
  it. The script stops if a DLL is found nowhere.
- **Licences**: those of the installed vcpkg ports (`share\<port>\copyright`), LibRaw
  source address (LGPL), Rust crates notices, as on the other systems.
- **No console window** in release (`windows_subsystem = "windows"`, `gui/src/main.rs`).
- **Not signed**: SmartScreen warns on first launch ("More info" → "Run anyway"). Signing
  (code-signing certificate) for the public beta, like the Apple notarisation.
- **Checked on 2026-10-21**: the CI `.msi` installed on a clean Windows (nothing
  installed, no Visual C++ runtime); the application starts and shows the images.
  Still to go through every feature.
