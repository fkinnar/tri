# Module `tri::versions`: crops of a photo

Decided on 2026-10-19 with the user, first piece of the "post-development" (see
`CLAUDE.md`). A photo can have several **versions** (9:16 story, 4:5 post…). They are crop
**instructions**, never copies: correcting the development is enough, all the versions
follow at export.

**One concept, "Version"** (2026-11-07, the user's feedback: crop and version were
mixed up): a version is a frame (crop, ratio) with an **optional** export preset; a "crop
alone" is a version without a preset. The interface says "Version" everywhere ("New
version", "Version presets"); "crop" still finds the action in the help and the palette.
Ids `core.version.*` (`core.version.new`, `.next`, `.prev`, `.save`, `.ratio.*`…); the old
ones (`core.view.crop`, `core.view.version_next` / `_prev`, `core.crop.*`) are still
accepted in `[keys]` (`actions::renamed_id`). Internal code names (`crop.rs`,
`Action::Crop`) and TOML keys (`[versions.presets]`, `[gui.crop]`) unchanged.

## User decisions

- **You crop what you see**: darktable's developed image (on top of its own crop, if any),
  LibRaw's RAW render, or the camera JPEG (`Source`). The user's practice: not cropping in
  darktable, and cropping in `tri`.
- **No straightening** (fine rotation): that is darktable's job. It may come later, in
  another tool, to keep interfaces simple and dedicated to one task.
- A version has a **name**, a **frame**, a **ratio**, and optionally an **export preset**.
  The latter only pre-fills the export dialog, which remains editable.
- Grid: a **badge** ("3 versions") on the thumbnail, not one thumbnail per version, so as
  not to clutter the grid.
- Cropping: the photo shown large, the adjustable frame (free or with a fixed ratio) and a
  **translucent grey veil** around it. Ratio and orientation can be set **with the mouse
  too**. Shortcuts depending on the profile: arrows (classic), `hjkl` (vim).

## Data

- `Rect`: the frame, in **proportions of the viewed image** (straightened), `[x, y, width,
  height]` between 0 and 1, rounded to 1e-5 in the file.
