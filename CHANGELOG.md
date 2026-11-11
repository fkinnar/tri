# Changelog

*[Version française](CHANGELOG.fr.md)*

## Unreleased

- **Colour charts** (#425): `cargo run --release --example colour_charts -- <folder>`
  writes the same chart in every common colour space (sRGB, Adobe RGB, Display P3,
  ProPhoto, Rec.2020, linear ones), as JPEG, TIFF and PNG with their profile, and checks
  that tri reads them all alike. The Display P3 profile given to iPhone HEIC files no longer
  carries a wrong cinema-P3 tag.
- **Colour management** (#423): every image is shown with its real colour space instead
  of being assumed sRGB: camera JPEGs (Adobe RGB too), flat images with their ICC profile
  (16-bit TIFF / PNG converted at their depth), HEIC, and developments with the output
  profile of their history (ProPhoto…; tri never forces one). They go through one wide
  working space (Rec.2020 primaries, sRGB curve), converted once when cached. On Linux and
  Windows, images are converted to the screen profile (`[gui] display_profile`, else
  colord's or Windows's); macOS does it by itself. RAW renders are now read with their
  real tone curve: a little brighter than before, mostly in the shadows. Thumbnails and
  previews are recomputed once.
- **Colours follow the screen profile on macOS** (#421): tri now declares its window in
  sRGB to the system, which converts to the screen profile (calibration included).
  Before, colours went to the screen unconverted: far too saturated on a wide-gamut
  screen.
- **Faster first grid at launch** (#419): thumbnails in RAW or developed render keep a
  small copy in the cache the first time they are reduced; later launches read it instead
  of the whole preview, and the grid no longer fills in one by one.
- **Development thumbnails no longer swap on opening** (#418): a thumbnail whose
  developed render is not known yet (its sidecar being read) stays empty a moment
  (`[gui.grid] render_wait_ms`) instead of showing the camera JPEG and then changing image;
  stack covers too. Same for a thumbnail whose viewing adjustments are still being read: it
  was shown without them, then every adjusted thumbnail was decoded again at once (half a
  second of frozen interface when opening an adjusted folder), and a thumbnail is no
  longer redone when its adjustments are read back unchanged. Stacks opened folded (#417)
  also keep their covers: shown first with their first image, they switched to the real
  cover once ratings and sharpness scores were read; the covers are now remembered too.
  And the grid waits for the folder's ratings: sorted by colour, its first frame showed the
  name order without marks, then reordered. The ratings are read with the folder's list,
  within the opening wait (`[gui.folders] open_wait_ms`): the previous folder stays shown
  until the new one arrives complete.
- **Stacked folders open already folded** (#417): the stacks of a folder are remembered in
  its cache after a complete grouping; reopened with nothing changed, it shows folded at
  once instead of folding on screen. Otherwise the grid waits for its stacks a moment
  (`[gui.grid] stacks_wait_ms`), with a "Grouping stacks…" box on large folders
  (`[gui.grid] stacks_overlay_from`).
- **More kept per folder** (#415): the image where one was (back to it when reopening
  the folder), the unfolded stacks, the "Subfolders" checkbox (`[gui.folders] subfolders`
  for a folder never set; no longer common to all folders), the stack time gap, and the
  software ticked in the "developed" filter. A folder coming from another computer keeps
  the render and the software chosen there, even when they are missing here.
- **"Stack automatically" kept per folder** (#413): like the filters, the sort and the
  thumbnail render; a folder never set takes the configuration's value.
- **Reset a folder** (#412): right click on a folder, Preferences, "Reset this folder…"
  ("this drive…"): one dialog with a checkbox per thing to clear, each with what it
  concerns, measured in the background: cache, tri's data (filters, sliders, covers…),
  ratings and versions. Ratings are reset inside the sidecars, never deleted: only
  stars, colours and flags change, so darktable's or Lightroom's sidecars keep the rest.
  Ratings need "File actions" allowed. Ratings and versions can be undone, in one step.
  "Clear the cache…" opens the same dialog with only the cache ticked, now on every
  folder. Command line: `tri <folder> --reset cache,data,notes,versions [--dry-run]`.
- **Everything on the drive** (#410): "Keep tri's data in this folder / on this drive"
  (right click on a folder or a drive) now keeps, besides the cache, everything `tri`
  remembers about the folders below: filters and sort, slider values, stack covers,
  preferred developments, ratings memory, in a hidden `.tri` folder, by file name only.
  A drive carried to another computer (or a synced folder) keeps everything. Off by
  default: nothing is then written next to the photos (before, `.tri/notes.json` was).
  The offload dialog offers it, ticked when the destination is a removable drive. Moved
  on first opening: the old `.tri-cache` becomes `.tri/cache`, and what the data folder
  kept per folder joins the folder's files. To sync a folder without its cache, exclude
  `.tri/cache`.
- **Dialogs in the centre** (#409): confirmations and dialogs open in the centre of the
  window from their real size (a fixed offset put them askew), the right-click rating
  dialog included.
- **Clear the cache kept with the photos** (#409): on a folder or drive that keeps its
  cache (`.tri-cache`), right click, Preferences, "Clear the cache of this folder…" (or
  "of this drive"), confirmed by a dialog with its size; the cache stays kept there. An
  action, which can be given a shortcut (`core.folders.cache_clear`).
- **Clear the cache from the interface** (#408): at the top of the "Cache" section of the
  settings window, the size of the thumbnails, camera previews, RAW previews, 1:1,
  developed renders and analyses, and the total; "Clear" per row and "Clear all",
  confirmed, in the background, the freed space announced. For example, clear the 1:1
  without losing the thumbnails, or the developed renders after a darktable update.
- **Tidier right-click menu on a folder** (#407): in groups, Access (show in the file
  manager, named: Finder, Explorer, Nemo, Dolphin…; open in darktable, favourite), Folder, Ratings, Development and Preferences; offload
  stays at the bottom. Each group is a submenu that the small button at
  its top right puts at the top level, between two lines (and the icon on its title folds
  it back): everyone arranges their own menu, kept for every folder
  (`[gui.folders] menu_inline`).
- **Viewing adjustments** (#404): exposure, white balance and black and white (channel
  mixer), to compare and cull a series. A new "Adjustments" section in the right panel,
  in tabs: Base (exposure with "Auto", highlight priority; balance in kelvins and tint, as shot, auto on the nearly neutral pixels,
  daylight, cloudy, shade, tungsten, fluorescent and flash presets; eyedropper: click a
  grey zone or draw a frame for an auto balance on that part), Style (creative offset: exposure, warmth, tint, contrast,
  saturation),
  B&W (filter presets, mix kept at 100 % when normalised) and Values (the table to copy
  into a development software); a double-click on a tab resets it; the open folder's right-click menu sets automatic
  exposure, white balance or both on all its images, in the background. With several images selected, "Sync N images" applies
  every change to all of them (a slider shifts each image by as much, temperature by the
  same ratio; a button, auto or preset, computes each one; one undo step). Also copy /
  paste adjustments (exposure and balance matched to the copied image), "Harmonise the
  selection", "Before" to compare. Stored in the sidecar under `tri:`; shown on
  thumbnails and large images. Every slider of the interface now moves by large steps,
  and finely with Shift (`[gui] slider_fine_speed`); both side panels have a minimum
  width (280 points) that the dragged edge respects.
  `tri --adjust <reference> <images>… --to <folder>` sets the reference and applies its
  adjustments to the others: exposure and balance matched to the reference (a photo at
  -1 EV gets about +1 EV), creative offset and black and white copied. It writes the
  adjusted JPEGs and prints the values to copy (`--ev auto`, `--kelvin`). On a RAW,
  exposure is applied to the sensor data. New `[adjust]` section: automatic exposure
  target, kelvins of the presets (#137).
- **Application log** (#403): errors keep a short message in the status bar, and their
  full detail goes to `tri.log` in the data folder: for a development software that
  fails, the image, the command, the exit code and all its messages; also failed
  exports, thumbnails and previews, an invalid configuration, crashes (`crash.log` is
  taken in). Click an error to open the log on its detail. The log window (palette, or
  the help's "Log" button) shows the newest entries first, filters by level, searches,
  copies, and shows the file. "Report a problem" offers the latest lines, to read and
  edit before they are attached to the form. New `[log]` section (size, kept files,
  detailed trace) and `[gui] report_log_entries` (#398).
- **End of an export in the middle of the screen** (#399): the summary ("3 images
  exported", the folder) also shows in a bubble over the images, with "Open the folder"
  ("Show the report" when the files went to several folders). It fades after a few
  seconds (`[gui.grid] export_toast_ms`), waits while the pointer is on it; an error
  stays until closed, even when other exports finish after it (#381).
- **Ctrl+Shift+click in the EXIF rows** (#401): Ctrl+Shift+click (Cmd+Shift+click) adds
  the range from the last clicked row to the selection, keeping what was already
  selected; Shift+click alone still replaces it. A Ctrl+click now also counts as the
  last clicked row, as in the Finder and the Explorer (#390).
- **Search in the `?` help** (#402): with the `?` help open, typing filters the list as
  you type, with the same fuzzy search as F1 and the palette (words of both languages,
  typos forgiven); the letters found are highlighted. Esc clears the search, a second
  Esc closes; `?` still closes (#382).
- **"Developed" filter after disabling the software** (#397): with the filter on,
  disabling every development software in the settings left it visible and active until
  turned off by hand. A disabled software now leaves the filter's list (it comes back
  checked when enabled again); when none of the filter's software is active any more, the
  filter turns itself off and says so in the status bar (#395). With a single software
  left, no more empty line under the filter's box.
- **Empty flag circle on the thumbnails** (#396): with nothing set, the flag's grey circle
  (bottom right) had a thicker stroke than the colour's (top right); it reused the neutral
  icon of the rating controls, whose stroke grows with its size. Both circles are now
  drawn alike (#394).

## 1.0.0-beta.1 — 2026-11-09

First beta: ratings kept in step with darktable and inside the images (embedded ratings,
merge and conflicts, colour lists), command palette and help, zoom and loupe from the
keyboard, comparison of up to 21 images, versions, a review of the shortcuts, and the
fixes from the end-to-end tests on macOS, Linux and Windows.

- **Image count of the filter bar** (#386): "570/571" for 570 photos; a RapidRAW sidecar
  (`.rrdata`) counted in the folder total while its development was hidden, RapidRAW
  being absent or disabled. The total now leaves out the developments of such software,
  like the grid.
- **Several EXIF rows** (#386, fixes #384): Shift+click, Ctrl+click or Cmd+click to add or remove a row,
  Ctrl+A and an icon next to the "EXIF" title to select everything; the selection is a
  grey band across the row instead of blue boxes. Right-click on a row copies its value,
  the selection or everything.
- **Hovering in the side panels** (#386, fixes #383 and #385): the row under the mouse
  (folders, EXIF) gets the same light grey band as the keyboard cursor, across the whole
  width, and becomes the pointed row; the open folder keeps its accent background
  separately. Hovered controls lose egui's white frame everywhere, like the bar icons.
  The metadata copy shortcut (`y`, `Ctrl+C` / `Cmd+C`) copies the EXIF row under the
  mouse without a click. The pointer is a hand over everything clickable in the bars
  and panels (buttons, checkboxes and their label, lists, folder rows), like in dialogs, instead
  of a mix of hand, arrow and text cursor; no text cursor over the bars' texts either.
- **Leaving a comparison opened from full screen** (#379, fixes #376): it went back to
  the grid; it now goes back to full screen, on the active image ("Back to full screen"
  button). Opened from the grid, it still goes back to the grid.
- **Clearer note icons** (#374): "Write ratings into the images" and "Read ratings
  back from the images" share the same picture (a photo with its star) with an arrow
  going in or coming out, so one reads as the reverse of the other; "Generate sidecars"
  shows a sheet with a star and a "+".
- **Show a folder in the file manager** (#375): right-click on a folder, "Show in
  Finder" (Explorer, file manager), like for an image.
- **darktable sees colors set in tri** (#373): the `darktable:colorlabels` list was
  written after darktable's history, and darktable then ignored it when syncing changed
  XMP files at startup. It now goes before the history, where darktable puts it. Sidecars
  already written move it at their next color change.
- **Cited shortcuts up to date** (#372): setting descriptions and a few messages
  (full-screen key reminder, "Before" of a development, version presets…) cited
  hard-coded shortcuts, some of them stale ("Shift+B" to see one's developments, "b" for
  the render). They now cite the action, and the interface shows the shortcuts in use,
  `[keys]` included; a test prevents writing them hard-coded again. An action without a
  shortcut no longer has an empty "()" in its tooltip.
- **Sidecars of both conventions** (#371): `[xmp] sidecar = "both"` creates and keeps up
  to date `IMG.RAF.xmp` (darktable) and `IMG.xmp` (Lightroom, Capture One). Two XMP
  sidecars of the same photo are now both read and merged (before: the most recent only).
  "Generate sidecars" creates the ones the convention expects (set to Adobe, a folder of
  darktable sidecars gets its `IMG.xmp`). Drop-down in plain words: "Darktable",
  "Adobe", "Both", with the file name in grey. The disagreeing-ratings dialog shows each
  source's modification date (a hint: a 2012 sidecar against a 2026 one).
- **Colors: one or several, and fields per program** (#371): in one-colour mode,
  `xmp:Label`, darktable's list and digiKam are written; when `Label` and darktable's
  or digiKam's colour differ (darktable never touches `Label`, which could hide a colour
  changed there), the reading order `[xmp] color_sources` decides (darktable, Label,
  digiKam by default; dragged into place in the settings window). In
  several-colours mode, like darktable, only its list is read and written.
  `[xmp.fields]` (a table in the settings window): one box per program (Label, darktable, digiKam, Lightroom), all
  ticked; an unticked field is neither read nor written, stays as it is and leaves the
  reading order. Drop-down list for `[xmp] embed` in the
  settings window.
- **Ratings kept in step by themselves** (#368): with `[xmp] embed` = `internal`
  (internal and fixed disks) or `everywhere`, each rating also goes into the image
  (JPEG, PNG, WebP, TIFF, HEIC), and a rating merged from another program is written at
  once into the other sources (an XnView colour reaches darktable's XMP). A folder can
  have its own setting, which wins (right-click on the folder, "Ratings in the images":
  as in the settings, always, never), kept in its `.tri` folder and marked in the folder
  panel; it does not apply to subfolders.
- **Ratings changed in several programs are merged** (#368): when darktable changes the
  stars and RapidRAW the colour (or XnView the image itself), both are kept, field by
  field, instead of the most recent sidecar winning, and the status bar says so. `tri`
  remembers what each source said last time (`.tri/notes.json` next to the photos, in
  `tri`'s data folder for a read-only folder). Two sources changing the same field
  differently, or sources never seen together that disagree: an orange sign on the
  thumbnail, and a dialog to choose which ratings to keep (also for every conflict in
  the folder at once); `tri <folder> --conflicts` lists them.
- **Ratings in the images themselves** (#276): JPEG, PNG, WebP, TIFF and HEIC can carry
  their ratings (embedded XMP, without re-encoding, nothing else lost), to find them in
  Windows Explorer, Spotlight, Lightroom, XnView… Read when an image has no sidecar
  (stars set elsewhere). Two actions, on the selection or from the folder menu: "Write
  the ratings into the images" and "Read the ratings back from the images" (changed in
  another program; undoable); `tri <folder> --embed-notes` / `--import-notes`. Never
  into a RAW.
- **No more crash when the graphics memory is full** (21 images compared at 1:1): the
  large textures have a budget (`[gui.view] textures_mb`, 1500 MB); beyond it, an image
  keeps its preview. A graphics card error is now logged and shown instead of stopping
  `tri`; the texture in cause is dropped and the session's budget lowered to what fitted.
  In a zoomed comparison, such an image is enlarged at the same scale as the others, with
  a warning sign. One line in the terminal, the details in `crash.log`. A "Keep N MB"
  button writes the lowered limit into the configuration.
- **A development zoomed at the scale of its photo**: at 100 %, a darktable or RapidRAW
  development (rendered at the preview size) looked ~2.4 times smaller than the photo
  next to it; it now matches the photo's 100 %. An info icon says it is enlarged, so its
  sharpness at that scale is not the file's.
- **Leaving a comparison of everything** gives back the grid as it was: nothing
  highlighted, so the next comparison takes everything again.
- **"Comparison limited to N images"**: a bubble, placed and timed like the position
  mark (`[gui.compare] limit_seconds`, `limit_place`), instead of a status-bar line.
- **Crop and version are one concept: "Version"**: a version is a frame with an optional
  export preset. The interface says "Version" everywhere; `v` creates one (instead of
  `c`), `Shift+V` / `Ctrl+Shift+V` go to the next / previous one; "crop" still finds it
  in the help and the palette. The action ids become `core.version.*`; the old ones
  (`core.crop.*`, `core.view.crop`…) still work in `[keys]`. The version bar is
  organised in titled parts (Version, Crop with the ratios in a drop-down, Export,
  Name); a custom ratio can be typed there (`17:23`, `1.91:1`) and added to the list
  with "+" (kept in `[versions] ratios`). Each part has its keys (`v` version preset,
  `r` ratio, `x` orientation, `o` overlays on the frame, `/` custom, `e` export, `n`
  name, `Ctrl+S` save as a preset, `0` or Ctrl+click on "Original": back to the original
  frame); Esc goes back to the grid when the version was started there; the orientation
  is an icon (the frame's rectangle, standing or lying). The filter bar, and the
  filters, view settings and histogram of the right panel, disappear while editing.
- **Big bug: RawTherapee and ART developments were exported without their development**
  (as their photo, a plain LibRaw render). They are now rendered by RawTherapee / ART, in
  full resolution (16-bit TIFF in the colour space of their profile), then converted,
  cropped, reduced and sharpened like the other exports. Interface and command line.
- **Back to 100 % and to fit from the keyboard**: `<leader> m m` sets the loupe back to
  100 %, `<leader> z z` the zoom to 100 %, `<leader> z /` fits it. In the menu of
  continuations, `m` and `z` are sub-menus listing what can follow.
- **Comparison from the bottom bar**: a button (lit while comparing, where it leaves the
  comparison), and the back-to-grid button in comparison too. Without a selection or a
  stack, everything shown is compared (at most `[gui.compare] max`, around the current
  image); with nothing highlighted in the grid, `=` does it at the first press.
- **Cmd / Ctrl+click takes the current image too**: without a selection, the highlighted
  image counts as chosen, so the first Cmd / Ctrl+click on another one selects both.
  With a selection and the grid cursor hidden, `=` (and ratings, file actions) act at
  once instead of only showing the cursor again.
- **One zoom control in the bottom bar**: the zoom and loupe controls are merged. It sets
  the zoom (round magnifier), or the loupe while its key is held (square magnifier). A
  click on the icon resets, a click on the value lets you type it. The pointer stays
  visible while the loupe key is held, the loupe no longer shows over the bar, and the
  image info switch moves to the far right.
  `TRI_GUI_TRACE` also shows the keys received.
- **Typed zoom and loupe on Linux**: after `<leader> m` or `<leader> z`, the top-row
  digits of an AZERTY keyboard (with Shift) changed the rating instead of the value:
  Linux also sends Shift itself as a key, which closed the entry. A lone modifier is now
  ignored.
- **Zoom settings in percent**: `[gui.zoom] steps`, `max`, `min` and `loupe_scale` are
  written as percentages (`max = 400`), and absurd values are refused (outside 1 to
  999 %, `max` below 100 %, `min` above). An existing file with the old values
  (`max = 4.0`) is reported as invalid. The value in the bottom bar keeps a fixed width.
  In the settings window, a typed number is checked on Enter or when leaving the field
  (not at each digit), and the error shows the message alone.
- **Comparison**: the active image no longer jumps back to its stack's cover when the
  stacks are recomputed during the comparison (a thumbnail arriving, slower machines).
- **Controls in the command palette**: under an action that has a control in the right
  panel (a box, a list, a slider), the palette shows that same control, with its state,
  to change it without closing the palette. Panel and palette draw it with the same code.
  From the keyboard: ↓ goes to the control, ← / → change it, Enter activates it. The
  search finds the panel's words ("sharp") and puts actions before settings. The
  thumbnail size has its own row.
- **A precise shown render from the palette**: one action per render (camera JPEG,
  cover, each development software, RAW render), without going through the others. No
  default key; `[keys]` can give them one.
- **One style for the key lists** (F1, `?`, the menu of continuations, the palette):
  keys in the accent colour, one row in two on a lighter background, and the "/"
  between several keys in grey so the keys stand out.
  In the menu of continuations, a label shared by most rows goes to the title (`f`:
  "Filter"); no details in brackets there (F1 and the palette keep them); shorter labels for the star
  filters and the colours (no "(add / remove)").
- **No 1:1 button in the grid**: the adjustable loupe replaces it; `z` still opens the
  selected image at 100 %.
- **"All thumbnails on screen" is a switch** (button and `*` in the grid): on, the
  thumbnails shrink to show them all; off, they get their size back. Changing the size by
  hand turns it off.
- **Palette history**: with an empty field, the command palette shows what was launched
  from it, most recent first; ↑ / ↓ and Enter launch it again. Kept between sessions
  (`[gui.dialogs] palette_history`, 20).
- **Stacks: pauses and slider progression** (#306): a pause longer than the folder's
  pace moves two images apart (a change of pose in the studio often falls on one). Two
  new sliders in the grouping window (`Ctrl+G`), remembered per folder: "Weight of
  pauses" (to the left, content only; to the right, pauses cut more, e.g. street
  photography) and "Slider progression" (to the left, each pixel removes the same share
  of the stacks; above 1, a gentler start from the middle and a faster right side).
  Fully to the left, the strictness slider now groups everything. Settings `[similarity]
  pause_weight`, `pause_range`, `strictness_curve`, `strictness_curve_max`. The window
  closed with Esc or `Ctrl+G` now also remembers the strictness.
- **Comparison**: up to 21 images by default (`[gui.compare] max`, 6 before: about the
  size of the thumbnails on a screen), and the loupe (`t` held) on the image under the
  pointer, like in full screen.
- **Stack filter button** in the filter bar, with stacks on: shows only the current
  stack (`f c`), lit while looking at it, click again to see everything.
- **Default shortcuts in a file** (#280): no shortcut is written in the code any more.
  The classic, vim and common sets are in `keys.toml`, shipped next to the program and
  read at startup. It lists every action, with `[]` for those without a shortcut. A
  built-in copy takes over if the file is missing or unreadable; an unreadable file is
  reported in the status bar. Your own shortcuts stay in `[keys]`. The panel prefix
  (`leader`, Ctrl+b) is in the file too; `[gui] leader` is now empty by default and
  replaces it only if you set it.
- **Shortcut review** (#280): one letter per domain, from its English name (c crop, i
  histogram, m magnify, n order, r render, s stacks, t text, v versions…). Shift + the
  letter is its variant, Ctrl + the letter the settings of the domain, and `<leader>`
  + the letter + a key a secondary setting (`<leader> i s`: histogram scale). The
  panels show and hide with `<leader>` Shift + a direction. The "both" profile is gone,
  and the loupe moves to `m` and into `keys.toml`. A leader or loupe key already bound
  to an action is refused. The full before / after table is in `doc/en/gui.md`.
- **Zoom like the loupe**: `<leader> z` then a number without Enter (`150`, `20` =
  200 %, `2` = 200 %); `Ctrl+K` / `Ctrl+J` change the zoom one step (the loupe if its key
  is held); in the bottom bar, next to 1:1, −, the value ("Fit" or the percentage; a
  click: fit or 100 %) and +.
- **Previous version** of an image in full screen: `Shift+V`, the other way round from
  `v`.
- **Small histogram** follows the chosen channel too (`<leader> i r`, `g`, `b`, `l`, `a`),
  not only the large one. Choosing a channel no longer opens the large histogram. In
  the large histogram, `s` changes the scale and `c` goes to the next channel.
- **Menu of continuations** (like which-key in nvim): after the start of a sequence
  (`<leader>`, `f`, `<leader> r`…), a short moment without a next key shows the possible
  keys and what they do at the bottom of the screen. The menu then waits for the next
  key without a delay; Esc abandons it. Settings `[gui] key_menu` and
  `key_menu_delay_ms` (400 ms); sub-menus named in `keys.toml` (`[menus]`).
- **Help here**: `?` shows over the screen the shortcuts of where you are (grid, full
  screen, comparison, crop, large histogram, folders or metadata panel), arranged by
  theme, in large type. `F1` remains the full list with a search. Settings
  `[gui.dialogs] help_here_text` and `help_here_column`.
- **Help search**: it also finds the words of the other language and forgives a typo
  in a long enough word ("histrogramme"); keywords per theme ("rate" finds the stars);
  an action with the word in its label comes first. Typos and ranking also apply to the
  settings search.
- **Typed loupe magnification** (`<leader> m`): one second between digits (600 ms
  before), and a digit typed just after still completes the number instead of rating the
  photo (with Shift on an AZERTY keyboard, typing is slower).
- **Command palette**: `Ctrl+Shift+P` (classic) or `:` (vim), type a few letters, Enter
  runs the action found or opens the setting found in the settings window. Each action
  shows its keys; actions without a shortcut become reachable from the keyboard.
- **Easier help**: for an action that has keys of its own in the chosen set (vim or
  classic), the help, the help here and the texts show only those (in vim, "x" to
  delete, without Delete); the common keys still work.
- **Search for a key**: in the help and the palette, "ctrl f" (or "Ctrl+F", "shift p")
  only shows the actions bound to that key. A single letter now only finds single
  letters (a key, "x"), no longer the "f" inside words.
- **Sort by…**: one action per order (name, capture date, modification date, stars,
  colour, flag), besides "Next order": in the palette ("sort name"), without a default
  shortcut (set one in `[keys]`). The palette gets a title and a roomier field.
- **Palette prefixes**: `>` only searches the actions (as in VS Code), `?` only the
  settings; alone, the prefix lists them all. Configurable:
  `[gui.dialogs] palette_actions_prefix`, `palette_settings_prefix`. The palette takes
  the look of the other dialogs (colours, margins, title, cross).
- **Search in the grid only**: the name search field is no longer shown in full screen,
  where `/` goes back to "fit". The help writes `/` for the search (alone in vim, with
  `Ctrl+F` in classic).
- **Palette, what it did not find**: the labels of the panel checkboxes ("Stack
  automatically"), the plural ("thumbnails render"), and three actions for the RAW / JPEG
  switches (RAW only, JPEG only, both). A line found by the words of its title comes
  first (palette and help).
- **Loupe in the bottom bar**: − and + change the magnification (they did nothing), and
  the magnifier icon is no longer a button.
- **Greyed-out filter**: "Developed", greyed out when no photo of the folder is
  developed, no longer turns on from the palette (left out) nor with `f r` (refused, with
  the reason in the status bar); turning it off still works.
- **The loupe in the help**: "loupe" finds it in the help, `?` and the palette, with its
  key (`m`, held); run from the palette, it reminds you to hold it.
- **One shown render**: "Shown source" and "Development of each photo" become one list,
  "Shown render", the same in the grid and full screen (camera JPEG, cover, each
  software, RAW render). `r` / `Shift+R`: next / previous render for the whole folder,
  kept from photo to photo; on a development under review, `r` stays before / after.
- **Review of developments tooltip**: it names the keys of the chosen set (`h` / `l` in
  vim, no more hard-coded "← / →") and `( (` to unfold a photo, instead of keys that no
  longer existed.
- **Loupe from the keyboard**: with the loupe key held (`m`), the arrows (`hjkl` in vim)
  move it without changing image, by a large step with Shift; moving the mouse gives it
  back to the pointer. Steps: `[gui.zoom] loupe_step`, `loupe_step_large`. The loupe also
  works **in the grid**, on the thumbnail under the pointer (from the keyboard: the
  selected one).
- **Panel checkboxes in the palette**: "Review the developments" and "Actions on files"
  have their action, which toggles the current state (the palette only offered their
  default value). Settings lines in the palette are now called "configuration": they
  open the default value.
- **Keyboard loupe steps**: measured in the loupe (the image at 100 %), the same in the
  grid and full screen: a quarter of a loupe, a whole loupe with Shift (on a thumbnail
  they jumped hundreds of pixels).
- **Right panel**: each section starts at the same distance from its title ("View" and
  "Grid", which open on a subheading, started lower).
- **Palette without filter**: it offers every action and every setting, wherever the
  keyboard is (from a side panel, "passer" did not find the review of developments), with
  no line limit by default (`[gui.dialogs] palette_max = 0`); what applies here comes
  first, and an action greyed out here says why when run.
- **Arrows as symbols**: in the help, the menus and the palette, arrows and Enter are
  written as symbols instead of "Left arrow"…, taken from a system font (Apple
  Symbols, DejaVu Sans, Segoe UI Symbol). Without one, spelled out. Settings `[gui] arrow_font`,
  `arrow_font_scale`; `[keys]` still accepts the written names.
- **Labels without keys**: the stacks one spoke of "left / right arrows" (in vim, `h` /
  `l`); labels say what the action does, its keys are shown next to it, those of the
  chosen set.
- **Loupe at the edge of the screen**: it stays inside the visible area (at the top or
  bottom, half of it went off-screen, mostly in the grid and focus mode), the aimed point
  at the centre of what it shows.
- **Arrows or hjkl, by key set**: after the leader, the menu of continuations and the
  help showed "h / Left"; they now show only `h` in vim, the arrow in classic (both keys
  still work).
- **Easier menu of continuations**: each key on the line of its label, and the order of
  `keys.toml` (purple with the other colours, no longer first because of "°"); columns
  separated by a rule and some room, each at its own width. A faint line between rows,
  there and in the help here (`?`).
- **Developments of active software only**: without development software, or with one
  disabled, its developments leave the grid (no more badge nor stack to unfold); its
  sidecars stay, enabling it again brings them back.
- **Loupe magnification from the keyboard**: `Ctrl+K` / `Ctrl+J` (or `+` / `-` with the
  loupe held) one step; `<leader> m` then a number, without Enter: three digits at once
  (`150`), two as tens (`20` = 200 %), one as hundreds (`2` = 200 %) after a short pause
  (`[gui.zoom] loupe_digits_ms`). The wheel, with the loupe held, changes it too (the grid
  does not scroll). The value shows in the bottom bar (loupe, −, value, +; a click on the
  value: 100 %). For the session.
- **Bar icons without a background at rest**: the buttons on the right of the bottom bar
  (grid, comparison, stacks, arrows…) had a slightly different background from the bar;
  like the other icons, they only get one under the pointer.

## 1.0.0-alpha.8 — 2026-11-05

Eighth and last test version before the beta: development software (darktable, RapidRAW,
RawTherapee, ART), network shares, stacks, accent colour and settings, fixes from the
end-to-end tests.

- **Filter count bubble** counts the photos like the status bar: a development shown on
  its own (clone, copy) no longer counts as one more image. Under "Developed", a software
  without any development in the folder is greyed out.
- **Image info icon** in the bottom bar (full screen, left of the 1:1): shows and changes
  whether the infos on the image are shown (`Ctrl+I`); hidden, the position mark goes too,
  like the notes.
- **Fixes from the end-to-end test**: the grid's own development renders stop when
  another folder opens (#303), without the line above the tasks; moving a strictness
  slider of an active filter shows the image count (#304); the "Developed" filter is
  greyed out in a folder without any development (#305).
- **End of the stack or of the list** (#307): in full screen, ← / → that no longer move
  say so for a moment ("End of the stack", "End of the selection"…), place and time in
  `[gui.end]`; it goes away as soon as another image shows.
- **"Developed" filter with a single software** (#301): one box instead of two, and no
  "None checked" line. Background tasks at the bottom of the right panel take its
  background colour (#300).
- **"Shown source" in the grid's order**: camera JPEG first, then the software, RAW last.
- **Development of each photo** (full screen): the list goes under "Review the
  developments", without a title, and is hidden while reviewing instead of greyed out.
  Reviewing outside stack mode now goes through every development (→ skipped those of a
  folded photo, or got stuck). On a development, "Shown source" is a list (the software,
  camera JPEG) like on a photo, instead of a "Before" box. With the camera JPEG as source,
  the developments options are greyed out (no review), and their list no longer offers
  the camera JPEG: it is chosen in one place, the source.
- **Development software applied at once**: changing Auto / Manual / Disabled in the
  settings looks for the software again, no restart needed; a disabled one is no longer
  offered for the thumbnails render. "Thumbnails render" and "Shown source" use the same
  names: JPEG and RAW, with "camera" and "LibRaw" in grey, the software. In full screen,
  "Render (the same as the grid)" becomes "Development of each photo", what it chooses.
- **Folder search, way out with the mouse**: a cross at the end of the field, and a second
  click on the "server" icon, close it (a click on the icon by mistake left "smb://").
- **Accent colour** adjustable (`[gui.colors] accent`, `accent_light` for the light
  theme), with a colour picker in the settings: selections, sliders, and everything that
  takes the accent (stars, dialog focus line) follow it. Colours that can follow the accent
  get an "Accent colour" box instead of the word to type. The bottom bar buttons light up
  like the filter bar ones: icon in the accent colour, no background. The title colour
  moves to the colours: `[gui.colors] title` (the old `[gui.dialogs] title_color` is
  still read).
- **External editor easier to set**: "System application" (the one the system picks for
  each image type, named in the settings) or "Other application", chosen with
  "Browse…", with `{files}` already as argument.
- **Notes on the image**: same frame and principle as the position mark, display time and
  place adjustable (`[gui.notes]`), bottom bar shown or not; at the same place, the mark
  goes next to the notes. In full screen, the folder panel icon is greyed out (it only exists in the grid).
- **Development software in the settings**: for each one, Auto (with where it was found
  at launch: a forgotten AppImage shows up), Manual (a path, "Browse…") or Disabled; with
  none, the list of supported ones.
- **"Browse…"** next to every path field (copy / move to, export, offload, copy from a
  server, settings): the system's folder chooser.
- **Without a development software installed** (darktable, RapidRAW, RawTherapee, ART),
  the settings only useful with them are hidden (settings window, View section of the
  panel); their paths stay, to point to one installed elsewhere.
- **Position mark**: "Dev. n / m" only when reviewing the developments; in full screen,
  a click on the image or Enter shows it again.
- **Full screen, photo to photo**: ← / → go from one photo to the next, each in the
  grid's render, without going through all its developments (Shift+W / Shift+Q for
  them). A "Review the developments" checkbox in the View section (with the render list,
  the same as the grid's) brings back browsing every development
  (`[gui.view] review_developments`).
- **Full screen, stack mode**: in a burst, a photo with developments no longer blocks
  browsing; after its last development, → goes to the next photo of the burst (← to the
  previous one). The position mark has a subtler shadow.
- **Offload without a subfolder** (#281): a session folder left empty offloads straight
  into the root folder ("offload here" into a folder just created).
- **Centred dialogs** (#282): ejecting a device and "Develop with…" (right-click on a
  folder) open in the middle, like the other dialogs.
- **Position mark**: on each image change, in the grid and in full screen, a small frame
  says where one is ("Photo 12 / 94", with stacks "Stack 3 / 20 · 2 / 5", and "Dev. 2 /
  3" for developments); display time and place in `[gui.position]`.
- **Dialogs move by their title only** (feedback of 2026-11-01: in the settings window,
  dragging a colour dot slightly off moved the whole window); a dialog without a title
  can still be grabbed anywhere.
- Thumbnails: no more "LibRaw" chip on a photo shown in the internal render (feedback of
  2026-10-31: it said nothing); only a software's render carries a chip.
- **Settings window: a dot editor for the colour sort order** (`[sort] color_order`,
  user's idea): five empty slots under "Sort", the remaining colours under "Available";
  drag or click to build the order, written at every change and applied at once (the
  grid sorted by colour follows). **Fixed: at launch, a sort by ratings was applied
  before the ratings were read** (name order until the next change): the grid is sorted
  again once the background reading ends.
- **"Develop with": every software is greyed until the photo is developed there**,
  darktable included (it would render by default, but there is nothing to see); only the
  LibRaw render is always offered. The keyboard shortcuts say so too (user's rule,
  2026-10-31: "c'est moi qui développe").
- **Fixed: a RawTherapee / ART profile (or a RapidRAW `.rrdata`) filled after the rating
  that created it did not enter the development stack** until the folder was reopened
  (feedback of 2026-10-31): the folder watcher now treats a main sidecar that becomes a
  development like the darktable XMP rewritten by darktable.
- Thumbnail chips of clones and copies: the number alone ("DT · 1", "RR · 2"); a photo
  shown in a software's render carries the short name too ("RT", not "RawTherapee").
- **Sort by ratings** (`f s`, the sort list of the "Grid" section, `[gui] sort`): by
  **stars** (a rejected photo below 0 stars), by **colour** (order of the new
  `[sort] color_order`, which may name only a few colours: the others and the uncoloured
  go last) or by **flag** (accepted, no flag, rejected). A rating set, undone or changed
  elsewhere sorts the grid again at once, the selection follows the image. Colours are
  written `"red"`, `"yellow"`… in TOML and in the saved folder views (the former
  `"Red"` is still read).
- **Sidecars dialog: installed software only** (XMP always), like the render list; ART
  was offered without being there (feedback of 2026-10-31).
- **RawTherapee / ART as an AppImage on Linux**: recent AppImages only launch the
  interface, so `tri` extracts the AppImage once into its cache and runs the extracted
  `rawtherapee-cli` / `ART-cli` with the bundled libraries. An ART AppImage is recognised
  by its name (`ART_*`), no longer confused with darktable's ("art" is in "darktable").
- **Fixed: the RawTherapee / ART render of a development failed** (first real run,
  2026-10-31): the background batch handed the development's virtual path (`IMG@rt.RAF`)
  to the command line instead of the photo's file.
- **Stacks switch back in the bottom bar of the grid** (it had left the filter bar with
  its reorganisation, #199) and **in the "View" section** of the right panel in full
  screen, next to the shown source (feedback of 2026-10-31).
- **Fixed: the "developed" filter showed photos that were only rated, as orphan darktable
  developments** (same-day feedback). Two causes: the stacks and the renderer / filter
  had two different rules for "a darktable development", and the filter judged the photo
  and its development separately (the development owns the photo's sidecar, so only the
  development passed). One rule now, everywhere: a darktable XMP is a development only
  with a **non-empty history** (`darktable:history_end` > 0, attribute or element); an
  XMP merely imported into darktable, or written by `tri` for the ratings, is not; and
  the filter keeps a photo with its developments of the checked software.
- **Fixed: rating photos restarted the background development batch** (same-day
  feedback: twenty ratings, twenty restarts). With RapidRAW installed a rating creates
  a `.rrdata`, which the folder watcher took for a new development: it reloaded the
  folder and relaunched the batch. A `.rrdata` holding only ratings no longer counts.
- **Fixed: rating a RapidRAW-developed photo re-rendered its development in the
  background** (same-day feedback: twenty ratings, twenty renders). The render cache key
  of a `.rrdata` now covers its adjustments only, like the darktable XMP ignores the
  colour labels and the RawTherapee / ART profiles ignore `[General]`.
- **Thumbnail info hidden by itself on small cells** (`[gui.grid] info_min_cell`, 110
  points): name, format and marks were unreadable; the Shift+T setting does not change,
  the info comes back when enlarging the thumbnails.
- **Thumbnails: empty marks** (`[gui.grid] empty_marks`, replaces `neutral_flag`): where
  nothing is set, a dark outline (`[gui.colors] empty_mark`; light grey at first,
  invisible on the selected cell) of a star, a colour dot and the neutral flag's circle
  shows every piece of information and where to click.
- Thumbnails: a uniform two-point dark rim around the stars (a traced outline, the
  points thinned a scaled star) and the dots; the mark picker is as wide as its content.
- **Developments on the thumbnail: a reel icon** (the "developed" filter's icon, in the
  cell's text colour, no coloured badge) replaces "N dev."; the count is in its tooltip;
  the "fold" icon of an unfolded stack is drawn the same way, without background. Both
  are centred on the colour dot above them, at its apparent size.
- Rating bar and pickers: no more accent ring around the chosen colour, the full dot
  against the faded ones is enough, like the stars and flags. The neutral flag's ring is
  drawn a little smaller, so that it looks the size of the accepted / rejected badges,
  and on the thumbnail it is a single dark circle. The filter bar's flags and colours
  follow the same style: full colour when the filter is on, faded otherwise, no background
  nor ring. The filter bar's **flags are three combinable buttons** (accepted, neutral,
  rejected: keep any mix; the keyboard presets stay), and the analysis switches fade when
  off (then back to medium grey: the faded accent was unreadable on the bar). The "N
  dev." and "fold" badges are dark grey (`[gui.colors]
  development_mark`, a bit darker in the light theme) instead of light blue.
- **Rating with the mouse on the thumbnail itself** (#257): clicking the stars, the flag or
  the colour dots of a thumbnail opens a small picker of that group right there (the
  rating bar's controls, split in three); a star or a flag closes it, colours stay open.
- **RawTherapee and ART as development software**, on the same traits as darktable and
  RapidRAW: their main development (`IMG.RAF.pp3`, `IMG.RAF.arp`, no clones) shows in the
  photo's stack, is rendered on demand by `rawtherapee-cli` / `ART-cli` (found in the
  PATH, the usual places, as an AppImage or a Flatpak; `[develop] rawtherapee`, `art`),
  and the folder can be opened in them. **Ratings follow in their profiles** (`[General]
  Rank`, `ColorLabel`, `InTrash` = rejected; new module `tri::pp3`, one format for both),
  created like the `.rrdata` according to `[sidecars.rawtherapee]` / `[sidecars.art]`; a
  profile holding only ratings is not a development. The neutral flag is the same empty
  circle everywhere (thumbnail, rating bar, filter), sized like the other badges; in the
  rating bar the unchosen accepted / rejected badges keep their colour, faded like the
  colour dots; the "N dev." badge keeps one height.
- **Same keys for burst stacks and development stacks**: `s`, `(` `)`, `Shift+S`, `d)`
  `y)` and the stack mode's arrows act on the burst when stacks are on and it holds
  several photos, else on the photo's developments. New **`((`** opens the next level
  (the folded burst, then the developments) and **`))`** closes the innermost one.
  `Shift+V` and `Ctrl+Shift+V` lose their default keys (`core.dev.toggle`,
  `core.dev.fold_all` stay bindable in `[keys]`). A photo's developments now always stay
  in its burst stack (the similarity cut between them, and the development stack vanished
  as soon as stacks were on). The "N dev." badge and the "fold" badge sit below the colour
  labels instead of over them. The **neutral flag is drawn too** (an empty circle at the
  flag's place, `[gui.grid] neutral_flag`), so every thumbnail shows all its information.
- **Fixed: "Develop with RapidRAW" on a photo whose `.rrdata` holds only a rating** rendered
  an orphan development, outside the stack: such a sidecar is no longer a RapidRAW
  development for the renderer either, and the entry is greyed in the right-click dialog.
- **Generate the missing rating sidecars from the interface** (follow-up to #253): a
  sidecar icon in a thumbnail's right-click dialog (the selection) and "Generate the
  sidecars…" in the open folder's right-click menu; the dialog offers every software or
  one format, can **also rewrite the existing sidecars** (their ratings only, the rest
  kept, an unreadable file recreated; with a warning; `--rewrite` on the command line),
  says how many sidecars will be created or rewritten before doing it, and reports in the
  status bar. Action `core.file.sidecars`, no default key.
- **Right panel: air below each group of settings** (`[gui.metadata] group_gap`, in line
  heights), and the full screen's development buttons in reading order, previous on the
  left (feedback of 2026-10-31).
- **Filter bar: the name search sits at the right**, just before the image count (#241).
- **Help: "Report a problem" in dark red** (#242), and **the help window can be moved**
  (#251; grab cursor on its title bar). The **offload dialog opens at the centre** like the other dialogs (#252).
- **`q q` quits** instead of `q` alone, too immediate next to `Shift+Q` (#244).
- **`tri-gui <image>` opens the image in full screen**, in its folder (#250), in focus mode
  by default (`[gui] image_focus`); **`tri-gui --focus`** forces the focus mode in every
  case, so `tri` can be the system's image viewer.
- **EXIF panel: details in grey** (#249): the 35 mm equivalent focal length and the
  sharpness coefficients on a second line, the megapixels after a dash, without
  parentheses. `Metadata::rows` returns `Row`s (label, value, note); `tri --metadata`
  prints "value – note".
- **Full screen: a "View" section in the right panel** in place of "Grid" (#239): the
  shown source as a drop-down list (camera render, RAW render, each development already
  rendered; on a development, a "before" check box) and the file actions switch. The grid
  section's thumbnail render is a drop-down list too (#246). The bottom bar's development
  buttons now step through the developments (they went to the rating code and moved
  one photo) and are symmetric (#240), and the "N dev." badge is hidden with the thumbnail info,
  where it was truncated (#243).
- **Notes written to every sidecar of the photo** (`tri::sidecars`, a format per
  software): the XMP and RapidRAW's `IMG.RAF.rrdata`, read from the most recent, all
  written. A missing `.rrdata` is created when rating according to
  `[sidecars.rapidraw] create` (`auto`: RapidRAW installed or the folder already has
  some; `always`; `never`), so a note set in `tri` shows in RapidRAW even on an image
  never opened there; `tri <folder> --sidecars rapidraw|xmp|all [--dry-run]` creates
  them on demand for the rated photos. A `.rrdata` without adjustments (a merely rated
  image) is no longer a development: no "1 dev." stack for it.
- **Thumbnail render kept per folder**, like the filters and the sort (`f d`, right panel):
  a folder never set uses `[gui.grid] render`. The render now decides the **cover** of a
  photo with developments: "camera JPEG" and "LibRaw" show the photo itself (the grid no
  longer changes after a few seconds), **"Cover"** (replaces "Latest development") shows
  the preferred development, else the latest, a software shows its main development.
  `render = "latest"` is still accepted in the configuration.
- **Folding development stacks with the mouse**: each thumbnail of an unfolded stack
  carries a "fold" badge (click: fold; Alt+click: fold and keep this development as the
  cover). `Ctrl+Shift+V` folds every development stack. `Shift+Enter` now always makes
  the development the cover (on the photo: no preferred one) instead of toggling.
- **Fixed: ratings reached RapidRAW only when set on its development** (#234). A photo
  and its main developments (darktable, RapidRAW) now share the same notes: rating any
  of them writes the photo's XMP (darktable, the RAW+JPEG pair) and RapidRAW's
  `IMG.RAF.rrdata`. Clones and virtual copies keep their own notes. Undo restores both
  sidecars.
- **Developments counted apart** in the status bar ("12 images (10 RAW, 3 dev.)": a photo
  and its two copies are no longer "3 images"), and **their thumbnails always rendered in
  the background**, each by its software, even with `[gui.grid] develop = "manual"`
  (which now only concerns the photos to develop for the chosen render).
- **Comparing a photo's developments** (`c` on a photo with developments, no selection):
  the camera JPEG, the darktable development, its clones and the RapidRAW copies side by
  side, like a burst; on a development, `b` shows the camera JPEG before / after. A collapsed
  burst still compares its photos, each through the cover of its development stack. Esc
  comes back to the grid on the active development, shown in place of the cover.
- **Exporting RapidRAW developments** (`Ctrl+E`, `tri --export`): a RapidRAW copy or
  main development is rendered by RapidRAW (16-bit TIFF, converted to the preset's space,
  cropped, reduced and sharpened like the others), a darktable clone by darktable,
  whatever is being looked at; a development without its software is exported as its
  photo. Files are named as the software names its own exports (`IMG_01`, `IMG_VC01`).
  Fixed: a darktable clone exported with a crop or sharpening preset was silently skipped.
- **Deleting a development** (Delete / `x` on a development) removes only it: a darktable
  clone, a RapidRAW copy or the main RapidRAW development lose their sidecar, the main
  darktable development has its XMP rewritten without the history (notes kept); the photo
  stays. **Ctrl+Delete** / `dd` deletes the photo with all its developments, even from a
  development; Shift+Delete / `d)` still deletes the whole stack. Several developments
  selected: one ticked checkbox each. Undoable (`u`); a sidecar changed by darktable in
  the meantime is never overwritten. Command line: `tri --delete` does the same.
- **Developments of a photo** (darktable and its clones, RapidRAW and its virtual
  copies): each one is an image of the grid with its own thumbnail (its render), its own
  notes and its own export, stacked under its photo (folded by default, "N dev." badge;
  `Shift+V` unfolds, `Shift+W` / `Shift+Q` step through them, `Shift+Enter` picks the
  cover, `b` in full screen shows the camera JPEG before / after). A photo with
  developments stays the camera JPEG and the LibRaw render; a development's name is the
  photo's, a chip says which one ("DT · clone 1", "RR · copy 2"). Clones used to be loose
  thumbnails and RapidRAW copies were missing.
- **Launching on a slow folder** (NAS): the window opens at once, and the last folder is
  read in the background with its progress; it waited several seconds for the folder's
  list, with a generic icon in the Dock.
- **Filters in full screen**: the filter bar (Shift+F) and the whole right panel
  (filters, grid) are there in full screen too, to rate in passes without going back to
  the grid.
- **Filters and sort kept per folder**, from one launch to the next
  (`[gui] remember_folder_view`, on by default).
- **RapidRAW virtual copies** (`IMG.RAF.<id>.rrdata`): they now follow the copied,
  moved or renamed image; they were left behind.
- **Default darktable render**: an image imported into darktable but not edited (a
  virgin clone, for example) now has its darktable render, which is not LibRaw's; a
  non-empty history was required until now.
- **One thumbnail per darktable clone**: a clone (`IMG_01.RAF.xmp`) appears next to its
  image, with its own ratings and its own development. Deleting it deletes only its
  sidecar; copying, moving or renaming it takes the image and all its clones along.
  RapidRAW's sidecar (`.rrdata`) follows the image too.
- **File actions forbidden by default**: renaming, copying, moving and deleting images
  and folders from tri only happens once allowed ("File actions" checkbox in the right
  panel, `[files] allow_actions`); otherwise these actions are greyed out and say why.
  When allowed, the dialogs remind that darktable, Lightroom or Capture One will no longer
  find moved or deleted files.
- **Network neighbourhood**: the servers announced on the local network (Bonjour / mDNS:
  SMB, AFP, SFTP, WebDAV, FTP, NFS) appear under "Network" in the folders panel; a click
  lists their shares, as when typing their address (`[neighbourhood] enabled`).
- **Developments to redo announced**: when photos developed elsewhere (darktable) have
  changed, a banner at the top says how many renders are redone, with a progress bar,
  then that they are done; it fades by itself (`[gui.grid] develop_toast_ms`).
- **Network shares on Linux (GVFS)**: listing a folder no longer asks the type of each
  file over the network (1 to 2 images per second on a share opened by Nemo); the folder
  watch spaces its reads out when they are slow (`[watch] max_share`, 10 % of the time at
  most) instead of keeping the network busy.
- **Clones and settings of other software follow the image**: moving, copying, renaming
  or deleting an image also takes its darktable clones (`IMG_01.RAF.xmp`…) and the
  settings of RawTherapee (`.pp3`), ART (`.arp`) and Capture One
  (`CaptureOne/Settings…/IMG.RAF.cos`, kept in its subfolder). They used to stay behind.
- **Slow folders (NAS, slow disk)**: a folder whose list takes time opens in the
  background, with a box showing how many images were found so far and a Cancel button
  (or Escape); the previous folder stays on screen meanwhile. Quitting no longer waits for
  the thumbnails in progress. `TRI_GUI_TRACE=1` now also traces slow frames of the
  interface, step by step (`[gui] trace_slow_ms`). The grid no longer reads the size and
  date of every source file again on each frame (up to 37 s frozen on a NAS): they are
  read once, and forgotten when the folder watch sees a file change; while the grid is
  drawn, a missing one is read in the background instead of holding the page, as are the
  sidecars it needs (darktable development, version badges), the most recent request
  first (scrolling fast, the visible thumbnails come before those already passed). A
  sidecar is looked up by its exact name before the whole folder is listed (full screen,
  from one image to the next). The engine checks the thumbnails on screen first, then the
  others by distance (dragging the scroll bar no longer waits for every row above). In
  full screen, the camera JPEG histogram is computed in the background.
- **Network login box**: a server asking for an account that the system does not ask
  for itself (Linux, Windows; macOS keeps its own dialog) opens a box: user, domain
  (SMB), password, "Remember the password" (Windows Credential Manager, desktop keyring on
  Linux). A refused account brings the box back with an explanation; other failures say
  why (server unreachable, share unknown, already connected under another account). FTP
  copies use it too; SFTP stays with SSH keys.
- **Network servers, other protocols**: besides SMB, `afp://`, `ftp://`, `sftp://`,
  `dav://` / `davs://` (WebDAV) and `nfs://` addresses, mounted by the system where it
  can (all on Linux, all but SFTP on macOS, SMB and WebDAV on Windows). Elsewhere, FTP
  and SFTP are copied locally (`curl`, OpenSSH `sftp`: no new dependency), after a box
  announcing the number of images and the size, with the destination to choose
  (`[remote]`).
- **Network shares**: connect to a share by its address, `\\nas\photo` or
  `smb://nas/photo` (the "server" icon of the folders panel, `c`, or the address typed in
  the folder search). The system mounts and asks for the password; the share is then
  kept in the "Network" section (right click or Shift+C to forget it). On Windows,
  network drives (`Z:`) go under "Network". A server alone (`smb://nas.local/`) lists
  its shares, to pick one; a share already mounted over AFP (Nemo) is recognised.
- **Folders panel**: "Subfolders" becomes an icon (a folder tree), grey, in the accent
  colour when the option is on (like the filters), right after the title; Refresh moves
  to the right edge of the panel, and the "Folders" title takes the size of the sections
  below.
- **Fixes from the review of 2026-10-28**:
  - rating an image developed in Darktable no longer redoes its render: only the
    sidecar's development counts, no longer its date (`tri` also writes the ratings
    there); renders already cached are redone once;
  - the LibRaw export follows the `[render]` settings, like the display;
  - the `[gui.colors] error` colour applies to every error message;
  - new settings `[gui.dialogs] poll_ms`, `max_errors`, `completions` (values that were
    hard-coded);
  - the tests type shortcuts as the real window delivers them (egui-winit's cut / copy /
    paste, #194), for every shortcut of every profile.
- **Download the latest version**: at the top of the help (F1), two buttons, the
  nightly build and the latest alpha, with a tooltip explaining the difference. This
  system's package downloads in the browser; when there are several (Linux: AppImage,
  .deb, archive; Windows: .msi, .zip), a dialog lets you choose, and the choice is
  remembered (`[update] formats`).
- **Open a folder in Darktable**: right click on a folder, "Open in Darktable", or `d`
  in the folder panel. Darktable opens on that folder (imported if needed), with its own
  library; an instance already open receives it on Linux and macOS; on Windows, the entry
  is greyed while Darktable is open. RapidRAW can only open an
  image, not a folder.
- **Luminance in the large histogram**: one more dot (a half-light disc) and `l` show
  the luminance alone, with its row in the table. On a RAW, it is computed on
  the sensor data, per block of the mosaic (2 × 2 in Bayer, 3 × 3 in X-Trans), with the
  shot's white balance; green no longer stands in for it. Colour `[gui.colors]
  hist_luminance`. Cached exposure analyses are recomputed once.
- **Camera JPEG in the histogram**: for a RAW, the camera JPEG's curve is drawn as a
  thin grey line over the sensor data's, to see what the camera did with the highlights
  and shadows. In the large histogram, a "Camera JPEG" row under Red, Green, Blue is its
  legend, with its figures (median, mean, range, spread, saturated, blocked); a button
  and `j` show or hide it, and that is remembered (`[gui.histogram] camera_jpeg`,
  `camera_width`, `[gui.colors] hist_camera`).
- **Right panel in sections**: the filter bar keeps the one-click filters and the name
  search; their settings and the grid's move to the right panel, in sections titled like
  the folder panel's and folded with a click: Histogram, Filters (each filter's settings
  only while it is on; two sliders for exposure; development software as check boxes),
  Grid (sort, thumbnail render, stacks), EXIF at the bottom; the RAW and JPEG boxes open
  the Filters section.
  The folder panel's sections fold too; folded sections are remembered. Section titles
  in capitals, larger, in the accent colour, with a gap between sections (`[gui.folders]
  section_scale`, `section_gap`). After a filter change, the number of images shows a
  moment in the middle of the screen (`[gui.grid] filter_toast_ms`). **From the
  keyboard** (leader then `l`): ↑ ↓ go through the settings too, Space uses them, ← →
  vary them, Tab / Shift+Tab (`)` / `(` in vim) go from section to section, in the
  folder panel too. The Grid section starts with the sort; the stack entering choice
  stays only in the bottom bar. "RATING", "FILTER" and "FOLDERS" labels on the bottom
  and top bars and the folder panel, in small capitals in the accent colour (`[gui.grid]
  bar_label_scale`). `[gui.grid] stack_slider` and `clip_slider` are gone: remove them
  from your configuration file if you set them. In a narrow window, the end of the
  filter and rating bars no longer slides under the right panel.
- **Developments in the grid**: after developing, the grid can show each RAW's
  development instead of the camera JPEG: `[gui.grid] render` = `embedded` (default),
  `latest`, `darktable`, `rapidraw` or `libraw`, also a list in the filter bar and `f d`,
  kept between sessions. A RAW without the wanted development shows its LibRaw render,
  computed in the background; missing developments are rendered in the background
  (`[gui.grid] develop = "auto"`) or on request (`"manual"`). A chip under the thumbnail
  names the software (issue #114).
- **"Developed" filter**: a film reel in the filter bar (`f Shift+D`), then the installed
  software to check: the images developed in one of them, shown in the most recent one,
  whatever the grid's render; none checked, the images without development, in their
  LibRaw render. Invertible; the checked software are remembered. Full screen opens on
  what the thumbnail shows.
- **RapidRAW** as a second development software, next to darktable: `rapidraw export`
  applies its `.rrdata` sidecar; found in the PATH, the usual places, as an AppImage or a
  Flatpak (`[develop] rapidraw`). `[develop] default` = `"auto"` (latest development) or a
  software. With RapidRAW's interface open, its export gave an all-black image: it is now
  refused with a plain message (issue #114).
- **Choosing the software**: `<leader> r` (the image's software), `<leader> d`
  darktable, `<leader> p` RapidRAW; "Develop with" in the right-click dialog; a dialog to
  develop a whole folder (each image in its own software, or all in one). In full screen,
  `b` goes through the camera JPEG, the RAW render, then each development already
  rendered, to compare them. Develop errors name their software. **Changed action ids**:
  `core.develop.render`, `core.develop.folder` (and new `plugin.rapidraw.develop`,
  `core.grid.render`, `core.filter.developed`): a `[keys]` section using the old ids must
  be updated (issue #114).
- **"Darktable"** with a capital D wherever the name is shown; identifiers, configuration
  values and `darktable-cli` stay lowercase.
- **Star filter**: at least, exactly or at most (`f =` or the ≥ / = / ≤ button before the
  stars) (issue #165).
- **Removable media** (Linux): plugged in but not mounted, they show greyed under a new
  "Devices" section, mounted and opened with a click; arriving media are mounted by
  themselves (`[places] auto_mount`, on by default); fixed drives get their own "Drives"
  section (issue #113).
- **Folder panel**: section titles larger than the folders, in bold (`[gui.folders]
  section_scale`; issue #120).
- **Panels**: a line at the top of the panel that has the keyboard (`[gui.dialogs]
  focus_line`, `focus_line_width`; issue #77).
- **Help and settings**: sequences starting with the panel prefix read `<leader> r`, the
  leader being shown on its own (issue #175).
- **Stacks**: the stack entry button is also in the grid bar, and its click works;
  entering a folded stack from the grid honours it too (issue #62). Under `f c`, a stack
  no longer splits in two (issue #163). With stacks on, `c` on a single photo compares
  it (issue #167).
- **Keys**: a held key repeats only moves (navigation, panning, zoom…), not one-shot
  actions: holding `e` opened the editor over and over (issue #160).
- **Windows**: external programs (eject, editor) no longer open a console window (issue
  #159).
- **Folders**: copying or moving a folder no longer takes its own `.tri-cache`; a click
  clears a status bar message (issue #157).
- **Background tasks**: folding back restores the panel's height (issue #158).
- **Window under Wayland**: maximized again at launch when it was (issue #58).
- **Narrow window**: in the "…" menu of the filter bar, the lists open as submenus (they
  closed with the menu; issue #181).
- **Linux**: `install.sh --uninstall` updates the icon cache; the app icon disappeared
  after moving from the archive to the `.deb`.
- **Several cards in a row**: the next card is also picked up after closing the
  offload summary or offloading in the background, and when it is mounted at the
  previous card's path (two cards of the same camera) (issue #37).
- **Background tasks**: folded, a thin discreet grey bar (`[gui] tasks_folded_height`,
  text on hover); it stays in place once unfolded, so a second click folds the details
  again (issue #36).
- **Panels**: the line under the keyboard cursor is a discreet grey band across the
  whole width (`[gui.dialogs] cursor_tint`) instead of an accent frame, which was cut
  on the left in the folder panel (issue #77).
- **Stacks**: `f c` again folds back a stack it unfolded (issue #52); the "|‹" button
  (enter a stack by its cover or its first photo) is always in the full screen bar, and
  Ctrl+Shift+S says which one is on (issue #62).
- **Grid**: a refilter (filter slider, ratings, analyses coming in) keeps the region
  shown when the selection is out of view, instead of going back to the start (issue
  #100).
- **Offload**: the progress bar follows the files when some are skipped (already
  offloaded or already there) (issue #101).
- **Drive pulled out** without ejecting during the computations: plugged back, the
  open folder is read again and the computations resume (issue #98).
- **Window under Wayland**: its size is remembered (the position stays with the
  compositor) (issue #58).
- **Deleted folder**: a deleted subfolder (with "Subfolders" on) leaves the grid and
  its computations stop; a darktable batch on a deleted folder is interrupted.
- **Comparing a stack**: `c` with no selection compares the photos of the current
  stack side by side (a collapsed stack too); `f c` shows only the current stack,
  expanded, in the grid (issue #52).
- **Several cards in a row**: with the offload dialog left open on its summary,
  plugging in the next card starts it again on that card, towards the same session
  folder (issue #37).
- **Background tasks**: one global bar (number of tasks, mean progress) at the bottom
  of the metadata panel, or in the status bar when it is hidden, instead of a row of
  bars; a click unfolds one bar per task above it (issue #36).
- **A whole folder in darktable**: Ctrl+Shift+B, or the right click on a folder of the
  panel, renders in the background every image with a darktable sidecar, one after the
  other, into the cache (progress in the background tasks, "×" to interrupt, summary
  at the end) (issue #69).
- **Grid**: no clipping badge on a collapsed stack, where its image count is (issue
  #75).
- **Panels**: the panel that has the keyboard has a slightly lighter background
  (darker in the light theme) instead of an accent outline; `[gui.dialogs]
  focus_panel_tint` replaces `focus_frame_width`. The metadata cursor no longer frames
  the histogram (issues #77, #78).
- **"Well exposed" filter**: a single slider, with a switch showing the clipping badge
  icon to choose between blown highlights and crushed shadows, instead of two sliders
  with hard-to-read grey labels.
- **Sharpness and exposure filters during the analyses**: the grid fills in as they
  are computed (at most every `[gui] analysis_refilter_ms`, 5 s) instead of only at
  the end, and the status bar says how many photos are analysed when the filter is
  turned on; a refilter no longer drops the analysis in progress (issue #73).
- **Copy / move to…**: "Continue in the background" closes the dialog during the copy;
  its progress is in the status bar (a click reopens it) and the end is announced
  there (issue #72).
- **Narrow window**: the end of the filter bar no longer covers the search field (sort
  and image count go into a "…" menu, invert and clear stay visible), and the rating
  bar's right-hand buttons wrap to a second line instead of covering the colours
  (issue #71).
- **Last folder gone**: launched while the drive of the last folder is unplugged, the
  window says which folder and why, large and centred, and opens it again by itself
  when the drive is back, unless another folder was opened meanwhile (issue #66).
- **Window**: its size, position and maximized state are restored at the next launch
  (issue #58; position not available on Wayland).
- **Stacks**: start and end of the stack with `(` / `)` (vim) or Shift+Home /
  Shift+End (classic); in the vim profile, the whole stack is now `d)` / `y)` (instead
  of `dw` / `yw`: a stack is a sentence, as in vim; on AZERTY, Shift+5 still gives 5
  stars). In stack mode, a "|‹" button next to the arrows (Ctrl+Shift+S, `[gui.view]
  stack_enter`) chooses whether ↑ / ↓ land on the stack's cover or on its first photo
  (issue #62). The cache location gets a drop-down list in the settings window.
- **Nightly builds**: their packages are named after the day and the commit
  (`tri-1.0.0-alpha.7-nightly-20261025-d16c32f-…`), no longer like the published
  version.
- **Stacks slider**: towards the right (stricter), it follows the rank of the folder's
  distances, so each step joins about as many photos and the far right leaves no stack
  at all; it was pixel-sensitive there and stopped with stacks left (issue #61).
- **Linux packages**: a `.deb` (Debian, Ubuntu, Mint) and an AppImage (to try it
  without installing anything), next to the `.tar.gz` archive.
- **Offload**: the previews and analyses of each photo are computed as soon as it is
  copied, while the copy goes on; the session is ready sooner. Checkbox in the offload
  dialog, `[offload] previews` (on by default); `--previews` on the command line
  (issue #40).
- **Cache on a USB stick**: the switch to winter time no longer computes the whole cache
  of a FAT32 drive again (dates in local time, shifted by one hour); reading the cache
  no longer writes to the drive; turning on the "well exposed" filter during the analyses
  no longer freezes the window (issue #74). The cache is computed again once.
- **Auto advance, stack mode**: option `[gui.view] stack_advance_next` (off by default)
  to move, at the end of a stack, to the first photo of the next stack.
- **Trash**: deleting many images no longer freezes the window; a progress bar shows
  in the dialog (on Linux, the whole trash was also read again for each file; issue #43).
- **Dialogs**: the same close cross at the top right of every dialog, instead of a "Close"
  button at the bottom (issue #39).
- **Offload, RAW only**: the progress counts only the RAW files, one by one (it
  counted the JPEGs left on the card too and moved forward two by two; issue #38).
- **Grid**: Ctrl / Cmd + mouse wheel, or a pinch on the touchpad, changes the size of the
  thumbnails (issue #44).
- **"Well exposed" filter**: two sliders, "blown" and "crushed", next to the sun, like the
  blur one; fully to the left, that side no longer excludes anything (photos on a black
  background, highlights blown on purpose). Remembered per folder; the badges follow
  (`[gui.histogram] strictness_span`; issue #48).
- **Folder panel**: on hover, the "offload" icon of a volume turns to the accent colour and
  "eject" to red, so they are not confused (issue #41).
- **Auto advance**: the star of the button stays visible when it is on (it had the accent
  colour of the background; issue #59).
- **Auto advance in stacks**: in full screen in stack mode, a rating moves to the next
  photo of the stack, like →, instead of the next stack (issue #60).
- **External programs** (editor, default application, file manager): their messages no
  longer fill the terminal `tri` was started from.
- **Cache with the photos**: the cache can be kept at the root of a removable drive (a
  transfer drive, found again on another computer) or in a folder (it follows the folder
  when it is moved): right click in the folder panel, `tri <folder> --cache-here` /
  `--cache-user`, or by default with `[cache] location = "disk"` / `"folder"`. Moving or
  renaming a folder from `tri` brings its cache along. The existing cache is computed
  again once (new format; issue #55).

## 1.0.0-alpha.7 — 2026-10-23

Seventh test version: large folders open at once, cache of one folder from the command line, XMP and settings fixes.

- **Cache of one folder**: `tri <folder> --clear-cache [level] [--depth N]` clears only
  the cache of that folder's images (and of its subfolders down to `--depth`), so they are
  computed again; all levels accepted (`embedded-full`, `developed` too). The cache is now
  organised by folder of photos: the existing cache is computed again once, the old one
  is deleted in the background.
- **Settings**: during a search, a click on a section in the list opens it as without
  search (export presets were all shown one after the other; issue #24).
- **Versions**: "Original" is reserved for the uncropped image in the versions list; a
  version at the image's ratio is first named "Crop".
- **Large folders on a slow disk or NAS**: the grid shows at once, the ratings are read in
  the background with their progress in the status bar (2,820 images on an external hard
  drive: 15 s of empty window before); setting a rating no longer rereads every sidecar
  of the folder; the engine's results no longer freeze the first frames (issues #15,
  #29).
- **Order**: the remembered order is applied at startup too (issue #30).
- **XMP**: sidecars with a nested `rdf:Description` (Lightroom, photographer's contact
  details): crop versions are written again (they were reported as saved but not
  written), and a rating field after the nested block is read and replaced (issue #27).
- **`tri <folder> --previews`**: waits for the background analyses (sharpness,
  similarity) before ending; some were not cached, and the final cache table missed them.

## 1.0.0-alpha.6 — 2026-10-22

Sixth test version: names of versions and presets, faster settings search.

- **Versions**: no two versions of an image with the same name (case aside); the
  proposed name is numbered, a taken name turns red and cannot be saved (issue #14).
- **Settings**: export and crop presets are renamed with a "Rename…" button instead of
  an editable name field, and two presets cannot share a name (issues #17, #18).
- **Settings**: scrolling a search ("export") is smooth again: the translation
  catalogues were re-read for every setting on every frame (issue #16).

## 1.0.0-alpha.5 — 2026-10-22

Fifth test version: the testers' issues, image order, named favourites.

- **Order**: by name, shooting date or modification date, ascending or descending (list
  and arrow in the filter bar, `f s` / `f r`, remembered); RAW+JPEG pairs stay together.
- **Favourites**: a favourite can be named (`n` or right click; path on hover), and
  unfolds without unfolding the same folder in the tree, and vice versa.
- **Full screen**: number, source and size in the status bar instead of on the image;
  in focus mode, on the image, hidden by `Ctrl+I`.
- **Background colour**: `Ctrl+L` turns the grid and full-screen background through the
  theme's, dark grey, black and white (`[gui.colors] backgrounds`), without duplicates.
- **Panels**: file name as a metadata row; a click in a panel gives it the keyboard, a
  click on the images gives it back to them; thinner outline (`[gui.dialogs] focus_frame_width`).
- **Ratings**: the tooltips of the stars, colours and flags give their shortcuts.
- **Help**: opens at its final size at once (it grew over several frames the first
  time), and the keys column no longer overflows the window (issue #11). Smooth opening:
  prepared at startup, only the visible lines drawn (75 ms per frame down to 0.3 ms).
- **Full screen**: blown, blocked and blurry badges in the rating bar, as in the grid
  (click: large histogram, sharpness map; issue #9).
- **Full screen**: ratings no longer shown twice; on the image only when the rating bar
  is hidden (issue #8).
- **"Well exposed" filter**: the grid could no longer be scrolled (refiltered endlessly,
  each time brought back to the selection; issue #10).
- **Full screen**: an image's versions in a list on the ratings line; the image no longer
  changes size from one photo to the next (issue #5).
- **Update dialog**: release notes formatted (bold, lists, code) instead of raw Markdown
  (issue #3).
- **Help**: version more visible and copyable (with the system), and a "Report a
  problem" button opening the GitHub form prefilled with them (issue #7), also `Ctrl+B b`
  (leader, then b).
- **Settings**: a key set in your configuration file is in bold, now said in the
  window and on hover (issue #6).
- **Release notes in the interface language**: the "Update" dialog shows the notes in
  English or French; changelog in English (`CHANGELOG.md`) and French
  (`CHANGELOG.fr.md`), both in the Release.
- **Crop**: a handle approached from inside is grabbed as soon as the cursor changes (it
  moved the whole frame; issue #1).

## 1.0.0-alpha.4 — 2026-10-22

Fourth test version, before beta 1.

- **Check for updates**: button at the top of the help (F1) and `tri --check-update`, on
  demand only (Releases page, read through `curl`); the dialog gives the new version's
  notes and opens its page. A nightly or personal build says so, and counts the commits
  of `main` it lacks ("git pull, then rebuild"); version number with the commit in the
  help.
- **Search in the help and the settings**: topics and sections stay visible (sections
  found first, with their number of results), a section's title finds it whole, hidden
  keywords ("vim" finds the shortcut set), and words found as typed set aside matches
  with scattered letters.
- **Nightly builds**: the CI builds and tests every night `main` changed, packages on the
  "nightly" Release.
- **Filters**: "invert" icon the size of the colour dots.
- **Mouse ratings**: clicking the set star again clears it, like the stars filter; the
  "⊘" button is gone (on the keyboard, the 0 key; `1` twice stays at 1).

## 1.0.0-alpha.3 — 2026-10-21

Third test version, the first with **Windows** (`.msi` and portable `.zip`), next to the
macOS (`.dmg`) and Linux (`.tar.gz`) packages, still without HEIC.

- **Stacks**: severity slider like the blur one; in the centre, the threshold computed
  from the folder (cut between series and scene changes), to the right stricter (small
  stacks); remembered per folder. `tri --groups` groups at the computed centre.
- **Sliders**: double-click = default value, everywhere. Blur: all the way left, all
  images; in full screen, the grid is no longer refiltered before going back (the image
  being looked at could change).
- **Colours**: stars, chosen colour and [RAW] / [JPG] switches in the theme's accent
  colour (no more yellow and big square), thumbnails' RAW badge too; "without blurred"
  filter: a triangle instead of the crossed-out drop.
- **Status bar**: no more flashing progress bars while adjusting a setting
  (`[gui] progress_delay_ms`).
- "Camera JPEG" becomes "Camera render".
- **Full screen**: resolution of the darktable development, its crop read from the
  sidecar.
- **Filters**: "well exposed" (sun, `f w`: without blown or blocked ones) and "invert"
  (◐, `f i`: what the filters remove; "blurred only" becomes inverted "sharp", `f B`
  removed).
- **RAW+JPEG pairs**: "with the other file of the pair" checkbox in the Delete and Copy /
  move dialogs (deleting all the JPEGs took the RAWs along).
- **Crop**: presets on `Tab` / `Shift+Tab` (`p` stays the sharpness map).
- **Windows**: `.msi` installer (Start menu, uninstall from Settings) and portable zip,
  with LibRaw and the Visual C++ runtime; no more console window behind the interface.
- **Settings**: English descriptions in the settings window
  (`doc/config.example.en.toml`), following the interface language.
- **Grid**: adjustable thumbnail size (slider at the bottom, `+` / `-`, `*` or "fit all"
  button), hideable thumbnail info (`Shift+T`); name without extension and format (RAW,
  JPEG, TIFF…) on the right; blurred photo: drop outline; ratings without a dark
  rectangle (stars bottom left, flag bottom right, colours top right); clipping as a
  half-white half-black triangle top left (click: large histogram), versions in the
  caption (click: crop); clear the filters: crossed-out funnel at the far right of the
  filter bar.

## 1.0.0-alpha.2 — 2026-10-21

Second test version, the first published with its packages on the Releases page (public
repository). macOS (`.dmg`) and Linux (`.tar.gz`) packages, without HEIC; Windows to
come.

- **Folders**: copy or move a whole folder (`Ctrl+C` or `y` / `Ctrl+X` in the folders
  panel, right click, undoable); `y` copied the selected images instead of the folder.
  **Drag and drop** of a folder or of grid images onto a folder, always confirmed
  (Option on macOS, Ctrl elsewhere: copy). Panel in titled sections: removable drives,
  network (mounted shares, GVFS on Linux), personal folders (Desktop, Documents,
  Downloads, Pictures, like the Finder), favourites, then the system. The "Add to
  favourites" right click opened the delete dialog: fixed.
- **Blur**: severity slider in the filter bar (settings in the centre, stricter to the
  right, more lenient to the left), live, remembered per folder; the metadata panel also
  gives the ratio to the stack.
- **Full screen**: resolution of what is shown (a version: its frame, reduced by its
  export preset).
- **Settings**: a crop preset's export preset is chosen from a list (no more typos).
- **Sharpness**: the map (`p`) also shows in the fitted view and follows the severity
  slider (more lenient: more sharp edges); in full screen, the slider in the strip and
  `Ctrl+J` / `Ctrl+K`.
- **Full screen**: right resolution while cropping and for the camera JPEG.
- **Documentation** in two languages (`doc/en/`, `doc/fr/`), `README.md` in English.
- **Filter bar**: two switches, labelled RAW and JPG, instead of "All / RAW / Flat"; blur
  slider shown only with a sharpness filter, as wide as the stacks one.
- **Subfolders**: checkbox in the folders panel, their images in the grid too (3 levels
  by default), choice kept.
- **Copy / move to…**: title without a verb and a "Confirm" button; recent destinations
  without duplicates.
- **Full screen**: after a darktable development (Shift+B), the other images no longer
  zoomed ("not developed yet"); an image without development now shows in the previous
  source, with its 1:1.
- **Stacks**: the similarity slider no longer unfolds the stacks while adjusting (the
  resulting stacks are visible); hold Shift to see them unfolded.
- **Export report** (`Ctrl+Shift+E`, or the button next to an export's summary): the
  finished exports, kept from one session to the next, with a link to each destination
  folder; can be cleared.
- **Vocabulary**: "export preset", like "crop preset" ("profile" stays for colour
  profiles).
- **Settings**: a single entry for export presets and one for crop presets, with a list,
  "New…" and "Delete".
- **Crop**: "Save as preset" becomes a small floppy disk next to the presets list; Save
  and Cancel on the right.
- **Export dialog**: the arrows only change the preset on its list; Tab moves on to the
  folder.

## 1.0.0-alpha.1 — 2026-10-19

First test version (private repository; the beta will be public). Ready-to-install
packages for macOS (`.dmg`) and Linux (`.tar.gz`), without HEIC; Windows to come.

- **Folders**: folders panel (volumes, favourites, `/` search), watching of the open
  folder, card offload (verified copy, duplicates, eject).
- **Grid**: thumbnails in the background with a persistent cache, filters (type, stars,
  colours, flag, sharpness, name), multiple selection, stacks of similar images (live
  adjustable threshold), side-by-side comparison.
- **Full screen**: camera JPEG or real RAW render (LibRaw), continuous zoom up to 400 %,
  loupe, focus point (Fujifilm), sharpness map, blown / blocked zones, overlays (grids,
  golden spiral, text, logo, 4:5 format…).
- **Ratings**: stars, colours, accepted / rejected, in XMP (darktable and Adobe
  conventions, nothing else touched), undoable.
- **Analyses**: RAW histogram in EV, sharpness score judged within the folder and the
  stack, grouping by similarity.
- **Files**: rename, trash, copy / move to…, external editor, AirDrop (macOS).
- **Developed in darktable**: `Shift+B` (or the button under the image) runs the
  darktable-cli render, cached, nothing written on your side; `b` then shows it.
- **Export**: `Ctrl+E`, JPEG / TIFF / PNG by export profiles ("Retouch" 16-bit ProPhoto
  TIFF, "Web" sRGB JPEG), developed by darktable (installed, AppImage or Flatpak);
  without darktable, a basic render by LibRaw; no ProPhoto in 8 bits.
- **Interface**: everything from the keyboard ("classic" and "vim" sets, adjustable
  shortcuts), settings window, help with search, French and English, dark, light and
  neutral grey themes; icon, top bar, focus mode (F11).
