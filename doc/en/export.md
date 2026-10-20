# Module `tri::export`: exporting developed images

Decided on 2026-10-19 with the user: first step of the "after-development" stage (see
`CLAUDE.md`). Images are exported as JPEG, TIFF or PNG with one **export profile** per
use. The user's own uses are the built-in profiles:

- **Editing** (`edit`): 16-bit TIFF in ProPhoto, full resolution, meant for an external
  editor;
- **Web** (`web`): sRGB JPEG at quality 90, 2048 pixels at most.

**Vocabulary** (decided on 2026-10-20): the interface and the command line say
**"export preset"**, like "crop preset": one word for one idea, and "profile" is kept for
**color profiles** (ICC), with no ambiguity in the export dialog. The code (`Profile`)
and the configuration file (`[export.profiles.<id>]`) keep their names, so that no
configuration breaks; this document still says "profile" in places, meaning preset.

Without darktable, export goes through LibRaw (see below). Next planned step: cropped
versions in `tri` (stored as instructions in the XMP).

## darktable does everything

`tri` implements neither formats nor color profiles: `darktable-cli`, already driven for
developed renders ([develop.md](develop.md)), takes care of them. A profile is just a
handful of options translated into arguments (`Profile::darktable_args`):

| Key | darktable-cli |
|---|---|
| `format` (`jpeg`, `tiff`, `png`) | `--out-ext jpg / tif / png` |
| `bits` | `--conf plugins/imageio/format/tiff/bpp=…` (or `png/bpp`) |
| `quality` | `--conf plugins/imageio/format/jpeg/quality=…` |
| `color` | `--icc-type SRGB / ADOBERGB / LIN_REC709 / LIN_REC2020 / PROPHOTO_RGB / DISPLAY_P3`, or `FILE` + `--icc-file` for an `.icc` |
| `intent` | `--icc-intent PERCEPTUAL / RELATIVE_COLORIMETRIC / …` |
| `max_side` | `--width N --height N` (0: full resolution), `--upscale false` |

Always `--hq true`: an export is made for quality.

