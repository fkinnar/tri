# Module `tri::develop`: rendering by a development software

The user's idea (2026-10-19): see your photos in `tri` **as they were developed**
elsewhere, darktable first. The render is **on request**: `Shift+B` or the button
(aperture) in the strip below the image. After that, `b` goes from the camera JPEG to the
RAW render, then to the **developed** one, and back to the camera JPEG; as long as the
image has not been developed, `b` only offers camera JPEG and RAW and never launches
darktable (user feedback, 2026-10-19: when culling, the basic render is almost always
enough, no point waiting 10 s for nothing).

## A compiled plugin driving an external program

Like the other plugins (overlays, sending), the code is in Rust and compiled with `tri`;
it calls `darktable-cli` as a **separate process**: darktable cannot bring `tri` down,
and its license remains its own. RawTherapee (`rawtherapee-cli`, `.pp3` profiles) and ART
would follow the same model.

**Several software** (2026-10-26, for RapidRAW and the next ones): each software is an
`Engine` (identifier, name, sidecar carrying its development) and a plugin of the
`Developer` trait (`render`); `Developers` is the list of those found on the machine
(`Developers::find`, work files under `<cache>/develop`). An image may have been
developed in several: `resolve` picks the one of the `[develop] default` setting
(drop-down list in the settings window: "auto" and the installed software) if it
developed the image, otherwise the latest development (`"auto"`, default). User's
choice: someone working in darktable who tries another software sets
`default = "darktable"`, so the latest does not take over.

- `find_darktable(réglage)`: `[develop] darktable` = `"auto"` (the `PATH`, then the usual
  locations: `/Applications/darktable.app/…` on macOS, `/usr/bin` on Linux,
  `Program Files` on Windows), a path, or `""` (never). On Linux, if it is not found: a
  darktable **AppImage** (`*darktable*.appimage` in `~/AppImages`, `~/Applications`,
  `~/.local/bin`, `~/bin`, the downloads folder, `/opt`; the most recent one), launched
  with `darktable-cli` as first argument: the AppImage then runs that binary instead of
  the interface (documented by darktable, "invocation per OS"). That is the user's
  installation on Mint (`~/AppImages/darktable.appimage`, 2026-10-19): **checked by him**
  the same day, rendering and export work. An AppImage path is also accepted in the
  setting. Otherwise, the **Flatpak** `org.darktable.Darktable` (system or user
  installation, `flatpak info`), launched with `flatpak run --command=darktable-cli
  org.darktable.Darktable` (`Cli`: program and arguments placed before `tri`'s);
  `"flatpak"` selects it outright. Observed on 2026-10-19 on Linux Mint: the Software
  Manager installs darktable as a Flatpak, with no `darktable-cli` in the `PATH` (and its
  exported launcher opens the interface, not `darktable-cli`). The Flatpak must be able to
  read `tri`'s images and cache (home folder): to be checked for images outside the home
  folder.
- `darktable_sidecar(image)`: `IMAGE.RAF.xmp`, **only if it is darktable's**: written
  by darktable (`darktable:xmp_version`), or with a history (`darktable:history_end` >
  0). An image imported but not edited (virgin clone) thus has its default darktable
  render, which is not LibRaw's (feedback of 2026-10-30). A sidecar written by `tri`
  alone (notes) has none.
- `Engine::sidecar(image)`: for a development (`developments.md`), its sidecar if it is
  this software's; for a photo that has developments of this software, **nothing**: the
  sidecar is theirs (they have their thumbnail), the photo remains the neutral render.
- `Darktable::render`: `darktable-cli image sidecar sortie --width --height --hq
  --core --configdir <cache>/darktable --library :memory: --conf
  write_sidecar_files=never`, cancellable (the program is stopped).

## RapidRAW (2026-10-26, #114)

Exporting RapidRAW developments goes through the same command (16-bit sRGB TIFF, then
conversion; `export_rapidraw`, see `export.md`, "Through RapidRAW").

RapidRAW (AGPL-3.0, called as a separate process) has a command without interface since
version 1.6: `rapidraw export <image> --output <file> --format jpeg --quality 95`, which
applies by itself the settings of the **`IMAGE.RAF.rrdata`** sidecar next to the image
(`rapidraw_sidecar`; its virtual copies `IMAGE.RAF.<id>.rrdata` are not read). It exports
at full size, with no size option: `tri` then shrinks to the standard preview side
(`shrink`). **RapidRAW's interface open**: the export without interface still runs (it
skips the single instance), but rendered an **all-black image** without any error (test of
2026-10-26). `shrink` refuses an image with no pixel above 0 (sensor noise is enough to go
above 0 on a real photo): nothing enters the cache, and the message says so plainly ("close
it and try again"). Should a future version return the "already running" refusal, it is
translated the same way.

`find_rapidraw(setting)`: `[develop] rapidraw` = `"auto"` (`rapidraw` / `RapidRAW` in the
`PATH`, `/usr/bin` of the `.deb` package, `RapidRAW.app` on macOS, `Program Files` on
Windows; on Linux, a `*rapidraw*` AppImage, then the `io.github.CyberTimon.RapidRAW`
Flatpak), a path (the executable or the AppImage, which receives `export` directly),
`"flatpak"`, or `""`. The program's messages go to a file (not a pipe: a talkative
program does not block). The real resolution of the developed image is read for
darktable only: for RapidRAW, unknown rather than wrong.