- `Ratio`: `Free`, `Original` (the image's own, possibly turned by a quarter), or
  `Fixed(w, h)`, where the orientation is the order (4:5 portrait, 5:4 landscape). Offered
  ratios (`PRESETS`): free, original, 1:1, 5:4, 3:2, 16:9, 4:3. `rotated` changes the
  orientation.
- `Version`: `name`, `crop`, `ratio`, `source`, `profile` (optional).

## Geometry, tested without an interface

The interface only draws; all the computation is here.

- `Rect::fit(ratio, aspect)`: the largest frame of a ratio, centred.
- `Rect::with_ratio`: change ratio or orientation around the same centre, without growing
  beyond the current area or leaving the image.
- `Rect::drag(handle, dx, dy, ratio, aspect)`:
  - a corner keeps the opposite corner fixed;
  - a side keeps the opposite side fixed, and the other axis centred if the ratio is fixed;
  - `Move` moves the frame;
  - the frame always stays within the image, with sides of at least `MIN_SIDE`.
- `Rect::pixels`: the frame in pixels of a given image.

## Storage

- In the photo's XMP sidecar, list `tri:Versions` (namespace `urn:tri:xmp:1.0`): an
  `rdf:Seq` whose every item is a **JSON object** (`serde_json`). Example:

  ```xml
  <tri:Versions xmlns:tri="urn:tri:xmp:1.0">
   <rdf:Seq>
    <rdf:li>{"name":"Story","crop":[0.07813,0.0,0.84375,1.0],"ratio":"9:16","source":"developed","profile":"web"}</rdf:li>
   </rdf:Seq>
  </tri:Versions>
  ```

- Generic list of the XMP module (`xmp::ListField`, `read_list`, `apply_list`,
  `write_list`), **separate from the notes**: the notes rewriter does not know it, so it
  copies it over as is. Writing the list changes nothing else, byte for byte. Only `&`, `<`
  and `>` are escaped, so that the JSON stays readable.
- Checked on 2026-10-19 on a copy of the user's darktable sidecar: the only addition is the
  `tri:Versions` element, and `darktable-cli` renders the image normally.
- An unreadable version (written by a newer version of `tri`?) is kept as is
  (`Versions::unknown`).
- **Undoable**: `history::Step::Versions` keeps the list before and after. If the sidecar
  has been modified elsewhere since, the undo is refused and nothing is overwritten.

## Interface (`gui/src/crop.rs`)

- **`v`** (`core.version.new`, 2026-11-07; `c` before, `r` at first): a new version of
  the displayed image (from the grid, opens full screen). You crop what you see, at the
  fitted scale. **`Shift+V`** / **`Ctrl+Shift+V`**: next / previous version (the original,
  then each version).
- Translucent grey **veil** around the frame, white frame with a dark edge, eight
  **handles**: corners and sides with the mouse, drag inside to move. The chosen overlay
  (`o`) snaps to the frame.
- **Bar** under the image, in parts titled by a label in the accent colour, like the other bars (2026-11-07): **Version**
  (preset); **Crop** (ratio in a drop-down: free, original, 1:1, 5:4, 3:2, 16:9, 4:3,
  shown in the chosen orientation, 4:5; a ratio of one's own is typed there, "Custom":
  `17:23`, `4/5`, `1.91:1`, `Ratio::parse_custom`, reduced to simple whole numbers; then
  the orientation, an icon: the frame's rectangle, standing or lying, its tooltip names
  the other one); **Export** (optional
  export preset); **Name** (it follows the ratio as long as it has not been typed) and,
  next to it, the floppy (save as a preset; greyed out without a name); Save / Cancel on
  the right. In the ratio list, the custom ratio field ("Custom" in grey inside it) and a
  "+" that adds it to the list, kept in the configuration (`[versions] ratios`). The
  filter bar, and the filters and histogram of the right panel, disappear while editing (nothing to filter on one
  image; the histogram is the whole image's, following the frame would need a RAW
  histogram recomputed on the area, later).
- **Esc** (or Cancel) goes back to where the version was started: the grid or full
  screen (2026-11-07); once saved, it is shown full screen.
- **Each part from the keyboard** (2026-11-07): `v` / `Shift+V` next / previous version
  preset (like `Tab` / `Shift+Tab`), `r` / `Shift+R` next / previous ratio (added ones
  included, `Crop::step_ratio`), `/` a ratio of one's own (numeric keypad too; the list
  opens on its field), `e` / `Shift+E` next / previous export preset ("none" included),
  `x` the orientation, `o` / `Shift+O` the overlays (fitted to the frame), `n` (or `F2`)
  the name (Ctrl+Esc leaves it without leaving the version, as on the lists), `Ctrl+S` the floppy, `0` the original frame (the whole image; also
  Ctrl+click on "Original", `Crop::reset_frame`); with `1`–`7`, `x`, `Tab` /
  `Shift+Tab`, Enter, Esc.
- **Unique names** per image (issue #14, 2026-10-22): the proposed name (ratio or preset)
  is already numbered if taken ("Story (2)", `Versions::new_name`); a typed name that
  already exists, case and spaces aside (`Versions::name_taken`), turns red and Save is
  greyed out (Enter too: message). A reopened version keeps its own name
  (`Versions::without`). "Original" is **reserved** for the uncropped image in the
  versions list (feedback of 2026-10-23: two indistinguishable "Original" entries): a
  version at the image's ratio is first named "Version" ("Crop" before 2026-11-07).
- **Keyboard**, "crop" context (`Scope::Crop`, takes precedence while the frame is shown):
  - arrows (classic) or `hjkl` (vim) to move;
  - with Shift: right side (left / right) and bottom (up / down);
  - digits 1 to 7 (with or without Shift, AZERTY) for the ratio;
  - `x` for the orientation, `o` / `Shift+O` for the overlays;
  - Enter to save, Esc to abandon.

  The keyboard step: `[gui.crop] step`. Everything is configurable in `[keys]`.
- **Versions** of an image in full screen: a "Versions: Original ▾" list on the notes
  line (issue #5, 2026-10-22: on a line of their own, they came and went with the image
  and changed its size; chips at the end of the line were covered by the icons on the
  right on a narrow screen):
  - "Original" then one entry per version;
  - for the version shown: Edit (reopens its frame, on the image it was made on) and
    Delete (undoable).
  - `n` cycles through the original then each version.
  - A version shown displays only its frame, fitted to the view; its name follows the
    source in the label. Marks placed on the sensor (focus point, zones) are hidden there.
- The frame of a new version follows the actual proportions of the displayed image: as
  long as they are unknown, nothing is computed, and saving waits for the first draw. There
  are no more assumed proportions (3:2): a version saved too early would have had a wrong
  frame.
- `[gui.crop]` settings: `step`, `shade`, `shade_opacity`, `line_width`, `handle_size`,
  `handle_grab`.

- **Grid**: badge in the caption of thumbnails that have versions, left of the format
  (drawn crop icon and their number; at the top left until 2026-10-21); a click starts
  cropping. Versions read from the sidecars on demand, then kept; read again after a
  modification (save, undo, folder watching).
- The crop bar only offers the export presets that apply to the cropped image
  (`applies_to`), and **never an "as is" preset**, whatever the image. A crop changes the
  image, so a preset that changes nothing makes no sense. User question (2026-10-19): "as
  is" is deduced from the preset (`Profile::is_as_is`: JPEG, no recompression, original
  colour space, full size, no sharpening), not from a setting. A user preset set up that
  way is treated the same.
