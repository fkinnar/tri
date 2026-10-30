# Licences, patents and distribution

> **Disclaimer:** this document sums up what we could check and what is commonly accepted.
> It is not legal advice. Before any distribution of binaries, especially commercial or by
> an organisation, these points must be checked by a professional.

The goal is to publish `tri` as open source if it succeeds. This document lists what that
implies, so that the choice of licence and of distribution method are made knowingly and not
discovered after the fact.

**Summary:**

1. The Rust code and the crates used impose almost nothing (MIT, Apache-2.0…).
2. LibRaw, libheif and libde265 are under the **LGPL**: dynamic linking is compatible with
   any project licence, provided a few obligations are met if they are shipped with the
   binary.
3. **The real issue is HEVC** (the codec of iPhone HEICs), covered by **patents**: this is
   a problem *separate* from the code licence. It concerns the distribution of *binaries*
   able to decode HEVC, not the source code of `tri`.
4. Homebrew's libheif embeds **x265 (GPL)**. It must not be redistributed as is with a
   non-GPL `tri` binary.

---

## 1. Dependency licences

### System libraries (dynamically linked)

Collected on 2026-10-16 from the installed Homebrew packages (`brew info`) and the actual
links (`otool -L`).

| Library | Version | Licence | Role in `tri` |
|---|---|---|---|
| LibRaw | 0.22.2 | **LGPL-2.1-only OR CDDL-1.0** (either) | RAW decoding |
| ↳ libomp (OpenMP) | — | Apache-2.0 WITH LLVM-exception | LibRaw's internal parallelism |
| ↳ little-cms2 | 2.19.1 | MIT | LibRaw's colour management |
| ↳ jpeg-turbo | 3.2.0 | IJG AND Zlib AND BSD-3-Clause | JPEG in LibRaw |
| libheif | 1.23.6 | **LGPL-3.0-or-later** | HEIF/HEIC container |
| ↳ libde265 | 1.1.3 | **LGPL-3.0-or-later** | **HEVC** decoder (patents, see §2) |
| ↳ **x265** | 4.3 | **GPL-2.0-or-later** | HEVC encoder: **useless for `tri`**, but linked by Homebrew's libheif |
| ↳ aom | 3.15.2 | BSD-2-Clause | AV1 (AVIF), unused for now |
| libjpeg-turbo (`libturbojpeg`) | 3.1.0 | IJG AND Zlib AND BSD-3-Clause | Fast JPEG (`turbojpeg` feature), no active patent. **Compiled and linked statically** (sources embedded by `turbojpeg-sys`, see [build.md](build.md)): permissive licences, the only obligation is attribution (IJG and BSD notices in the documentation of a distributed binary) |

**LibRaw**: we take the LGPL-2.1 option. The CDDL is incompatible with the GPL; the LGPL
keeps every door open.

### Rust crates (compiled into the binary)