Options checked on 2026-10-19 on darktable 5.6.2 (`darktable-cli --help`,
`--help icc-type`, `darktablerc` keys). The color spaces offered are those of darktable's
export list, without the HDR variants (PQ, HLG), useless for a photo export.
Beware: **darktable's built-in ProPhoto is linear** ("Linear ProPhoto RGB", read back by
ExifTool). For a classic gamma 1.8 ProPhoto, the path of an `.icc` profile must be given.
That is the one the user uses (confirmed on 2026-10-19: the only one offered by
darktable's interface): the "Editing" profile therefore gives the same file as their
usual darktable export.

## No ProPhoto in JPEG

User's idea, 2026-10-19: "stop some idiot from exporting a JPEG in ProPhoto". ProPhoto
(very wide) and the linear spaces (Rec709, Rec2020) only make sense in **16 bits or
more**: in 8 bits, their gradients break into steps (posterization). sRGB, Adobe RGB and
Display P3 remain safe in 8 bits.

- **Engine**: `Profile::check` rejects such a profile, with the reason. The rejection
  happens as soon as the settings are read, like any invalid value; a hand-written
  configuration file is therefore reported at startup.
- **Settings window**: the drop-down list of spaces does not offer those for a profile in
  JPEG or in 8 bits, nor JPEG for a profile in ProPhoto (`fits`). An inconsistent change
  is rejected with the reason.
- An `.icc` profile is left to the user's judgment (`color_fits`).

## A single darktable launch

All the images of an export go through **a single** `darktable-cli` (`--import` per
image): its startup (~6 s) is paid only once. Measured on 2026-10-19 on two X-T4 RAFs, in
2048 px JPEG: 13.5 s in total.

- Each image is developed according to **its own** `IMAGE.RAF.xmp` sidecar, history and
  crop included. Checked: DSCF0331, cropped in darktable, comes out at 2763 × 4912.
  An image without development (no sidecar, or a sidecar written by `tri` alone) gets
  darktable's default settings. These are not the user's automatic presets
  (configuration specific to `tri`, `--apply-custom-presets false`).
- **File names**: `$(FILE_NAME)` (image name without extension), then the profile
  suffix, then the extension. darktable never overwrites anything: it appends `_01`…
- **Progress**: darktable writes an `[export_job] exported to `…'` line per file. Read
  as it comes, it gives the progress and the files actually written
  (`Report::written`). Images with no file written are counted (`Report::failed`).
- **Cancellation**: the program is stopped; files already written remain.
- **RAW+JPEG pairs**: see "RAW + JPEG pairs, mixes" (both are exported).
- **Nothing is written in the user's setup**: a dedicated configuration
  (`<cache>/darktable-export`, separate from the one for developed renders, so that both
  can run at the same time), in-memory library, `write_sidecar_files=never`.
  Checked: sidecars identical byte for byte after export.

## Built-in profiles

| Profile | Format | Space | Size | Without recompression | Applies to |
|---|---|---|---|---|---|
| Editing (`edit`) | 16-bit TIFF | linear ProPhoto | full | — | everything |
| JPEG as is (`asis`) | JPEG | original (`""`) | full | yes | developed images |
| Web (`web`) | JPEG 90 | sRGB | 2048 px | yes | everything |

"JPEG as is" (requested on 2026-10-19): a JPEG is copied without touching anything
(size, space: an Adobe RGB JPEG stays so; no quality shown, it is that of the original
file). **For exporting JPEGs only** (camera JPEG, original JPEG): a RAW has no color
space, its render must be given one. When a JPEG is exported, it is offered first.

**What a profile applies to** (`applies_to`, user's idea, 2026-10-19): `"all"`
(default), `"raw"` (render of a RAW: darktable, LibRaw) or `"flat"` (already developed
image: camera JPEG, JPEG, TIFF…). "JPEG as is" is `"flat"`. The dialog only offers the
profiles that apply to what is being exported (`AppliesTo::allows`, `exports_flat`), and
the command line rejects the others, with the reason.

**Offered profile** (2026-10-19): `[export] default_raw` (RAW render, default `edit`) and
`default_flat` (already developed image, default `asis`), rather than a per-profile
parameter (a single choice per case, never two "default" profiles). Within the session,
the last profile chosen for this type of export wins. An unknown identifier, or a
profile that does not apply, falls back to the first in the list
(`ExportSettings::default_index`).

## Settings (`[export]`)

```toml
[export.profiles.web]        # autant de profils que voulu
name = ""                    # empty: translated built-in name, otherwise the id
format = "jpeg"
bits = 8
quality = 90
color = "srgb"
intent = ""
max_side = 2048
suffix = ""
folder = "export"            # voir « Dossier du profil »
```

Each missing key comes from the built-in profile with the same identifier (`edit`,
`web`), otherwise from an sRGB JPEG (`ProfileToml`, `Profiles::deserialize`). The
settings window can thus write a single value without making the rest of the profile
disappear. Order: built-in profiles, then the user's ones by identifier.

### Profile folder

User's idea (2026-10-19): the folder is part of the profile ("Web" in `~/Web`, "Editing"
next to the images…), rather than remembered from one export to the next. Rule for
`folder` (`folder_for`, `output_pattern`):

- `/…`: absolute;
- `~/…`: from the home folder;
- otherwise **relative to each image's folder**: `"export"` (default) creates a
  subfolder next to the images; `""` or `"."` writes **next to the source**. Passed to
  darktable as `$(FILE_FOLDER)/<folder>/$(FILE_NAME)`: a selection coming from several
  folders is exported into each of them.
- darktable variables are allowed (`$(YEAR)`…, see darktable's documentation), passed
  as is.

Checked on 2026-10-19: subfolders created by darktable, accents and spaces included
(`séance été/export/…`), and `"."` next to the RAF.

## We export what we are looking at

User's rule (2026-10-19): the exported image is the one being looked at.

| Where | Exported image |
|---|---|
| Grid | developed by darktable if available, otherwise LibRaw render |
| Full screen, developed (`Shift+B`) | developed by darktable |
| Full screen, RAW render (`b`) | LibRaw render |
| Full screen, camera JPEG | the camera JPEG (embedded preview, at its own size) |

The export dialog offers **no choice**: you press `Ctrl+E` on the desired image (user
feedback, 2026-10-19: no extra complexity in the dialog). `Render::of` decides, and a
line of explanation says so. On the command line: darktable if available, `--libraw` or
`--embedded` for the others.

The **camera JPEG** (`Local::Embedded`) is the RAW's embedded preview, at its own size
(71 % of the sensor on an X-T4), straightened.

## Original space of already developed images

User's remark (2026-10-19): a camera JPEG can be in **Adobe RGB** (a setting on almost
every camera), and so can a JPEG on disk. So nothing is assumed: `Origin` reads the
actual space of each camera JPEG or flat image.

1. The **embedded ICC profile** (`jpeg::icc_profile`: APP2 segments joined end to end;
   other formats: the one read by the `image` crate). sRGB and Adobe RGB are recognized
   there by their description, in ASCII or UTF-16.
2. Otherwise the **EXIF**, following the DCF standard: `InteroperabilityIndex` "R03"
   (Adobe RGB) or "R98" (sRGB), and `ColorSpace` 2, which some cameras write for Adobe
   RGB.
3. Otherwise sRGB, the default EXIF space.

Checked on the user's X-T4: camera JPEG in "R98", hence sRGB.

The export profile's space then decides:

- `""` (**"JPEG as is"**): the original space is **kept**, with its ICC profile.
- Same space as the original: nothing to convert.
- Another space: **conversion** by `moxcms` (16 bits), then the ICC profile of the new
  space. An Adobe RGB JPEG exported with "Web" therefore comes out in sRGB, as it should
  for the web.

## RAW + JPEG pairs, mixes

User feedback (2026-10-19): the JPEG of a pair was developed by the camera, the RAW is
developed by darktable or LibRaw; they are two different images, both to be exported.

- `Batch::new` sorts the export: the RAWs (rendered), the already developed images (JPEG,
  TIFF…: copied, converted or downsized), and among them the **pair JPEGs** whose RAW is
  also exported. These take the `[export] camera_suffix` suffix (`-camera`):
  `DSCF0001.jpg` (RAW render) and `DSCF0001-camera.jpg` (camera JPEG). Checked on
  2026-10-19 on a real pair, with the Web profile, in 12.8 s.
- Exception: in full screen on the **camera JPEG**, the pair goes out only once. The
  camera JPEG and the JPG file are then the same image from the camera.
- `run` exports the RAWs through the chosen engine (darktable, going through a temporary
  TIFF if cropping or sharpening is needed; LibRaw; camera JPEG), then the already
  developed images, with a single progress and a single summary. The command line and
  the interface both go through it.
- **Offered profiles** (`Kind`): RAWs only, profiles that apply to RAWs; developed images
  only, those that apply to them; **mix**, only those that apply to everything
  (`applies_to = "all"`), the lowest common denominator.
- The dialog says so under its title, for example "1 RAW, developed by darktable · 1
  camera JPEG (pair, suffixed name)".
- The offered profile is found again by its **identifier** (the last one chosen for this
  type of export, otherwise `default_raw` / `default_flat`), since the list changes
  depending on what is exported.

## Without recompression (`passthrough`)

User's idea (2026-10-19): do not recompress an already compressed file a second time.
Per-profile setting (`passthrough`), **enabled by default** (user feedback), "Without
recompression" checkbox in the dialog, `--passthrough` on the command line. It only
applies to JPEG profiles, from a JPEG, **in the same space** (see above: a conversion
necessarily recompresses):

- **Original JPEG** (flat image): copied as is, byte for byte (EXIF, profile included).
- **Camera JPEG** of a RAW: its bytes are written as is; only the RAW's **orientation**
  is set in its EXIF, without recompressing (`jpeg::set_orientation`: tag modified in
  place, or a minimal EXIF segment added). Checked on an X-T4 RAF: compressed data
  identical to the embedded preview extracted by ExifTool, same file size, "Rotate 90
  CW".
- An image **larger than the profile's maximum side** is re-encoded (downsizing means
  recompressing). A **render** (darktable, LibRaw) has nothing compressed to keep: no
  effect, and the checkbox is not offered.