## Nothing is written in the user's files

Configuration specific to `tri` (in its cache), library in memory: neither darktable's
configuration nor its library is touched, and darktable can stay open (no lock).
`write_sidecar_files=never`: the sidecar is never rewritten, whatever the configuration;
a test checks this on a real render (on a copy).

## Opening a folder in the software (2026-10-28)

`Developer::open_folder` gives the command that opens a folder in the software's
interface; `Developers::folder_openers` lists the installed software that can,
`Developers::open_folder` gives the command (started by `files::launch`, without waiting
for it).

- **darktable**: `darktable <folder>` opens the lighttable with that folder as the
  collection, imported if needed. An instance already open receives it: through D-Bus on
  Linux; on macOS, through `open -a darktable.app <folder>` (the system passes it,
  darktable's `_osx_openfile_callback`). Starting the `.app`'s program started a second
  instance, which crashed (feedback of 2026-10-28). On Windows, nothing of the kind:
  with darktable already open (`process::running`, `tasklist`), the entry is greyed
  (`Developer::open_blocked`). This time with the user's configuration and library, not `tri`'s: the user is
  the one working. The interface is found from `darktable-cli`
  (`develop::darktable_gui`): the `darktable` program next to it (package, `.app`,
  Windows installer), the AppImage without the `darktable-cli` command, or the Flatpak
  without `--command`.
- **RapidRAW**: no folder opening; as an argument, it only opens an image in its editor
  (RapidRAW's `launch_request.rs`: a path = `OpenFile`, checked on 2026-10-28).

## Cache and computation

Level `Level::Developed(engine)`, one per software (folder `developed` for darktable,
unchanged: its cache stays valid): the software's JPEG, at the standard preview size, stored **as is**
in the cache. Its key includes the fingerprint of the **development** described by the
sidecar (`develop::render_stamp`: the `darktable:…` fields without
`darktable:colorlabels`, or the whole `.rrdata`): changing the development in darktable
redoes the render, rating the image in `tri` (which writes to the same `IMAGE.RAF.xmp`)
does not. Until 2026-10-28, the file's size and date: every rating redid the render. In the interface, a developed image already
displayed whose sidecar has changed (folder watching, or at the time of `b` / `Shift+B`)
is forgotten: `b` goes back to the basic render and `Shift+B` redoes it (feedback from
2026-10-19: a modified crop was not seen). Computed **on explicit request only**
(`Shift+B`, displayed image; an image already in the cache stays in the `b` rotation,
even from one session to the next), by the heavy worker (darktable occupies the whole
machine), never the whole folder in the background. Measured on 2026-10-19 (X-T4 RAF,
Apple Silicon Mac): 8 to 10 s per image, of which ~6 s is darktable's startup.

## Size of the developed image (2026-10-21)

`developed_size(xmp, size)`: the size of the image darktable would produce at full
resolution, for the resolution shown in full screen. Read from the sidecar's history
(`darktable:history`, entries before `darktable:history_end`, last state of each module,
main instance):

- **`crop`**: its parameters are four little-endian 32-bit floats in hexadecimal, left,
  top, right and bottom edges as fractions of the image (checked on real sidecars of the
  user, darktable 5, crop v3);
- **`flip`**: `-1` (`ffffffff`) follows the EXIF orientation; another value is a
  darktable orientation, width and height swapped if bit 4 is set;
- **`ashift`**, **`clipping`**, **`rotatepixels`**, **`scalepixels`** active: unknown
  size (`None`), rather than a wrong one; likewise for compressed parameters (`gz…`).

## Comparing developments in full screen (2026-10-26, #114)

`b` goes through everything that exists for the shown image: camera JPEG, RAW render, then
**each of its developments already rendered** (`view::Source::next`,
`TriApp::rendered_engines`), its most recent development first (or the preferred
software's), and back to the camera JPEG. A development not rendered yet is not in the
round (`Shift+B`, `<leader>d` / `<leader>p` render it). The label says which one is shown
("Developed in RapidRAW").

## In the interface

While waiting for the render, the RAW render (or the camera JPEG) with "darktable
development in progress…"; when the image has no development, the reason instead. No
developed 1:1: you zoom into the render. The focus point and the clipped / blocked areas,
placed on the whole sensor, are hidden on a developed image (cropped). Without darktable,
the source is not offered and `Shift+B` says so.

**A whole folder in the background** (2026-10-25, issue #69): `Ctrl+Shift+B` (the open
folder) or the right click on a folder of the panel, "Develop the whole folder
(darktable)". Each image with a darktable sidecar that has a history is rendered one
after the other (`gui/src/develop_batch.rs`), by the same producer as `Shift+B`
(`CachedPreviews`, `Developed` level): another rendering program plugged in there will
serve the same way. Already in the cache: skipped. One batch at a time; its progress is a
background task ("×": interrupt), the summary (rendered, already done, failed) goes to
the status bar; afterwards, `b` offers the developed render of each rendered image.

**Renders to redo announced** (2026-10-29: photos edited in darktable were rendered
again without a word): the batch first sees what is already in the cache
(`DevelopBatch::redone` → (done, to redo)); if renders are left to redo, a banner at the
top of the image says so, "12 changed developments to redo (3/12)" with the progress bar,
then "12 developments redone" at the end; each fades by itself (`[gui.grid]
develop_toast_ms`, 6 s; 0: never). Progress stays in the status bar.

**Choosing the software** (2026-10-26, user's choice; shortcuts configurable):
`Shift+B` and `<leader>r` develop the image in its software (the one of its development,
see `[develop] default`), `<leader>d` in darktable, `<leader>p` in RapidRAW
(`Action::DevelopWith`, identifiers `plugin.darktable.develop`,
`plugin.rapidraw.develop`; `core.develop.render` and `core.develop.folder` for the other
two). Right click on an image, "Develop with": the installed software, or the RAW render
(LibRaw). The chosen software takes over the development's one for that image, for the
session (`develop_choice`). Right click on a folder, "Develop the whole folder…" opens a
dialog: each image in the software of its development (otherwise it keeps the RAW
render), or all in one software (the images it developed). `Ctrl+Shift+B`: each image in
its own, without dialog.

Coming: `darktable-cli` can process a folder in a single launch (amortized startup) — for
sending developed JPEGs in the "after-development" stage.

## Developments in the grid (2026-10-26, #114)

After developing, one wants to see the developed images; when offloading and culling, the
camera JPEG is enough. `[gui.grid] render` (`gui/src/grid_render.rs`, `GridRender`), also
in the filter bar (list on the right, `f d` for the next one), kept from one session to the
next: `embedded` (camera JPEG, default), `latest` (each image's most recent development,
`develop::resolve` without preference), `darktable` / `rapidraw` (always this software),
`libraw` (RAW render). The list only offers the installed software. A thumbnail has only
**two states**: camera JPEG, or developed (a chip under the thumbnail with the software's
name, or "LibRaw"; "DEV" at first, feedback of 2026-10-26: it said nothing). A RAW without the wanted development shows its **LibRaw render**: as
soon as the grid shows renders, LibRaw renders go to the background for the whole folder
(`Scheduler::set_raw_previews`, one more background task). A development not rendered yet:
in the background, a batch like `Ctrl+Shift+B` (`[gui.grid] develop = "auto"`, no summary if
there was nothing to render), or on request only (`"manual"`); LibRaw, then the thumbnail,
meanwhile. The batch's renders, made outside the engine, are looked up in the cache at most
once per `render_poll_ms`. The render, much larger than a thumbnail, is decoded reduced
(`jpeg::decode` at thumbnail size). Flat images keep their thumbnail.
Enter or a double click opens full screen on what the thumbnail shows: its development
(in the same software), or the RAW render; with the camera JPEG in the grid, the source
chosen in full screen (`b`) stays.

## Filtering the developed images (2026-10-27)

The user's idea: a **film reel** in the filter bar (`f Shift+D`), filtering when on; a
list of the installed software then appears next to it, to **check**: the images
developed in **one of the checked software**; **none checked**, the images **without
development** (shown in their LibRaw render). Folded, the list names the software when it
is alone, the abbreviations when there are several ("DT + RR", `Engine::short`), "LibRaw"
when none is checked. The checked boxes are kept from one session to the next
(`gui-state.toml`, `developed_engines`); at first launch, all the installed software.
Engine: `develop::DevelopedRule` (checked software) and `DevelopedFilter` (`Filter`
trait), which read the sidecars (`Engine::sidecar`: non-empty darktable history,
RapidRAW's `.rrdata`); a flat image has no development. A content filter: cleared by
`f Esc`, invertible (`f i`).

Under the filter, the thumbnails show the checked software's development, whatever the
render chosen for the grid (feedback of 2026-10-27): the most recent one when several are
checked (`DevelopedRule::engine_for`), the LibRaw render when none is; full screen opens on
it. Inverted, the filter leaves the grid its render (`TriApp::shown_render`).

## Other development software (research of 2026-10-28)

What each program allows from the command line: a render without interface (what a
render plugin needs), opening a folder ("Open in…"), and where it keeps the edits (to
know an image was developed). From docs and man pages; what only comes from forums is
marked.

| Software | Render without interface | Open a folder | Edits | Linux |
|---|---|---|---|---|
| darktable | `darktable-cli` (integrated) | `darktable <folder>` (integrated) | `photo.raw.xmp` | yes |
| RapidRAW | `rapidraw export` (integrated) | no: one image only | `photo.raw.rrdata` | yes |
| RawTherapee | `rawtherapee-cli` | `rawtherapee <folder>` | `photo.raw.pp3` | yes |
| ART | `ART-cli` | `ART <folder>` | `photo.raw.arp` | yes |
| vkdt | `vkdt-cli` (GPU) | unverified | `photo.raw.cfg` (forum) | yes |
| digiKam / showFoto | no | `showfoto <folder>` | XMP | yes |
| Lightroom Classic | no | files as arguments → import dialog | catalog, XMP `crs:` optional | no |
| Lightroom (cloud) | no | no | in the cloud | no |
| Capture One | no (AppleScript on macOS) | no (forum) | `CaptureOne/Settings…/*.cos` (sessions) | no |
| DxO PhotoLab / PureRAW | no | no | `photo.raw.dop` (forum) | no |
| ON1 Photo RAW | no | no | database, optional `.on1` | no |
| Photomator | no | no | sidecars in Pictures / iCloud | no |
| Luminar Neo | no | no | catalog | no |
| Exposure X7 | no | no | `.exposurex7` in a subfolder | no |
| NX Studio | no | no | `NKSC_PARAM/*.nksc` | no |

- **RawTherapee and ART** are the only candidates for a plugin like darktable's: render
  with `rawtherapee-cli -o out.jpg -s -j92 -Y -c photo.raf` (`-s`: the sidecar; `-c`
  last), the same for `ART-cli` with the `.arp`. No size option: it would have to come
  from a profile (`-p`). Opening a folder is documented; the behaviour with an instance
  already open is not (to try, given darktable on macOS). The user's decision: later,
  not urgent.
- **vkdt**: `vkdt-cli -g photo.raf.cfg --filename out --format o-jpg --width 2560
  --height 2560`, on the graphics card (Vulkan); Windows not mentioned.
- **Commercial software**: none can be driven from the command line for a render. At
  best, recognise their sidecars to mark "developed in…", without showing their render.
  Lightroom Classic takes files as arguments (import dialog, broken when it is already
  open according to FastRawViewer); Capture One has an AppleScript dictionary, said to
  be incomplete.
- "Open in…" therefore stays with darktable (and one day RawTherapee / ART).

Sources: [RawTherapee](https://rawpedia.rawtherapee.com/Command-Line_Options),
[ART](https://manpages.opensuse.org/Tumbleweed/ART/ART.1.en.html),
[vkdt](https://github.com/hanatos/vkdt),
[showFoto](https://dyn.manpages.debian.org/trixie/showfoto/showfoto.1.en.html),
[darktable](https://www.mankier.com/1/darktable),
[Lightroom Classic, XMP and ACR](https://helpx.adobe.com/au/lightroom-classic/desktop/organize-photos-in-lightroom-classic/create-xmp-acr-files.html),
[Lightroom Classic started by FastRawViewer](https://fastrawviewer.com/node/855),
[Capture One, AppleScript](https://support.captureone.com/hc/en-us/articles/360002681418),
[DxO (forum)](https://forum.dxo.com/t/automation-using-cli/36444),
[Exposure](https://support.exposure.software/hc/en-us/articles/360005225913-How-large-are-Exposure-s-sidecar-files),
[NX Studio](https://nikonimglib.com/nxstdo/onlinehelp/en/save_80.html),
[Photomator](https://support.pixelmator.com/faq-photomator/advanced-workflows/save-photomator-linked-files-to-an-alternate-location-on-mac).