Collected via `cargo metadata` (and completed with each addition: `blake3` is under CC0-1.0 OR
Apache-2.0, `turbojpeg` and `turbojpeg-sys` under Unlicense OR MIT, `cmake` — build tool only — under MIT OR Apache-2.0, `serde` and `toml` — configuration
file — under MIT OR Apache-2.0, like their dependencies `toml_parser`,
`toml_datetime`, `serde_spanned`, except `winnow`, under MIT; `objc2-foundation` — macOS, location of a file moved to the trash for
undo, already present through `trash` — under MIT; `objc2` and `objc2-app-kit` — macOS,
sending by AirDrop (share sheet, `tri::send`), same project (madsmtm/objc2, active),
already present through winit — under Zlib OR Apache-2.0 OR MIT; `windows-sys` — Windows,
ejecting a volume (`places::eject`), Microsoft (microsoft/windows-rs, very active),
already present through winit and others — under MIT OR Apache-2.0; `toml_edit` — settings
window: edit the file without losing comments or layout, same
maintainers as `toml` (toml-rs), very active, already present as an indirect dependency — under
MIT OR Apache-2.0, with `indexmap` and `toml_writer` under MIT OR Apache-2.0; `quick-xml` — XMP sidecars —
under MIT, with `memchr` under Unlicense OR MIT; `kamadak-exif` — EXIF of flat
images and of the embedded JPEG of RAWs — under BSD-2-Clause, with `mutate_once` under
BSD-2-Clause; `trash` — system trash, maintained, version of 2026-09-13 — under
MIT, with `objc2`, `objc2-foundation`, `objc2-encode`, `urlencoding` under MIT and
`percent-encoding`, `scopeguard`, `once_cell`, `chrono`, `windows` under MIT OR
Apache-2.0; `fluent-bundle`, `fluent-syntax`, `fluent-langneg`, `intl-memoizer`,
`intl_pluralrules`, `unic-langid` — translations — under MIT OR Apache-2.0, with `tinystr`
under Unicode-3.0 and `self_cell` under Apache-2.0 OR GPL-2.0-only, of which we take
Apache-2.0; `sys-locale` — system language — under MIT OR Apache-2.0; `moxcms` —
ICC profiles of exports without darktable, pure Rust colour management, already compiled by
`image` (same version), actively maintained (0.9.1 of 2026-09-15) — under BSD-3-Clause OR
Apache-2.0; `serde_json` — versions (crops) in the XMP,
one JSON object per element, dtolnay, very well maintained (1.0.154 of 2026-10-11) — under MIT OR
Apache-2.0; `mdns-sd` — network neighbourhood (Bonjour / mDNS), pure Rust without an
async runtime, keepsimple1/mdns-sd, active (0.21.6 of 2026-10-19), without its default
features — under Apache-2.0 OR MIT, with `flume` under Apache-2.0 / MIT, `if-addrs` under
MIT OR BSD-3-Clause, `socket-pktinfo` under MIT, `mio`, `socket2`, `fastrand` under MIT
(OR Apache-2.0)): the 89 crates
of the engine's
dependency tree on macOS (`cargo tree -p tri -e normal,build`, recounted on
2026-10-17) are under permissive
licences: MIT, Apache-2.0, BSD, Zlib, Unlicense, almost always dual-licensed
`MIT OR Apache-2.0`. None is copyleft. `libheif-rs` and `libheif-sys` are under MIT:
they are *bindings*, libheif itself remains LGPL.

**Interface (`tri-gui`)**: egui, eframe, winit, wgpu and their dependencies, ~370 more
crates, all permissive (MIT, Apache-2.0, BSD, Zlib, ISC, BSL-1.0, Unicode-3.0). Three
dual-licensed crates, where we take the permissive option: `r-efi` (MIT OR Apache-2.0
OR LGPL-2.1-or-later) and `self_cell` (Apache-2.0 OR GPL-2.0-only). egui's default fonts
(`epaint_default_fonts`) are under **OFL-1.1** and the **Ubuntu Font Licence**: font
licences that allow embedding them in software under any licence,
with their notice. Development only: `egui_kittest` (MIT OR Apache-2.0).