- The summary says how many files were written without recompression
  (`Report::passthrough`).

## Output sharpening (`sharpen`)

User's idea (2026-10-19), like Lightroom: sharpen for the destination, to compensate for
the softness caused by downsizing (screen) or by ink (print). `sharpen` key of each
profile: `"none"` (Editing, JPEG as is: the external editor or nothing), `"screen"`
(Web), `"print"`.

- **Unsharp mask** (`unsharp`), in 16 bits, **after downsizing**, like Lightroom:
  `original + amount × (original − gaussian blur)`, where the difference exceeds a
  threshold. The threshold avoids sharpening the noise. The `image` crate's function has
  no amount setting, hence a home-made version.
- Parameters in `[export.sharpening]`, for screen and print:
  `screen_radius` / `screen_amount` / `screen_threshold`, and likewise `print_*`. By
  default: screen 0.6 px, amount 0.6, threshold 2 levels; print 1.2 px, amount 0.9,
  threshold 3. Starting values, to be adjusted with use.
- **darktable** does not sharpen for us: a profile that sharpens (or a version) goes
  through `export_via_tiff`. darktable renders a temporary 16-bit TIFF, in a single
  launch, which is then cropped, downsized, sharpened and written. A single compression.
- A **copy without recompression** is not sharpened: sharpening means re-encoding. A
  profile that sharpens is therefore never "as is".