- Export of a version without a preset: the chosen preset, otherwise the first JPEG preset
  (Web by default), rather than a retouching TIFF.

## Exporting a version

- `Ctrl+E` on a version shown exports it, and it alone, on the image it was cropped on
  (its `source`); the dialog is titled "Export the "Story" version" and pre-filled with its
  preset if it has one (otherwise the last one chosen, otherwise the default preset).
- File: `DSCF0001-Story.jpg` (`export::Cut`, name cleaned of characters forbidden by the
  systems: `3:2` gives `3_2`).
- **LibRaw or camera JPEG**: render, then crop, then downscale and write (`export_libraw`
  with `Some(cut)`). No copy without recompression: cropping means recompressing.
- **darktable**: `export_version_darktable` renders a **temporary 16-bit TIFF** in the
  preset's colour space, at full resolution; it is then cropped, downscaled and written in
  the preset's format, with the TIFF's ICC profile. There is thus only a single
  compression, and the TIFF is deleted. Checked on 2026-10-19 on the user's RAF (ignored
  test `darktable_exports_a_cropped_version`).
- The frame is in proportions of the **viewed** image: on a cropped darktable development
  (2763 × 4912 for the test RAF), these are its proportions, not the sensor's.

## Crop presets

User idea (2026-10-19): "Insta story" = a name, a ratio with its orientation (9:16) and an
export preset (Web). You choose the preset, adjust the frame, Enter.

- **Storage**: `[versions.presets.<id>]` (`VersionSettings`, `Presets`), with `name`,
  `ratio` (`"free"`, `"original"`, or `"l:h"` in the desired orientation) and `profile`
  (`""`: none). A missing key comes from the built-in preset with the same identifier, as
  for export presets. Built-in presets: `story` (9:16, Web) and `post` (4:5, Web).
- **Crop bar**: "Preset" list; `v` / `Shift+V` or `Tab` / `Shift+Tab` moves to the next / previous one
  (`p` until 2026-10-21: it is also the sharpness map, which stayed on; `p` turns it off
  during cropping too).
  - Choosing a preset applies its **exact** ratio, its name and its export preset.
  - The export preset is only applied if it exists and applies, and never an "as is"
    preset.
  - The frame re-centres to the largest of that ratio, then you adjust it.
- **Save as crop preset** (small floppy disk next to the presets list; Save and Cancel on
  the right of the bar, apart, user feedback of 2026-10-20): the ratio, name and export
  preset of the current frame are written to the configuration file, without losing the
  comments (`config_edit`). The identifier is derived from the name (`versions::new_id`:
  lowercase, accents removed, a number if it is taken). Applied live.
- Settings window: a single "Crop presets" entry, drop-down list of the presets, "New…"
  (copy of the one shown), "Rename…", "Delete" (the user's own), unique names (see
  `export.md`); drop-down list of the ratios for
  `ratio` (like export presets, see `export.md`).

Along the way (2026-10-19): export presets, crop presets and darktable settings modified
in the settings window are now applied **live** (previously, a restart was needed).