Practical obligation: reproduce the copyright and licence notices of these crates in the
distribution. A tool like [`cargo-about`](https://github.com/EmbarkStudios/cargo-about)
generates this file automatically. **Adopted on 2026-10-19** (MIT OR Apache-2.0, Embark
Studios, version 0.9.2 of August 2026: maintained): build tool only, nothing
goes into the binary; `about.toml`, `scripts/package.py`, see
[packaging.md](packaging.md).

### What the LGPL requires (LibRaw, libheif, libde265)

With **dynamic linking** (what `tri` does), the LGPL imposes nothing on the licence of
`tri` itself. If we **ship** these libraries with the binary (macOS `.app` bundle,
Windows installer, AppImage), we must:

- provide their source code, or a written offer to provide it, for the shipped version;
- include the text of their licence;
- allow the user to replace the library with another version: this is
  naturally the case with a separate `.dylib`/`.so`/`.dll`;
- not forbid, in `tri`'s licence, the reverse engineering needed to debug these
  modifications.

With **static linking**, we would also have to provide `tri`'s object files allowing it to
be relinked against another version. We avoid this: dynamic linking only. Note: the LibRaw
module is meant to become a dynamic library loaded by `libloading` anyway (see
`CLAUDE.md`).

If we do not ship these libraries (a Linux distribution package depending on the system's
`libraw` and `libheif`), these obligations fall on the distribution, not on us.

### The x265 trap (GPL)

`otool -L` shows that Homebrew's `libheif.dylib` is **linked directly to `libx265`**,
under GPL-2.0-or-later. It is not a plugin loaded on demand: it is a hard dependency.

- **In development**: no problem, nothing is distributed.
- **In distribution**: shipping this libheif with `tri` amounts to shipping GPL code linked to
  the program. If `tri` is under a permissive licence (MIT/Apache), that is a problem. And x265
  is useless here: it only *encodes* HEVC.
- **Solution**: for a distributed binary, build libheif yourself **without an encoder**
  (CMake option `WITH_X265=OFF`; libde265 alone for decoding), or go through the system
  decoder (see §3).

On Linux, from libheif 1.17, codecs are separate plugins
(`libheif-plugin-libde265`, `libheif-plugin-x265`…): we depend only on the decoding
plugin, and the user or the distribution installs it.

---

## 2. HEVC (H.265) patents

### What this is about

An iPhone HEIC photo is a HEIF container holding an image compressed with **HEVC**.
HEVC is covered by a large number of patents, held by many companies. Their
licences are managed by several competing *patent pools* (notably Access Advance and
Via Licensing Alliance), plus independent holders. There is **no one-stop
shop**, and terms vary by pool.

This is **independent of the code licence**: libde265 is free software (LGPL) written
without proprietary code, but *implementing* HEVC can still fall under these patents
in countries where they are recognised and valid.

### Who is concerned

- **The source code of `tri`**: it does not implement HEVC. It calls libheif, which delegates to a
  decoding plugin. Publishing this source code does not mean distributing an HEVC decoder.
- **A distributed binary that embeds libde265** (or any other HEVC decoder), for example a
  macOS bundle or a Windows installer: this is the case that may be exposed to royalty
  claims, depending on the country.
- **Personal use** (building and using `tri` on one's own machine): in practice, not an issue.
- **Geography matters**: software patents are widely recognised in the United States.
  In Europe, "software as such" is not patentable, but HEVC-related patents have been
  granted there. So the situation is not zero there either.
- **Duration**: HEVC was standardised in 2013; patents generally last 20 years from
  filing. A good share should expire around the early 2030s, but not all of them.

### What others do

A useful indicator, not a guarantee:

- **Fedora** ships no HEVC decoder in its official repositories. libheif is built there
  without HEVC, and full support comes from RPM Fusion (package `libheif-freeworld`), a
  third-party repository that users enable themselves.
- **Debian / Ubuntu** ship libde265 in their repositories.
- **Windows**: HEVC decoding goes through Microsoft's "HEVC Video Extensions"
  (paid, on the Store), which the user installs; applications use the system
  decoder.
- **macOS / iOS**: the HEVC decoder is part of the system. Apple pays the licences, and
  applications that use the system APIs (ImageIO) need not worry about it.

### AVIF / AV1

AV1 is promoted as royalty-free by the Alliance for Open Media (AOM), with a free patent
licence from its members. A third-party pool (Sisvel) nevertheless claims patents on AV1. The
risk is generally considered much lower than for HEVC. AVIF is not yet supported by
`tri`.

---

## 3. Distribution options

Nothing needs deciding before the MVP. These options are to keep in mind so as not to close
any door:

1. **HEIF optional at build time** — ✅ **done**: Cargo feature `heif` (enabled by
   default). `cargo build --release --no-default-features` produces a binary without any
   HEVC dependency (checked with `otool -L`: no libheif, no libde265, no x265). In that
   case, `flat::decode` returns `FlatError::HeifNotCompiled` for HEICs.
2. **Linux: depend on the system libheif** (`.deb`/`.rpm` package, Flatpak with
   extension). The HEVC plugin is installed, or not, by the user or the distribution. We
   ship no decoder.
3. **macOS: use the system decoder** (ImageIO, via the `objc2-*` crates) rather than
   libheif. No patent on our side, no LGPL to ship. Trade-off: platform-specific
   code behind the same `flat::decode` function.
4. **Windows: go through WIC** (Windows Imaging Component), which uses the user's HEVC
   extension if it is installed.
5. **Never ship x265**: `tri` does not encode.

Option 1 is in place and compatible with all the others: all libheif code is
confined to `src/flat/heif.rs`. A system decoder (options 3 and 4) could later
replace it behind the same `flat::decode` function.

---

## 4. Choice of `tri`'s licence: **GPL-3.0-or-later** (decided on 2026-10-19)

The user's choice: modified and distributed versions remain free. File
`LICENSE` (official text from gnu.org), `license` field of both `Cargo.toml`. Comparison
used to decide:

Both families are compatible with the current dependencies, provided §1 is respected
(dynamic linking, no x265 shipped if the licence is permissive).

| | **MIT OR Apache-2.0** | **GPL-3.0-or-later** |
|---|---|---|
| Use in the Rust ecosystem | Standard | Less common |
| Can a third party turn it into closed software? | Yes | No: modified and distributed versions remain free |
| Compatibility with LGPL dependencies | Yes (dynamic linking) | Yes |
| Shipping a libheif linked to x265 (GPL) | **No** | Yes (GPL-2.0-or-later is compatible with GPL-3) |
| Reuse of the engine by other projects | Very easy | Only by GPL-compatible projects |
| Explicit patent clause | Yes (Apache-2.0) | Yes (GPL-3) |

Note: **none of these licences settles the HEVC patent question** (§2). The patent
clauses of Apache-2.0 and the GPL-3 only cover the patents *of `tri`'s
contributors*, not those of HEVC patent holders.

The choice must be made **before the first external contribution**: changing licence afterwards
requires the agreement of all contributors.

---

## 5. Before a first release

- [x] Choose `tri`'s licence, add `LICENSE` and the `license` field in `Cargo.toml`
      (GPL-3.0-or-later, 2026-10-19).
- [x] Generate the crate notices file (`cargo-about`, `licenses/rust-crates.html`
      in the packages, 2026-10-19).
- [x] Windows (2026-10-21): Visual C++ runtime shipped next to the executable
      (Visual Studio redistributable files, "app-local" deployment allowed by Microsoft);
      installer built by WiX (MS-RL licence, which only covers the tool, not the
      `.msi` it produces).
- [x] Linux (2026-10-25): `.deb` made by `dpkg-deb` (system tool, nothing added);
      AppImage by `appimagetool` 1.9.1 (MIT, build tool only, downloaded by the CI). The
      AppImage starts with the AppImage project's type 2 *runtime* (MIT), a separate
      program that mounts the contents; it embeds libfuse (LGPL-2.1) and squashfuse
      (BSD-2-Clause), not linked with `tri` (mere aggregation), sources at
      `github.com/AppImage/type2-runtime`.
- [x] Decide how HEIF is handled in distributed binaries (§3): **without HEIC**
      (decided on 2026-10-19), builds without the `heif` feature; to be reopened with the
      system decoders (ImageIO, WIC).
- [x] If LGPL libraries are shipped: include their licences and a link to their exact
      source code (`licenses/README.md` in the packages, 2026-10-19). For the public
      beta, also attach LibRaw's source archive to the release (safer
      than a link that may disappear).
- [x] Check that no distributed binary contains or depends on x265: without the
      `heif` feature, neither libheif nor x265 (checked with `otool -L` on the macOS package,
      2026-10-19).
- [ ] Reread this document: versions, licences and distribution practices may have
      changed.