- Summary in the dialog: "… · screen sharpening".

## Without darktable: LibRaw

Step 2 of the plan (2026-10-19, "simpler", according to the user): without darktable
installed (or with `--libraw` on the command line), export goes through LibRaw
(`export_libraw`). You get `tri`'s **basic render**, the one of the `b` key: camera
white balance, no automatic brightness, no adjustments. It is the actual exposure of the
shot, flatter than darktable's default settings.

- **RAW**: `RawFile::render_deep`, full render in **16 bits** directly in the requested
  space (LibRaw's `params.output_color` and `params.gamm`).
  | Space | LibRaw | Embedded ICC profile |
  |---|---|---|
  | `srgb`, `""` | 1, sRGB curve (1/2.4, 12.92) | sRGB |
  | `adobergb` | 2, gamma 2.2 | Adobe RGB |
  | `prophoto` | 4, linear | "Linear ProPhoto RGB", like darktable |
  | `rec709` | 1, linear | "Linear Rec709 RGB" |
  | `rec2020` | 8, linear | "Linear Rec2020 RGB" |

  Display P3, an `.icc` file and 32-bit TIFF require darktable: `libraw_check` says so,
  the export dialog too, and the button stays greyed out.
- **Flat images** (JPEG, TIFF…) and **camera JPEG**: their original space is kept or
  converted (see "Original space").
- Lanczos3 downsizing in 16 bits (`image::imageops::resize`), then writing by the
  `image` crate: JPEG (8 bits, profile quality), TIFF or PNG in 8 or 16 bits.
- **ICC profiles**: generated by `moxcms`, the pure-Rust color management already
  compiled in by `image` (see [licensing.md](licensing.md)), then embedded in the files.
  No `.icc` file is shipped.
- Same folder, same names, same rule: nothing is overwritten (`unique_path`: `_01`…).
  A failing image does not stop the others; the reason is in `Report::errors`.
- Measured on 2026-10-19 (two X-T4 RAFs, Apple Silicon Mac): 16-bit ProPhoto TIFF at
  full resolution, 8.6 s for both. Visually compared with darktable's export of the same
  image: consistent framing, orientation and colors, darktable brighter (its default
  settings).

## Creating a profile from the interface

Requested on 2026-10-19: no more need to edit the file. In the settings window
(`Ctrl+,`), a single **"Export presets"** entry on the left (feedback from 2026-10-20:
one entry per profile cluttered the list); on the right, the shared explanation, then:

- a **drop-down list** of the profiles (built-in, then the user's ones): the chosen
  profile is set up below;
- **"New…"**: asks for a name, then copies the displayed profile under that name.
  **All** its values are written, so that the new profile does not depend on another
  one's default values. Identifier derived from the name (`versions::new_id`). After
  that, it is set up like the others (drop-down lists, inconsistencies rejected);
- **"Delete"**, for the user's profiles only (greyed out on a built-in profile: they can
  be modified but not deleted).
- Internally, each profile remains a section (`SettingsWindow::schema`: sections of the
  example, plus those found in the file; `ConfigFile::subtables`, `remove_table`); the
  left-hand list shows only one per family (`listed`), and the search finds them all
  ("Export preset "print"").
- Applied live: the profile is immediately offered in the export dialog.

Crop presets are created the same way (see `versions.md`).

## Interface and command line

- `Ctrl+E` (`Cmd+E` on macOS), action `core.file.export`: dialog on the selection (or
  the current image).
  - **Preset**: up / down arrows when the list has the keyboard (at first), or click;
    Tab moves to the folder, where the arrows no longer change the preset (feedback from
    2026-10-20). The last one chosen is remembered during the session.
  - **Folder**: the profile's, which follows the choice of profile; can be changed for
    this time only (Tab completion), with the rule recalled and the actual destination
    of the first image.
  - **Export**: Enter. It runs in the background, with a progress bar in the status bar
    (× button to stop it), then a summary.
- **Export report** (`Ctrl+Shift+E`, action `core.file.export.report`; user's idea from
  2026-10-20): the list of finished exports, most recent at the top, in a scrolling area
  of limited height. For each: preset, version, files written out of images requested,
  how long ago ("3 min ago": no date library, and the age is enough), stop or failure.
  Its destination **folders** (one per image folder if the folder is relative) are links
  that show them in the file manager (`files::reveal` on their first file); the list of
  files and the failures can be expanded. "Export report" button in the status bar,
  **before** an export's summary (a long path does not push it out of the window).
  "Clear the list" empties the log: a small broom at the bottom right (tooltip "Clear
  the list"), far from Close (feedback from 2026-10-20).
  - Engine: `tri::export_log` (`Entry`, `read`, `append`, `clear`), file
    `exports.jsonl` in the data folder (one JSON object per line: an added export does
    not rewrite the rest; unreadable line ignored). Kept from one session to the next,
    limited to the `[export] report_max` most recent (500; 0: no limit) so that it does
    not grow for years.
- `tri --export <profil> <image>... [--to <dossier>] [--libraw|--embedded] [--passthrough] [--dry-run]`:
  `--to` replaces the profile's folder (same rule); `--libraw` forces the LibRaw render,
  `--embedded` exports the camera JPEG.

## Tests

- Unit (`export::tests`): built-in profiles and arguments, keys completed by the profile
  with the same identifier, inconsistent profiles rejected (ProPhoto in JPEG included, as
  soon as the settings are read), pairs, destination, reading of the progress lines.
- `darktable_exports_a_tiff_and_a_jpeg` (`#[ignore]`, `TRI_TEST_DARKTABLE_RAW`): real
  TIFF then JPEG export of a copy, progress, size, sidecar intact.
- Interface: `ctrl_e_offers_the_export_profiles_and_remembers_the_choice`;
  settings window: `export_profiles_only_offer_coherent_choices`.
