# Graphical interface: `tri-gui`

egui/eframe application, in its own package (`gui/`) of the Cargo workspace. The engine
(`tri`) does not depend on egui: the engine / display separation of `CLAUDE.md` is thus
guaranteed by Cargo itself.

```sh
cargo run --release -p tri-gui -- <dossier> [--cache <dossier du cache>] [--config <fichier>]
```

All settings (theme, shortcuts, grid dimensions, delays, capacities, colours, and the
engine's) come from the configuration file: see [settings.md](settings.md)
and [config.example.toml](../config.example.toml). An invalid configuration is reported in
red in the status bar (details on hover), and the default settings are used.

Always use **`--release`** for real use: in a debug build, the Rust code is not
optimized, and the 1:1 of the camera render goes from 0.4 s to 2 s (rotation, conversion for
the GPU), the 100 % RAW from 4.5 to 7 s. `--cache` uses another cache folder than the
system one ("cold" measurements, experiments).

## What is in place

- **Thumbnail grid** of the folder, fed in the background by the
  [scheduler](scheduler.md); only the visible rows are drawn
  (`ScrollArea::show_rows`), which handles thousands of images.
- **Panel icons** (2026-10-17) in the status bar: on the left, folders panel, filter bar,
  rating bar; on the right, metadata panel.
  Each icon draws a window and the panel's strip, filled when it is shown; the
  shortcut is shown on hover. Same actions as the keyboard.
- **Status bar**: progress in **discreet grey** (`[gui.colors] progress`, the theme's grey
  by default) rather than in the accent colour, which drew the eye to background
  information (feedback of 2026-10-17); folder, number of images (including RAW), filter,
  progress of thumbnails, of "Camera render" previews (hidden if there is no RAW) and of
  standard previews, name of the selected image. **Ratings** first while they are read in
  the background (issue #15, 2026-10-23): a folder opens with the list of its images
  (`Folder::scan`), the grid shows at once, the ratings follow (`folder::Loading`,
  `TriApp::poll_loading`; a filter on ratings applies once they are read). The engine's
  results are taken within one frame's budget (`[gui] load_budget_ms`, like thumbnails;
  thousands arrive at once on an already computed folder), and those already known
  (announced again after each refilter) are not reread. The remembered order is applied
  at startup (issue #30).
- **Selection** with the keyboard or the mouse; the scheduler is notified (`set_focus`), so
  the preview of the selected image and of its neighbours moves to the head of the queue.
- **Selection on click, framed hover** (2026-10-17, second version): the selection
  (solid turquoise background) only changes on click or with the keyboard; the thumbnail
  hovered by the mouse is only framed in light grey (`[gui.colors] hover`, `[gui.grid]
  hover_frame`). History: at first, hover (grey background) and selection (turquoise)
  looked so alike that they suggested two selected images; we first tried "hover
  selects" (a single cursor), but moving to the rating bar across other thumbnails then
  changed the rated image. Clearly distinct styles (frame versus solid background) solve
  the first problem without creating the second. The pointer, and the frame with it, is
  **hidden as soon as you type** and reappears at the first mouse movement, in the grid
  as in full screen.
- **Folders panel** (2026-10-17), to the left of the grid: places expandable as a tree,
  arranged in titled sections separated by a line (2026-10-20): removable drives,
  network, personal folders (with Desktop, Documents, Downloads, Pictures), favourites,
  then only the system (root, collapsed); **Subfolders** checkbox (2026-10-20,
  remembered): images of subfolders are shown too (`[gui.folders] subfolders_depth`
  levels), read on demand by the engine (`places::Tree`, see [places.md](places.md)). The
  triangle expands or collapses (double-clicking the name too), a click on the name opens
  the folder without restarting (`folder::Folder`, see [folder.md](folder.md)): the
  filters are kept, the selection starts again from the first image. A plugged-in drive
  or card appears by itself within a few seconds (engine monitoring, `places::Monitor`);
  "Refresh" or `F5` rereads volumes and folders right away. Each volume has an "eject"
  icon: confirmation (modal), the open folder is closed if it is on that volume
  (otherwise the system refuses), then ejection in the background (`places::eject`); the
  result, or the system's message, is shown in the status bar. A **fixed** volume
  (`[places] fixed`, e.g. the always-connected work drive) has no eject icon but a drive
  icon, not clickable: no accidental ejection (user's idea, 2026-10-17). On hover, the
  "offload" icon turns to the accent and "eject" to red (`[gui.colors] error`): two
  small neighbouring icons not to be confused (2026-10-24, issue #41). The open folder
  is expanded and shown at startup. `Shift+D` shows or hides the panel; settings
  `[gui.folders]` (shown, width, indent, hidden folders). Without a folder argument,
  `tri-gui` reopens the last opened folder (`[gui] reopen_last`, remembered by the engine
  in the data folder), otherwise opens on the panel, with the invitation to pick a folder
  large and grey in the centre. Last folder gone (drive unplugged, issue #66): its name
  and path above, and it opens again by itself when the drive is back (volume watching),
  unless another folder was opened meanwhile (that would go back without being asked).
  Drive of the **open** folder pulled out without ejecting during the computations, then
  plugged back (issue #98): the folder is read again and the computations resume
  (`TriApp::set_volumes`). The filter bar wraps when space runs
  out.
- **Rating** (2026-10-17): stars, colours, picked / rejected on the selected image
  (grid or full screen), written immediately to its XMP sidecar (see [xmp.md](xmp.md),
  [notes.md](notes.md)). Ratings loaded when the folder is opened, with those already set
  by darktable, Lightroom or digiKam. Display drawn as shapes (no glyphs, see
  `marks.rs`): flag (green tick / red cross), golden stars, colour dots, at the bottom
  left of the thumbnail and of the full screen view; thumbnail of a rejected photo faded
  (`[gui.grid] rejected_opacity`). A failed write (malformed sidecar, read-only disk) is
  reported in the status bar, and the displayed rating stays the one on disk. A single
  colour per photo by default (`[xmp] multiple_colors`).
- **Filters** (2026-10-17): type (all / RAW / flat) and ratings (at least n stars;
  picked, without rejected or rejected; one or more colours), combined.
  - Keyboard: prefix `f` (user's choice), followed by the matching rating key:
    `f f` next type, `f 1`…`f 5` at least n stars, `f 0` any stars, `f p` picked,
    `f u` without rejected, `f x` rejected (the same sequence removes the filter),
    `f 6`…`f 9`, `f °`, `f ,`, `f .` colours (add / remove), `f Esc` clear everything.
    Digits and letters with or without Shift.
  - Mouse: **filter bar** at the top of the grid (`Shift+F` shows or hides it,
    `[gui] filter_bar`): each click triggers the same action as the keyboard (clicking the
    active star again clears the star filter). More complex filters (window or side
    panel) are still to be discussed.
  - Status bar: active filters in plain words ("RAW · 3 stars and more · without
    rejected").
  - The selection is kept if the image stays visible; otherwise it keeps its place in the
    list. Rating an image under a rating filter refilters at once: rejecting a photo with
    "without rejected" makes it disappear and selects the next one (fast culling).
- **Sharpness** (2026-10-17, engine: `tri::sharpness`, see `sharpness.md`): in full
  screen (fitted or 1:1 since 2026-10-21), `p` highlights sharp edges (focus peaking),
  `Ctrl+P` changes colour (3 configurable colours, `[gui.sharpness] colors`); threshold
  tied to the strictness slider (`Ctrl+J` / `Ctrl+K`, slider in the bottom bar). Computed
  by the loader, tinted when drawn.
- **Blurry photos** (2026-10-17, engine: score from `tri::sharpness`, see `sharpness.md`):
  drop (outline) under the thumbnail (the word "blurry" until 2026-10-21), "Sharpness" row in the metadata panel, filters
  "without blurry" (`f b`), inverted for blurry only (`f i`); judgement relative to the folder's
  median. Finished progress bars are hidden (`[gui] progress_hide_done`), the status bar
  stays short.
- **Focus point** (2026-10-17): marker of the autofocus point in full screen and in
  comparison (`m`), read from the RAW (Fujifilm for now); square, cross, circle or heart
  (`[gui.view] focus_marker`), colour `[gui.colors] focus`, dark outline underneath to
  stay visible on any background.
- **Display state kept** (2026-10-17, `gui/src/state.rs`): what is shown or hidden with a
  key (focus point, sharpness map, zones, histogram, panels, bars, auto-advance) is
  restored at the next launch, from `gui-state.toml` in the data folder; the
  configuration only gives the first-launch value (`[gui] remember_display = false`:
  always that value). Written only when the state changes (temporary file then rename),
  never during an automatic screenshot nor in tests (temporary data folder). **The window
  too** (2026-10-25, issue #58): size outside maximization, frame position (absent on
  Wayland, which does not give it) and maximized state, followed on each frame
  (`TriApp::track_window`) but kept once still for `[gui] window_save_ms` (1 s), or when
  closing; restored at launch (`state::restore_window`), `[gui] window` only used on the
  first one. Under Wayland, egui does not give the inner rectangle either (it needs the
  position): the size comes from the drawing area (feedback of 2026-10-25: nothing was
  kept). The position stays with the compositor: Wayland does not let an application
  place its window.
- **Key hints** at the top right of the full screen view ("b : Camera render / RAW,
  z : 1:1…"): hidden by default since 2026-10-17 (it overlapped the image, F1 opens the
  full help with search); `[gui.view] key_hints = true` to bring it back.
- **1:1 from the grid** (2026-10-17): `z` (or the bar's "1:1" button) opens the selected
  image directly at 100 %, to check sharpness quickly; Esc goes back to the grid. With the
  keyboard and the button, the 1:1 is **centred on the focus point** when the camera
  recorded it (`[gui.view] zoom_on_focus_point`), otherwise in the middle; if the
  metadata has not been read yet, it recentres as soon as it arrives. Double-click always
  zooms on the clicked point.
- **Rating bar, on the right** (2026-10-17): 1:1 (turquoise background when active),
  previous / next image (chevrons, mostly useful at 1:1) and auto-advance (star followed
  by an arrow: "after a rating, move on"; turquoise background when active), as icon
  buttons (`[gui] bar_icon_size`), label and shortcut on hover.
- **Histogram as a smoothed curve** (2026-10-18, feedback: the bars looked like stairs):
  the engine's bins are grouped into classes (`[gui.histogram] classes`), smoothed by a
  Gaussian kernel (`smoothing`, standard deviation in classes), then drawn as a filled
  area per channel with an outline. Wide classes fill the empty bins of a JPEG on the EV
  scale (the former "comb"). Saturated / blocked **dots** in the corners: lit above
  `indicator_min` (0.1 % of the pixels) and no longer from a single pixel (on DSCF0305:
  0.007 % blocked, a few dead pixels, lit the dot while the curve was far from the edge).
  The **large histogram** (`Shift+E`) is a dialog like the others: title, movable,
  buttons at the bottom, at its own width (`Dialog::free_width`).
  Colours in **additive blending** (feedback of 2026-10-18): red + green = yellow, all
  three = white, like light (`fill_strength`). The **small** histogram (panel) only shows
  the curve; the indicator row (source, headroom; saturated and blocked are in the table,
  per channel, feedback of 2026-10-18) and a **statistics table** per channel are only in
  the large one: median, mean, range from shadows to highlights (percentiles
  `stats_tail`, 1 % and 99 %), standard deviation, saturated, blocked, in EV below
  saturation (`Exposure::channel_stats`, computed in the engine from the EV histogram
  already cached). No luminance on RAW data (a photosite measures only one colour): green
  stands in for it, and this is stated.
  **Analysis tool** (user's idea, 2026-10-18): four dots at the bottom right of the large
  histogram (red, green, blue, white = all; `Shift+C` cycles through them and opens the
  large histogram) show only one channel, its single statistics row and its zones. To the
  right of the table, a **small analysis image**: the thumbnail in grey, saturated zones
  in yellow and blocked ones in blue (`analysis_over`, `analysis_under`), from the
  engine's exposure mask (bits per channel, `tri::exposure::mask`); all channels:
  saturated in any channel, blocked in all channels together
  (`histogram::analysis_image`, tested). Layout: since 2026-10-19, a square to the right
  of the histogram, at its height, background slightly lighter than the dialog
  (`analysis_lighten`), image centred, no legend (see below). The movable dialog's "grab"
  pointer no longer replaces an element's pointer (hand on the dots).
- **Refresh** (folders panel): circular arrow icon, label, shortcut (F5) and
  explanation on hover.
- **Dialogs** (right-click rating, ejection, offload, rename, trash; feedback of
  2026-10-17): **movable** by dragging their background or title, larger and more airy
  (`[gui.dialogs]`: margins, spacing, widths; title in the accent colour, `title_color`,
  underlined by a separator line).
  egui's `Modal` cannot be moved (its area also covers the greyed background): `Dialog`
  keeps the position and shifts it while dragging; the frame is in the drag area, and
  texts in it are not selectable (otherwise dragging the title selected text). Buttons
  and fields keep priority.
- **Compact filter bar** (feedback of 2026-10-17: it overflowed on a laptop screen):
  drawn icons (stars; flags: green tick = picked, grey cross = hide rejected, red cross =
  rejected only; triangle (sharpness, 2026-10-21: the crossed-out drop was unclear) = hide blurry, sun = well
  exposed (2026-10-21), both in the accent when on; colours), ◐ inverter and funnel at
  the far right, "26/26" counter; the full label and shortcut of each
  button on hover.
  File type (2026-10-21, tester feedback: "Flat" was not clear): two **toggles**, RAW and
  JPG labels (JPEG, TIFF, PNG, HEIC…) at the size of the other icons (no page icon, user
  feedback 2026-10-21), instead of "All / RAW / Flat"; when lit, the type is shown, one of
  the two always stays lit (`marks::file_type`). Blur strictness slider
  shown only with a sharpness filter, like the stacks one with stacks; same width (90
  points).
- **Side-by-side comparison** (2026-10-17, `c`, `gui/src/compare.rs` for the layout):
  the selected images, or with no selection those of the current stack (2026-10-25,
  issue #52: `c` on a collapsed stack; Esc comes back to its cover; for a stack larger
  than the limit, `f c` shows the whole stack, expanded, in the grid, `f c` again or
  "Clear" gives everything back, and a stack that was collapsed collapses again) (at least 2, at most `[gui.compare] max`, 6 by default: each keeps
  its preview and its 1:1 in graphics memory), laid out to be seen as large as possible
  (number of columns chosen from the proportions of the images and of the window, last
  row centred). The **active image** (frame in the selection colour) is the current
  image: metadata panel, histogram and bar follow it; arrows / hjkl change it (up / down
  from one row to another), clicking too. Ratings go to the active image only
  (auto-advance: to the next compared image); `Space` **eliminates it** (it leaves the
  comparison and the selection: you keep the best of a burst by elimination); `Enter` or
  double-click opens it in full screen; Esc or `c` goes back to the grid. `z`: **linked
  1:1**, at the same relative point of each image, moved together (drag any of them,
  Shift + arrows); `b` changes the source of all of them; `w` overlays the zones. The
  engine serves all compared images (`Scheduler::set_shown`), not just the current image
  and its neighbours.
- **Multiple selection** (2026-10-17, engine: `tri::selection`, see `selection.md`):
  `Space` marks, `v` visual mode (range following the cursor, `v` confirms, Esc cancels),
  `Ctrl+A` / `Ctrl+Shift+A`, Shift+click (range), Cmd/Ctrl+click (add / remove); Esc
  deselects before leaving full screen. Ratings, trash and editor apply to the visible
  selected images. Orange frame (`[gui.colors] marked`).
- **Developed in darktable** (2026-10-19, see `develop.md`; a whole folder in the background:
  `Ctrl+Shift+B` or the right click on a folder, 2026-10-25, issue #69): `Shift+B` or the aperture
  button starts the render (a few seconds, kept in cache); only afterwards does `b` go
  from the camera render to the RAW render then to the developed image; focus point and
  zones hidden on the developed image.
  **Resolution** in the info line (2026-10-21, `TriApp::shown_resolution`): that of the
  image at 100 % in the displayed source (loaded 1:1, otherwise metadata dimensions;
  camera render: `Metadata::embedded_size`, read from the embedded JPEG's header); for a
  version, its frame, reduced to the long side of its export preset; **while cropping**,
  the current frame and the export preset of the crop bar (live). Developed image (2026-10-21): darktable renders at the preview size;
  the real size, the whole image cropped in darktable, is read from the sidecar
  (`develop::developed_size`, read again when it changes); nothing with a straightening
  or a perspective correction (unknown size rather than a wrong one).
  The "developed" choice persists from one image to the next, but an image **without a
  development** is shown in the previous source (camera or RAW), with its 1:1;
  "not developed yet (Shift+B)" says so (`TriApp::source_for`, feedback of 2026-10-20:
  after a Shift+B, no other image would zoom any more).
- **Export** (2026-10-19, see `export.md`): `Ctrl+E`, dialog (`gui/src/export.rs`) with
  the export presets (up / down arrows on the list only, summary of each; Tab moves to the
  folder) and the folder (Tab completion); export in the background, progress bar and ×
  button in the status bar, summary at the end. Chosen profile remembered for the
  session. Exported image: what you are looking at (grid: darktable, otherwise LibRaw;
  full screen: the displayed source), changeable in the dialog (`export::Render`).
  Pared-down dialog (feedback of 2026-10-19): profiles, folder and actual destination
  only; the explanations (arrows, folder rule, render) are in the docs.
- **Export report** (2026-10-20, see `export.md`): `Ctrl+Shift+E` or "Export report"
  button before the summary in the status bar; `export::report_dialog`, log
  `TriApp::export_log` read at launch. The click on the button that opens it also lands on
  the dialog's background in the same frame: ignored (`export_report_since`).
- **Icon, top bar, focus mode** (2026-10-19): icon chosen among three options (a "t" and
  the rating star, `scripts/make-icon.py`, images in `gui/assets`) for the window, the
  Dock and the taskbar; **top bar** with the **"t★ri" wordmark** on the left, like
  darktable (user's idea: cleaner than the icon followed by the name; letters tinted in
  the theme's text colour, yellow star, version on hover; `[gui] title_bar`,
  `title_bar_size`). **Focus mode** (`F11`, `Ctrl+Shift+F`: on macOS, F11 shows the
  desktop): system full screen, all bars and panels hidden (not closed: they come back
  as they were), a hint on how to leave it for a few seconds at the top
  (`focus_mode_hint_ms`). Esc does not leave it: it is used to go back to the grid.
- **Full screen, nothing on the image** (2026-10-22): number, source, size and waits
  ("RAW render in progress") in the status bar, after the name; ratings in the rating
  bar. In focus mode, without bars, they are drawn on the image; `Ctrl+I`
  (`core.view.image_info`, `[gui.view] image_info`) hides or shows them, remembered.
- **Background colour** (`Ctrl+L`, `core.view.background`, 2026-10-22, like Lightroom's
  "lights"): grid and full screen (comparison and loupe included), in turn the theme's
  background then `[gui.colors] backgrounds` (dark grey, black, white), without
  duplicates (a colour equal to the theme's background or already seen is skipped:
  `next_background`); remembered from one session to the next.
- **Neutral grey theme** (`[gui] theme = "neutral"`, 2026-10-19, Lightroom style):
  dark grey panels, neutral grey central background in the grid as in full screen (a
  black background makes images look brighter and more contrasted), slightly lighter
  cells, matching stack backgrounds, image names in dark on the cells, input fields
  apart; tones taken from a darktable screenshot (panels `#2a2a2a`, background
  `#5e5e5e`, cells `#767676`, fields `#3e3e3e`, light grey selection `#b3b3b3` and dark
  text, unless `selection` is given); colours `[gui.colors] neutral_*`. Only retouches the
  backgrounds of egui's dark theme (`Theme::apply`), restored when changing theme; applied
  live from the settings.
- **Search** (2026-10-19): `/` in the grid and in the folders panel (in full screen, `/`
  stays "fit"), Ctrl+F everywhere. **Grid**: field after the filter bar's counter
  (`[gui.grid] search_width`; narrow window, issue #71: the sort and the number of images
  go into a "…" menu just before invert and clear, always visible, and the field shrinks
  down to `search_min_width`; the right-hand buttons of the rating bar wrap to a second
  line, and stay there while the width does not change; widths measured on the previous
  frame), live filter by file name (`tri::fuzzy::NameFilter`,
  combined with the other filters, "Clear" empties it too); Enter keeps the filter and
  gives the keyboard back to the grid, Esc clears it. **Folders**: field under the title;
  during the search, the results replace the tree (see `places.md`: `~/…`, `/…`,
  otherwise from the pointed folder); ↑ ↓ choose, Enter or a click opens (and shows the
  folder in the tree), Esc closes. egui takes the keyboard away from the field as soon as
  Esc arrives: `search_focused` remembers the active field from the previous frame so
  that Esc goes to it.
- **Large histogram reworked** (2026-10-19): analysis photo in a **square to the right of
  the histogram, at its height**, no legend ("the colours speak for themselves"); hidden
  by default (`[gui.histogram] analysis`), `p` or the photo icon (left of the scale icon,
  in the accent colour when shown) toggles it, and the histogram then takes the full
  width; statistics below. Movements (arrows, hjkl, Home / End, pages) pass under the
  dialog: you browse the images, the histogram follows. The scale is toggled by an
  **icon** to the left of the channel dots (straight line: linear, logarithmic curve: EV;
  it shows the current scale), no longer by a text button next to "Close".
- **Overlays** (`o`, `Shift+O` orientation, like Lightroom, 2026-10-19, see
  `overlay.md`; saturated / blocked zones move from `o` to `w`): built-in grids and user
  overlays (text, logo, 4:5 format…), in full screen and in comparison; name in the
  status bar.
- **AirDrop** (2026-10-19, see `send.md`): action `plugin.airdrop.send`, share button in
  the right-click dialog, no default key; macOS only (hidden elsewhere:
  `Action::available`).
- **Keyboard in the help and settings** (feedback of 2026-10-18): the help scrolls with the
  up / down arrows (`[gui.dialogs] help_scroll_line`), Page up / down and Ctrl+U /
  Ctrl+D, the search keeping the letters. Settings: the section list scrolls to the
  chosen one with the arrows (and the settings start again from the top); from the
  search, → (empty search), Enter or Tab give the keyboard to the first setting shown,
  then Tab / Shift+Tab move from one to the next. A test goes through all sections: it
  found a "←" in the example's comments, missing from egui's font (replaced by words).
- **Hidden cursor** (2026-10-18, to see the stacks as a whole): in the grid, Esc when
  there is nothing left to close (multiple selection already emptied), or a click in an
  empty area (which also empties the selection), hides the highlighted image. An action
  aimed at the current image (move, rate, delete, open, select…, `uses_cursor`) only makes
  it reappear, in its place, without acting on an image you could not see; a click on an
  image too. Filters, panels, bars remain usable with the cursor hidden. Clicking the
  image again or Space ruled out: double-click (open) and multiple selection.
- **Shortcuts in the settings window** ("Shortcuts [keys]" section, 2026-10-18): one row
  per action, by theme like the help (the search also finds actions: label, identifier,
  keys). Its shortcuts as chips (those of the profile, or those of `[keys]`: label in
  bold), a click removes one; "record…" takes the keys typed next (Esc included, the
  window does not close; a sequence key after key, ended after the sequence delay; a
  character without a named key, "°", taken as a character); the small field accepts a
  written shortcut ("g g"); "default" goes back to the profile. Conflicts are shown at the
  top and under the action. `Keymap::specs_for` / `keys_spec` write shortcuts the way
  `[keys]` reads them (a test checks the round trip of all the profiles' shortcuts).
- **Settings window** (`Ctrl+,`, 2026-10-18; `gui/src/settings_window.rs`, engine:
  `tri::config_edit`, see `config_edit.md`): sections on the left (up / down arrows from
  the search), settings and their comments on the right, fuzzy search like the help
  (name, section, description). Field by type: checkbox, number to drag or type, colour
  (picker), text or TOML list (written on confirmation). Key present in the file in bold (said in the window and on hover, issue #6),
  "default" button to remove it. Each change is validated (whole file reread) then
  written (when the mouse is released); a rejected value reverts with the error message.
  Applied immediately: interface (`[gui.*]`), shortcuts, language, `[files]`, `[xmp]`,
  `[offload]`; the engine (`[preview]`, `[render]`, `[cache]`, `[scheduler]`,
  `[exposure]`, `[sharpness]`, `[places]`, `[data]`) and the window size at the next
  launch, as stated. File: the one read at launch, otherwise `--config` or the default
  location. Sections named by a translated label followed by the file name in grey
  ("Colours [gui.colors]", `settings-section-*` in the catalogues; a test checks that no
  section lacks one). Folder keys (`dir`, `*_dir`, `destination`) have Tab completion.
- **Continuous zoom and loupe** (2026-10-18, `[gui.zoom]`): the scale (1.0 = 100 %, one
  image pixel per screen pixel) goes from fit to `max` (400 %). Keyboard, in **steps**
  like Lightroom: `+` / `-` (top row or numeric keypad: bound as characters, the key
  depending on the layout) next step up / down (`steps`), `*` 100 %, `/` fit (swapped on
  2026-10-18, like Lightroom), `z` and double-click toggle fit / 100 %; you can **zoom out
  below fit** down to `min` (10 %), the image smaller than the screen, centred; the 1:1
  button is lit only at 100 %.
  Mouse and trackpad, **continuous**: pinch, or Ctrl / Cmd + wheel, zooms around the
  pointer; scrolling moves the zoomed image. Beyond fit, the 1:1 is what is displayed,
  reduced or enlarged; the scale is shown ("200 %"); it is kept from one image to the
  next, like the position, and also applies in comparison. **Loupe**: in fit view, while
  `loupe_key` is held (`t` by default since 2026-10-19: with Shift, every Shift+… shortcut
  made it appear; a free key or a modifier, a key already bound to an action is reported;
  `<` impossible, egui only receives its character), after `loupe_delay_ms` (no flicker
  on a short press), a square of `loupe_size` points under the pointer shows the image at
  `loupe_scale` (100 %); its 1:1 is loaded for the occasion, the enlarged preview shown
  while waiting ("loading 100 %…").
- **"Expand all" button** next to the similarity slider (`Shift+S`).
- **Stacks** (2026-10-18, `gui/src/stacks.rs`, engine `tri::similarity`, see
  `similarity.md`): similar neighbouring images (bursts, series) form stacks, collapsed
  by default: one thumbnail, the **cover** (the picked one, otherwise the best rated,
  otherwise the sharpest, otherwise the first), with the number of images in an accent
  dot and sheets behind; expanded, a continuous accent line above its images. `s`
  expands / collapses the image's stack, `Shift+S` all of them. The grid is only a
  **view**: the full list remains the engine's (per-image computations, sharpness);
  navigation and auto-advance skip hidden images. **Grouping window** (`Ctrl+G`, like
  Capture One): group or not, threshold slider (the grid follows live), maximum time gap,
  number of stacks; the threshold is remembered for the folder on closing
  (`group-thresholds.tsv` in the data folder; the default: `[similarity] threshold`).
  Second feedback of 2026-10-18: **a single toggle** (`Ctrl+S`, stack button in the
  filter bar and the rating bar, accent background when active, always visible; right
  click: `Ctrl+G`): stacks active, grouping in the grid **and** in full screen ← / →
  within the stack, ↑ / ↓ from stack to stack; inactive, ← / → from photo to photo.
  Stacks in **shades of grey**, no more turquoise. Third feedback: with all stacks
  expanded, frames and lines were no longer readable (a line under every image); an
  expanded stack now has a **background**, two greys alternating from one stack to the
  next (`[gui.colors] stack_band`, `stack_band_alt`), as while adjusting the slider.
  Grid **not grouped at startup** (feedback of 2026-10-19): `[gui.grid] stacks`
  (false by default); fingerprints are computed in the background anyway
  (`[similarity] background`), `Ctrl+S` groups at once.
  Fourth feedback: an isolated photo is a group of one image, with its own background too
  (two neighbouring isolated photos no longer blend together): stacks active, each group
  alternates, two cases instead of three.
  Feedback of 2026-10-18 (since replaced): frame around each image and line at the
  bottom; **similarity slider** in the filter bar (stack icon: `Ctrl+G`): while
  adjusting, stacks stay collapsed, you see the resulting stacks (feedback of
  2026-10-20); **Shift** held while adjusting expands them, every other one on a lighter
  background (`stack_band`); threshold remembered for the folder on release.
  Since 2026-10-21, a **strictness slider** like the blur one: centre computed from the
  folder, to the right stricter (smaller stacks), double-click: back to the middle (see
  `similarity.md`); likewise in the `Ctrl+G` window. **Stack
  mode** in full screen (`Ctrl+S`, bar button, inspired by Capture One's culling mode):
  ← / → browse the stack, hidden images included (at the end: stop, or back to the other
  end with `[gui.view] stack_wrap`), ↑ / ↓ go to the cover of the previous / next stack
  (↑ ↓ buttons next to ‹ ›), "stack: 3 / 12" in the status bar. **The favourite**:
  Enter in stack mode, or `Shift+Enter`, makes the image the cover of its stack,
  separate from ratings (one per stack, remembered: `stack-covers.txt` in the data
  folder; choosing it again forgets it).
- **Panels from the keyboard, tmux style** (2026-10-18, `actions::Scope`): the leader
  (`[gui] leader`, `Ctrl+B` by default) then `h` / `l` (or ← / →) gives the keyboard to
  the left / right panel, `o` to the next one (leader then `b`: report a problem,
  2026-10-22) (folders, images, metadata; a hidden panel
  is shown, folders only exist in the grid). The active panel has a slightly lighter background (darker in the light theme, `[gui.dialogs] focus_panel_tint`; an accent outline until 2026-10-25, issue #78: with the accent cursor, the double frame was heavy; the metadata cursor no longer frames the histogram, issue #77); the line under the keyboard cursor (folders, metadata, search results) is a **discreet grey band** across the whole width (`[gui.dialogs] cursor_tint`, `cursor_row`; an accent frame before 2026-10-25, too loud and cut on the left when the panel scrolled), the
  status bar says so; a click in a panel gives it the keyboard, a click in the images
  area gives it back to them (`TriApp::click_focus`, 2026-10-22; dialogs opened on top
  do not count). **Its keys go first, the others keep their effect** (user's
  choice: `3` still rates the current image); Esc gives the keyboard back to the images.
  Folders: ↑ ↓ / `j` `k`, → / `l` expands (already expanded: enters), ← / `h` collapses
  (already collapsed: goes up), Home / `gg`, End / `G`, Enter opens and gives the keyboard
  back to the images (cursor in the engine: `places::Tree::cursor`…); on a removable
  drive, `o` offloads it and `e` ejects it (same dialogs as the icons; `o` rather than
  `d`, which would shadow vim's `dd`); on an ordinary folder, `o` **offloads here**:
  offload dialog with this folder as root (not remembered for the next ones), and a list
  to choose the card if there are several. Folders (idea of 2026-10-18, like nvim-tree):
  `a` creates a folder in the pointed folder, `r` / F2 renames it, `x` / Delete deletes it
  (confirmed, with what it contains; trash according to `[files] trash`); places cannot
  be renamed or deleted; everything can be undone (`u`), the open folder follows its
  renaming. **Favourites** (2026-10-18): `m` ("mark", like nvim-tree's bookmarks) adds the
  pointed folder to the favourites or removes it; they are at the top of the panel,
  before the volumes, marked with a star (the folder too, where it appears in the tree),
  and follow a renaming done in `tri`. `n` (or the right click) **names** the pointed
  favourite (2026-10-22): that name in the favourites section, the path on hover; a
  favourite unfolds without touching the same folder in the tree. With the mouse:
  **right click** on a folder, menu of the same actions (new folder, rename, delete,
  favourite; offload a card here, or on a removable drive: offload, eject), shortcut on
  hover.
  Metadata: the histogram at the top then each row (sharpness included) can be pointed at
  (↑ ↓ / `j` `k`), Enter on the histogram opens it large; Shift + ↑ ↓ / Shift + `j` `k`
  (or Shift+click) extend the selection, `y` copies it (one row: the value; several:
  "label: value" per row; `Ctrl+C` / `Cmd+C` too), Shift+`y` copies the whole panel; a
  click on a row points at it and gives the keyboard to the panel. Large histogram open:
  `r`, `g`, `b` one channel, `a` all (it takes the keys while open: `b` does not change
  the source there). A single keymap: each action states its panel (`Action::scope`);
  when looking up a key, the active panel's one wins over the general action, and
  conflicts are only reported between actions of the same panel. No `z` (enlarge the
  panel, like tmux) for now: `Shift+D` / `Shift+M` already hide the panels.
- **Undo / redo** (2026-10-18, engine `tri::history`, see `history.md`): `u` or
  `Ctrl+Z` (`Cmd+Z` on macOS), `Ctrl+R` or `Ctrl+Shift+Z`; `Ctrl+H`: list of the
  session's history, clicking a step goes back to it. Ratings, renaming, copy / move,
  trash; refusal explained if a file changed elsewhere.
- **Path fields** (2026-10-18, `gui/src/path_field.rs`): wherever you type an existing
  folder (copy / move, offload root, settings), Tab completes like a terminal: a single
  possible folder is taken; otherwise the common prefix, and the list of folders under
  the field.
- **Close cross** (2026-10-24, issue #39): every dialog (common `Dialog` frame) has the
  same cross at the top right, at the title's height, labelled "Close"; no more "Close"
  button at the bottom (Cancel stays in dialogs to confirm). Without a cross
  (`Dialog::busy`): copy and deletion in progress, which cannot be interrupted. The help,
  an egui window, already had its own.
- **Cache kept with the photos** (2026-10-24, issue #55): right click on a folder of
  the panel, "Keep the cache in this folder" (on a removable drive: "on this drive"), or
  "No longer keep the cache here"; not offered where `[cache] location` already puts it.
  When a folder is opened, `location` (`disk`, `folder`) applies if its cache is still
  the user's. Moving a folder: "Bring the cache along" checkbox (ticked); renaming: the
  cache always follows. The work is done in the background (`TriApp::cache_job`), message
  in the status bar.
- **Pointer in dialogs**: the hand on everything clickable (`interact_cursor` in the
  common `Dialog` frame); "grab" elsewhere, to move the dialog.
- **File actions** (2026-10-17, engine: `tri::files`, see `files.md`): rename (`F2`),
  move to trash (`Delete`, `Backspace` — the Mac "delete" key — or `x`, as in vim; `dd`
  removed on 2026-10-18), show in the Finder / file manager (`Shift+R`), open in the
  external editor (`e`, `[files] editor`), copy (`y`) or move (`Ctrl+X`, cut) to a
  folder, **the whole stack** of the image: `d)` (trash) / `y)` (copy): a stack is a
  sentence, as in vim (2026-10-25, issue #62, the user's choice; `dw` / `yw` before, a
  stack as a word, idea of 2026-10-18: a grid row, `dd`, made no sense; stacks must be
  active), and `(` / `)` go to its start and end (Shift+Home / Shift+End in classic;
  `[` / `]` ruled out: Alt+Shift on a macOS AZERTY). Read as characters: direct on
  AZERTY; on QWERTY, Shift+9 / Shift+0 give them (and so no longer rate in the vim
  profile: 9 and 0 alone remain). Since `y` is also the start of `y)`, it waits briefly for what follows
  (`[gui] key_ambiguous_timeout_ms`, 300 ms): a key that is a shortcut and the start of
  another is no longer reported as shadowed, the sequencer (`Sequencer::expire`,
  `deadline`) runs it after the delay, or before the next key that does not continue.
  Copy / move (2026-10-18, see `files.md`: field with Tab completion, recent destinations
  with the arrows, progress). Also through six icon buttons in the right-click dialog
  (pencil, two sheets, arrow into a folder, trash can, folder, outgoing arrow; label and
  shortcut on hover; `[gui.dialogs] icon_size`). Rename: the name (without extension) is
  selected, you type the new one; the dialog shows what becomes of the image, its pair
  and its sidecars, or why it is impossible (name taken); Enter confirms, Esc cancels.
  Trash: the list of files (pair and sidecars included) before confirmation (Enter).
  Then the folder is reread, the view (grid or full screen) kept: the renamed image stays
  selected, after a deletion the next one takes its place. Errors in the status bar.
  Code: `gui/src/files.rs` (dialogs), `TriApp::file_action`, `reload_folder`.
- **Auto-advance after a rating** (`[gui] auto_advance`, disabled by default; `Shift+A`
  or the bar's "auto-advance" button): after a rating from the keyboard or the bar, the
  next image is selected (not from the right-click dialog, which rates the clicked image).
  Two rules (`TriApp::rate_selected`):
  - **under a filter, never skip an image**: if the rating takes the image out of the
    filter (rejection under "without rejected", 2 → 1 star under "2 stars and more"), it
    disappears and the next one already takes its place under the selection; advancing
    as well would skip one (known Capture One bug at one time). So we only advance if the
    rated image is still in the list;
  - **the other image of a RAW+JPEG pair**, rated at the same time, is skipped.
  In full screen in stack mode, it advances within the stack like → (stops at the end of
  the stack, or wraps around with `[gui.view] stack_wrap`), not to the next stack
  (2026-10-24, issue #60). As an option, `[gui.view] stack_advance_next` (off by default)
  moves at the end of the stack to the first photo of the next stack (2026-10-25). From
  stack to stack (↑ / ↓), we land on its cover or, with `[gui.view] stack_enter =
  "first"`, on its first photo: "|‹" button of the full screen bar, next to the stacks
  switch (always there since 2026-10-25; in the grid too, with stacks on, to see its
  state, since 2026-10-26), or Ctrl+Shift+S, remembered from one launch
  to the next (issue #62); a message says which one is on (nothing moves on screen).
  Sidecar write failure: we stay on the image (the error is in the status bar).
- **Rating with the mouse** (2026-10-17, requested by the user: not everyone wants to do
  everything with the keyboard):
  - **rating bar** at the bottom (above the status bar), in the grid and in full screen:
    flags, stars and colours of the selected image, clickable, faded when not set, their
    shortcuts in the tooltip (2026-10-22);
    `Shift+N` shows or hides it (`[gui] rating_bar`); on the right, the "auto-advance"
    button (see below). In full screen, the ratings are drawn on the image only without
    it (hidden, or focus mode; issue #8: they showed twice). In full screen too, after the
    ratings: the grid's badges, blown / blocked (click: large histogram) and blurry
    (click: sharpness map; issue #9);
  - **right click** on a thumbnail or in full screen: **modal dialog** with the same
    controls, for the clicked (selected) image. A simple menu was not enough (feedback of
    2026-10-17): the mouse leaving the menu selected other thumbnails, while the rating
    went to the clicked image. Behind the dialog, nothing moves: no hover, no navigation;
    only the rating keys (for this image) and Esc act. It stays open to set several
    ratings, and closes with Esc, "Close" or a click outside. It opens **at the click
    point** (`modal_at`), not in the centre: on a large screen, the way to the centre was
    long (feedback of 2026-10-17, same for the eject confirmation, opened near the icon);
  - **reaction on hover** (same feedback): on the clickable stars, dots and flags (bar,
    dialog, filter bar), hand pointer and highlighted background (`marks::clickable`);
  - each click triggers the same action as the keyboard. **Flag**: clicking it again does
    not clear it (feedback of 2026-10-17: a real danger in fast culling), a separate empty
    circle does ("Neither picked nor rejected", like Shift+U). **Stars**: since
    2026-10-21, **clicking the set star again resets to zero**, like the star filter (no
    accidental double click to fear with the mouse); the "⊘" control (remove stars) is
    gone. On the keyboard, only the 0 key resets to zero (`1` twice stays at 1). Colours
    are added or removed on click, as with the keyboard;
  - controls named for accessibility (`WidgetInfo`), which also lets the interface tests
    click "3 stars".
- **Card offload** (2026-10-17): "offload" icon on each non-fixed volume of the folders
  panel; dialog near the click: inventory in the background, root and session folder
  (suggested `YYYY-MM-DD_`; root remembered), checkboxes (RAW only, open the folder,
  eject the card, delete the copied and verified files from the card), verified copy in
  the background with progress and "Interrupt", then the ticked follow-ups chained
  (deletion, opening, ejection) and summary. **In the background** ("Continue in the
  background", Esc): the copy continues, clickable progress in the status bar; at the
  end, the session is offered ("Open the session") rather than opened automatically. See
  [offload.md](offload.md). The work is in the engine ([offload.md](offload.md)), the
  dialog (`offload.rs`) only displays.
- **Metadata panel** (2026-10-17), on the right, in the grid and in full screen
  (`Shift+M`, `[gui.metadata]`): file name as the first row (2026-10-22, rather than a
  title), then the engine rows (`Metadata::rows`, see
  [metadata.md](metadata.md)) for the selected image, values wrapped rather than widening
  the panel (lens names). Reading in a separate thread (`meta.rs`, `MetaLoader`): a few ms
  locally, but a NAS must not freeze the interface; results kept in memory, forgotten
  when the folder changes. In the grid, the panel also shows the **histogram** of the
  selected image (in full screen, it is on the image); its indicators wrap when the panel
  is narrow.
- **Histogram and exposure warnings** (2026-10-17), computed by the engine on the
  **sensor data** for a RAW, even when the camera render is on screen (see
  [exposure.md](exposure.md)); the drawing is in `histogram.rs`:
  - **histogram** at the top of the metadata panel, in the grid as in full screen
    (`i` shows or hides it); panel hidden: no histogram (request of 2026-10-17: the
    overlay on the image got in the way more than it helped). **Large histogram** in a
    centred modal dialog (`Shift+E`, or click on the panel's histogram;
    `[gui.histogram] large`: share of the window width), closed by Esc, the same key or a
    click outside;
    range = **useful range of the image** (~13.9 EV for a 14-bit RAW, ~11.7 for a JPEG;
    `[gui.histogram] ev_shown`: ceiling, 0 = automatic); R, G, B channels overlaid, one
    bar per bin (not per pixel, otherwise a "comb"), scale **in EV below saturation** by
    default (`Shift+I`: linear), graduations, red / blue dots if pixels are saturated /
    blocked, and in plain words: RAW bit depth, headroom before saturation, saturated and
    blocked shares;
  - **zones** overlaid on the image (`w`, "warnings"; `o` until 2026-10-19): engine mask as
    a texture (nearest-neighbour sampling), stretched over the image (also at 100 %).
    Saturated **per channel** (requested on 2026-10-17, like FastRawViewer): a single
    channel → its colour (`clip_red`, `clip_green`, `clip_blue`), two channels → colours
    added (red + green = yellow), all three → `clip_over` (magenta); blocked →
    `clip_under` (cyan: blue was confused with "blue alone saturated" and was hard to see
    in the shadows).
    `[gui.histogram] clipping_channels = false`: a single colour for any saturation.
    Configurable colours and opacity;
  - **badges** in the grid: half-white half-black triangle at the top left of the
    thumbnail (dots at the top right until 2026-10-21; click: large histogram) above `[gui.histogram] badge_over` / `badge_under`, details on hover;
    analyses read from the cache within the image's loading budget; not on a collapsed
    stack (issue #75: its image count is there);
  - checked on a real shoot: fully blown photo (red everywhere, badge), portrait in a
    black sweater (blue zones in the folds, blue badge, -3.5 EV of headroom), portraits on
    a white background (nothing, -0.45 EV of headroom).
- **RAW+JPEG pairs**: rating `IMG.RAF` also rates `IMG.JPG` (user's choice, 2026-10-17),
  see [notes.md](notes.md).
- **Help** (`F1` or `?`, closed by Esc or `F1`): each action with an understandable label
  and its keys, **arranged by theme** (Navigation, Display, Rating, Filters, Panels,
  Application: `Action::group`); the technical identifier only appears on hover. The help
  is **modal**: while it is open, only Esc, help and Quit act. **Prepared beforehand**
  (issue #11, 2026-10-22: ~75 ms per frame, the opening fade stuttered): lines
  (`HelpLines`) and column widths computed during the first frames, redone only when the
  search, the shortcuts or the language change; only the visible lines are drawn
  (`ScrollArea::show_rows`). About 0.3 ms per frame.
- **Search in the help, fzf style**: the field at the top has focus as soon as it opens,
  you type directly. Each word of the query must be found letter by letter, in order,
  possibly with gaps, in the label, the keys or the identifier ("pnd" → *panneau des
  dossiers*, "etoile 3", "maj+e", "core.nav"), ignoring accents and case; words may be in
  any order. Results sorted by relevance (`tri::fuzzy`: consecutive letters and word
  starts count more, gaps are penalized; on a tie, the shortest label). While typing, `?`
  is written into the field: close with Esc or `F1`.
- **Search, help and settings** (reworked on 2026-10-22: you often know only roughly
  what you are looking for): the help's **topics** and the settings' **sections** stay
  visible while searching (in the settings, the left list puts the sections found first,
  with their number of results, the others greyed out; a click leaves the search and
  opens the section as without search, for a family its best-found member — issue #24,
  2026-10-23: it only brought its results into view, and a family showed all its members
  one after the other); a section's or topic's **title** finds all of it; **hidden
  keywords** per setting, section or action (`kw-<id>` in `gui/locales/*.ftl`,
  `actions::keywords`; "vim" finds the shortcut set, "focus point" the autofocus marker;
  a test checks that each names something that exists); and when a typed word appears
  **as is** somewhere, matches with scattered letters are dropped (`tri::fuzzy::find` and
  `relevant`): "vim" no longer drowns the right setting among all those where a v, an i
  and an m happen to be.
- **Typing in a field** (help search, offload folder name): keys are text; only those
  that type nothing (Esc, F1 to F12: `Trigger::types_no_text`) remain shortcuts.
- Standard preview size **"Auto"**: largest side of the screen, in physical pixels.
- Dark theme by default.

- **Full screen** (Enter or double-click): see below.

## Full screen

Designed for culling: in a fast pass, the **camera render** serves as the base (available
within a few seconds for the whole folder) while the RAW renders are computed behind;
then the sharpness of the candidates is checked **at 100 %**.

- **Two independent settings** (choice of 2026-10-16): `b` chooses the **source** (camera
  JPEG by default, or RAW render), `z` chooses the **scale** (fit or 1:1). Zooming never
  changes the source, so never the "look" of the image; `b` while zoomed goes from one 1:1
  to the other at the same place. The chosen source stays active from one image to the
  next. If the wanted source has not been computed yet, the best available one is shown
  (the other source, then the enlarged thumbnail). Flat images have only one source.
- **RAW renders on demand**: in JPEG mode, no RAW render is computed, the machine stays
  free. In RAW mode, the interface asks the scheduler for the render of the displayed
  image and its neighbours (`Scheduler::request`, policy `RawPreviews::OnDemand`); a first
  `b` costs ~0.5 s. Computing the whole folder in the background remains possible
  (`RawPreviews::Background`, default of the engine and of the command-line demo).
- **What is displayed is always written** at the top left: position, name, "Camera
  JPEG", "RAW render", "RAW render · 100 %", "Thumbnail (preview being computed)", and
  "computing 100 %" when applicable. The camera render reflects the camera's settings
  (contrast, film simulation); LibRaw's RAW render is more neutral, hence duller: this is
  expected.
- **1:1**: `z` (centred), or double-click (the clicked point stays under the cursor).
  Dragging with the mouse moves the image, never leaving an empty edge.
  - in JPEG: the embedded JPEG at its original size, ~70 ms. Honest label: "Camera
    JPEG · 1:1 (71 % of the sensor)" (4416 px out of 6240 for an X-T4). The camera's
    sharpening and noise reduction: answers "is it sharp enough for what I'll do with
    it?";
  - in RAW: the full render, ~4–5 s, "RAW render · 100 %". Answers "is the focus really
    on the eye?".
  Meanwhile, the image stays fitted with the note "loading 1:1" or "computing 100 %". The
  framing of the embedded JPEG may differ by a few pixels from the RAW's; the relative
  position is kept, so the same area to within a few pixels.
- **Navigation**: ←/→ (h/l), Home/End (g/Shift+G). **At 1:1, the zoom and the displayed
  area are kept from one image to the next**: you compare the sharpness of several shots
  at the same place. Esc goes back to the grid (on the displayed image).
  - **Preloading**: when zoomed, the 1:1 of both neighbours is requested and loaded in
    advance; when moving forward one image, its 1:1 is usually already there (measured:
    immediate display).
  - **Never through the fit view nor a black screen**: if the new image's 1:1 is not
    ready yet, the best available preview is shown **already at 1:1 scale** (size
    estimated from the last 1:1 displayed, same camera), blurry for a moment then sharp,
    as in Lightroom. As a last resort, the thumbnail (loaded in 1–2 ms) fills in while
    waiting.
  - RAW mode: the source stays RAW from one image to the next. If no RAW render is ready
    yet for the new image, the camera render is shown meanwhile, with the note "RAW render
    in progress" (without it, users thought RAW mode was disabled: feedback of
    2026-10-16). Requests are sent **in priority order** (displayed image fitted, its 1:1,
    then the neighbours): sent in another order, a 100 % (~4.5 s, not interruptible) got
    ahead of the fitted render (~0.5 s) and the JPEG stayed on screen for several
    seconds.
  - RAW mode: a 100 % RAW (~4.5 s, which LibRaw cannot interrupt) is only requested after
    **0.4 s spent on the image** (`gui.view.full_raw_dwell_ms`). Without this delay, when
    pressing the arrows quickly in a row, each image passed through could start its
    100 %, and the RAW render of the image where you stopped waited up to ~5 s behind a
    computation that had become useless (the JPEG stayed on screen: feedback of
    2026-10-16). Measured after the fix: no 100 % request for the images passed through,
    RAW render of the arrival image in 0.8 s. The fitted render (~0.5 s) and the JPEG 1:1
    (fast) are still requested immediately.
  - RAW mode: preloading goes through the single LibRaw worker (~4.5 s per 1:1). When
    skipping an image, the wanted 1:1 waits for the end of the neighbour's one already
    started (X-Trans demosaicing cannot be interrupted): up to ~6 s measured.
  - In a RAW+JPEG folder, the neighbours of a .RAF are its .JPG: RAW preloading is only
    really useful with the "RAW only" filter (`f f`).
  - In a RAW+JPEG folder, the 1:1 of a .JPG (6240 px) and that of a .RAF's embedded JPEG
    (4416 px) do not have the same magnification: that is the principle of 1:1 (one
    image pixel per screen pixel).
- **Smoothness**: large images are decoded by a loading thread (`loader.rs`): the
  displayed image first, then its neighbours n+1 and n-1 (preloading). The list replaces
  the previous one: an outdated image is never loaded for nothing. Large textures live in
  an 8-entry LRU (a 26 Mpx 1:1 takes ~100 MB of graphics memory). An image larger than
  the GPU's texture limit is reduced for display (its original size is still used for the
  geometry).

## Modules

| File | Role |
|---|---|
| `main.rs` | Arguments, window, theme |
| `app.rs` | `TriApp`: state, engine events, drawing |
| `actions.rs` | Named actions and shortcuts |
| `grid.rs` | Pure grid computations: columns, navigation, scrolling |
| `textures.rs` | LRU texture cache |
| `view.rs` | Pure full screen logic: source choice, fit, 100 %, panning |
| `loader.rs` | Loading thread for large images |
| `marks.rs` | Drawing of ratings: stars, colour dots, flag |

### Actions and shortcuts (`actions.rs`)

A key never calls a function directly: it is translated into an `Action`, with a stable
identifier (`core.nav.next`, `core.filter.cycle`…), as decided in `CLAUDE.md`. The
`[keys]` section of the configuration file binds keys to these identifiers (see below);
the mouse triggers the same actions.

Two shortcut sets are shipped, **to choose from** (`[gui] profile`: `"classic"` by
default, `"vim"`, or `"both"` for both together, which do not overlap; drop-down list in
the settings window). Decided on 2026-10-18: a user of classic tools has no use for vim
keys, and vice versa; what is the same for both is written only once, in the common lists
(`BASICS`, rating, filters, selection, files, panels, histogram; `actions.rs`), what is
specific to each set in `CLASSIC` / `VIM`. A test checks that each set covers all actions,
without conflict. The help and the texts that mention a key read it from the table in
effect: they follow the chosen set.

| Action | Classic | Vim |
|---|---|---|
| `core.nav.prev` / `core.nav.next` | ← / → | h / l |
| `core.nav.up` / `core.nav.down` | ↑ / ↓ | k / j |
| `core.nav.first` / `core.nav.last` | Home / End | gg / Shift+G |
| `core.nav.page_up` / `core.nav.page_down` | Page up / down | Ctrl+U / Ctrl+D |
| `core.view.open` | Enter | Enter |
| `core.view.zoom` | z | z |
| `core.view.source` | b | b |
| `core.view.pan_left` / `pan_right` | Shift+Left arrow / right | Shift+h / Shift+l |
| `core.view.pan_up` / `pan_down` | Shift+Up arrow / down | Shift+k / Shift+j |
| `core.view.back` | Esc | Esc |
| `core.app.help` | F1, ? | F1, ? |
| `core.app.quit` | q | q |
| `core.file.copy` / `core.file.move` | Ctrl+C / Ctrl+X | y / Ctrl+X |
| `core.file.copy_stack` / `delete_stack` | Ctrl+Shift+C / Shift+Delete | y) / d) |
| `core.stack.first` / `core.stack.last` | Shift+Home / Shift+End | ( / ) |
| `core.stack.enter` | Ctrl+Shift+S | Ctrl+Shift+S |
| `core.file.delete` | Delete, Backspace | x, Delete, Backspace |
| `core.edit.undo` / `redo` | Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y | u, Ctrl+Z / Ctrl+R, Ctrl+Shift+Z |
| panels (folders, metadata) | arrows, Home / End, Shift + arrows | j k h l, gg / G, Shift + j k, y |

Rating, common to both profiles (2026-10-17, designed for AZERTY too):

| Action | Keys |
|---|---|
| `core.rate.star.0` … `core.rate.star.5` | 0 … 5, with or without Shift |
| `core.rate.color.red` / `yellow` / `green` / `blue` | 6 / 7 / 8 / 9, with or without Shift |
| `core.rate.color.purple` | ° / , / . |
| `core.flag.accept` / `reject` / `none` | Shift+P / Shift+X / Shift+U |

Why: on AZERTY, the digits of the top row need Shift, those of the numeric keypad do not;
egui-winit translates the *logical* key and, if it does not know it ("&", "é"), the
*physical* key: both give `Num1`… Exception: the unshifted 6 on AZERTY is "-", recognized
as `Minus`, hence Shift on the top row. Purple is on "°" (Shift + the key to the right of
0 on AZERTY), which egui does not name: it is a **character**, received through the `Text`
event; only characters bound to a shortcut are taken into account (the others would
interrupt a sequence). The comma (`Comma`) and the period (`Period`) cover the numeric
keypad's decimal separator, "," on macOS but "." on Linux Mint (observed on 2026-10-17;
egui does not distinguish the keypad), as well as the keys of the bottom row. `u`
alone stays free for undo (to come), as in vim.

A colour already set is removed by the same key; in single-colour mode (default),
another colour replaces the previous one.

Cmd and Alt trigger no action: these combinations stay with the system (Cmd+Q…).
Ctrl is allowed (Ctrl+U / Ctrl+D of the vim profile); on Linux and Windows, egui also
reports Ctrl as `command`: only `mac_cmd`, or `command` without Ctrl, means Cmd. No
Ctrl + arrows shortcut (on macOS, it switches desktop), hence Shift for panning.

Filters, common to both profiles: see "Filters" above (`core.filter.*`: `f f`,
`f 0`…`f 5`, `f p` / `f u` / `f x`, `f 6`…`f 9`, `f °`, `f ,`, `f .`, `f Esc`);
`core.view.filter_bar`: Shift+F; `core.view.rating_bar`: Shift+N; `core.view.folders`:
Shift+D; `core.view.refresh_folders`: F5; `core.view.histogram`: i;
`core.view.histogram_scale`: Shift+I; `core.view.histogram_channel`: Shift+C;
`core.view.clipping`: o; `core.view.metadata`:
Shift+M.

Characters: "°" arrives as a key (Shift + `Minus`, physical, egui not knowing "°")
**then** as a character, sometimes in a following frame (observed on 2026-10-17 on macOS
and Linux: `f °` rated purple instead of filtering). Two safeguards: `Keymap::chords`
ignores a key followed by a bound character in the same frame, and the `Sequencer`
ignores, in the middle of a sequence, a key bound to no shortcut (even alone): `f` keeps
waiting until the character. A key that has a meaning alone always interrupts the
sequence (`g` then `j`: row below). `f &` works on AZERTY because egui, not knowing "&",
falls back on the physical key `1`.

**`[keys]` section** of the configuration file, arranged by action:

```toml
[keys]
"core.flag.accept" = ["Maj+P", "a"]
"core.nav.first" = ["Début", "g g"]
```

- the listed shortcuts **replace** those of the profiles for that action (empty list: none
  at all); the chosen set (`[gui] profile`) remains the base;
- a key is written the way the help displays it ("Left arrow", "Page up", "Esc",
  "Shift+P", "Ctrl+u") or with egui's name ("ArrowLeft", "PageUp"); a single capital
  letter means Shift + the letter ("G", as in vim); a sequence is separated by spaces
  ("g g"); a character without a key name ("°") is recognized as a character;
- problems reported at startup (stderr and status bar, details on hover), without
  invalidating the rest of the file: unknown action or key, shortcut assigned to two
  actions (the first in identifier order is kept; a shortcut from the file wins over a
  profile's, and says so). A shortcut that starts a longer sequence is no longer a
  problem: it waits briefly for what follows (`[gui] key_ambiguous_timeout_ms`);
- a test checks that each default shortcut, written as the help displays it, reads back
  identically.

**Key sequences**: a shortcut can be a series of keys, like `gg` (vim). The first is put
on hold (`Sequencer`); the next must arrive before `[gui] key_sequence_timeout_ms` (1000
ms by default, like vim's `timeoutlen`). If it completes no sequence, it acts alone (`g`
then `j`: row below). No shortcut is both a single key and the start of a sequence
(checked by a test): no need to wait for the timeout to decide.

**Panning in a zoomed image**: Shift + arrows or Shift + hjkl, by a fraction of the view
per press (`[gui.view] pan_step`, 0.2), key repeat possible; no effect when not zoomed.

Each action has two names: a stable **identifier** for files and code
(`core.view.back`), and a **label** for the user ("Close / go back").
`core.view.back` closes what is in front: the help, otherwise the full screen view.

Keys are displayed with **names spelled out in full** (`key_label`: "Left arrow",
"Home", "Page down", "Esc"), in a monospace font so that "l" and "I" remain distinct. No
symbols: "⏴" (name provided by egui) and even "←" are missing from egui's default font.
The interface tests fail on any character without a glyph, which guarantees that the
help displays everywhere.

The help scrolls if it exceeds 75 % of the window height (33 actions as of 2026-10-17).

### Grid (`grid.rs`)

Pure functions, tested without a window:

- `Layout::fit(largeur, cellule, espace)`: number of columns; `rows`, `row_pitch`,
  `rows_in`, `width`;
- `navigate(sélection, n, layout, lignes_par_page, action)`: up/down keep the column;
  moving down from the second-to-last row, you go to the last image if the column does not
  exist on the last row;
- `scroll_to_show(layout, ligne, décalage, hauteur)`: minimal scrolling to make a row
  visible.

Scrolling is stored as an *intention* (`Reveal::Minimal` after a move, `Reveal::Top`
after a list change) and resolved when drawing the grid, when the width, hence the number
of columns, is known. Computing it earlier gave a wrong row at startup (bug seen on a
screenshot). A refilter (a filter slider, ratings, analyses coming in) while the selection
is out of view (scrolled with the wheel without moving it) keeps the **region shown**
(`Reveal::Anchor`, `TriApp::grid_anchor`): the first visible image, or the next one still
in the list, stays at the top, at the same offset (issue #100: the grid went back to the
start).

### Settings (`settings.rs`)

The whole configuration file as seen by the interface (engine sections + `[gui]`): see
[settings.md](settings.md#interface-guisrcsettingsrs).

### Textures (`textures.rs`)

Visible thumbnails are loaded from the disk cache (~1–2 ms each) within a budget of
**8 ms per displayed frame**: beyond that, loading resumes at the next frame, without
freezing the interface. Textures live in an **LRU cache of 600 entries**, indexed by path
(so it survives a filter change). Unbounded, 5,000 thumbnails of 400 px would take ~2 GB
of graphics memory.

## Development: automatic screenshot

```sh
TRI_GUI_SCREENSHOT=/tmp/aide.png TRI_GUI_SELECT=40 TRI_GUI_ACTIONS=core.app.help \
  cargo run --release -p tri-gui -- <dossier>
```

The application captures itself (its window content only, not the screen), then quits.
The capture happens once all thumbnails are computed and the visible ones displayed (in
full screen: the wanted image displayed, including the 100 % if requested), **and at
least 1 s after startup**, at the latest after 60 s. Without this delay, a capture taken
during a window's opening fade showed it half transparent.

- `TRI_GUI_SELECT=<n>`: image selected at startup;
- `TRI_GUI_ACTIONS=<étape>,<étape>...`: script run at startup. Steps: an action
  identifier (`core.nav.next`, `core.app.help`…), `wait:<ms>` (pause, to let the engine
  work), `shot` (immediate capture, without waiting for everything to be ready: to see an
  intermediate state), `rating_dialog` (right-click dialog for the selected image),
  `settings:<section>` (settings window opened on a section: `settings:keys`),
  `search:<text>` (typed in the search of the open settings window or help:
  `search:vim`). An unknown
  step is reported and ignored.
  Example: `core.view.open,core.view.zoom,wait:1500,core.nav.next,wait:30,shot`.
- **Crash log** (`gui/src/crash.rs`, 2026-10-19): any Rust error is recorded (message,
  file and line, thread, backtrace) in `crash.log` in the data folder, before stopping; a
  crash while drawing the window (called from AppKit on macOS) otherwise left only a
  system report, without the message.
- `TRI_GUI_TRACE=1`: timed trace of the startup steps (configuration read, window opened,
  folder read, tree, first image, engine; added on 2026-10-19 for a startup slowness seen
  on macOS) and of each step of image loading (request to the engine, image ready,
  dropped, reading and decoding, image displayed) on standard error. This is what made it
  possible to measure the delays above.

**Screenshots for the testers' manual** (`doc/manuel.md`, `doc/manual.md`):
`scripts/manual-screenshots.sh <dossier de photos> [--lang fr|en]` copies the photos to a
demo volume (disk image "Photos" on macOS: the folders panel only shows that one),
launches the interface with its own configuration, cache and data folder (nothing of the
user's is touched), chains the captures (grid, full screen, right click, comparison,
help), numbers the areas and converts to JPEG in `doc/manual-images/<langue>/`. The
sharpness judgement is disabled there: on a folder of varied subjects (close-ups on a
blurry background among landscapes), it wrongly flagged sharp photos.

## Touch-ups of 2026-10-21

- **Accent colour** (the theme's selection colour) for what is switched on: stars
  (filter bar, rating bar, thumbnails; `[gui.colors] stars` for another colour), ring of
  the chosen colour, [RAW] / [JPG] toggles (the rectangle only, no big square around it).
  In the light theme, stars and rings use the dark variant of the accent
  (`marks::accent_mark`): the pale blue was unreadable on white.
- **Double-click on a slider: default value**, everywhere (like Lightroom): blur and
  stacks back to the middle, stack time gap to `[similarity] max_gap_s`. Any new slider
  must do the same.
- **Blur slider**: all the way left, no photo is blurry (all visible; even at the most
  lenient some were missing); in full screen it no longer refilters the grid (the image
  being viewed left the filter and the image changed): refiltered on returning to the
  grid.
- **Progress bars** hidden for `[gui] progress_delay_ms` (400 ms) after a refilter: the
  cache almost always answers first, a bar shown for 1/10 s was unreadable. **A single
  bar** since 2026-10-25 (issue #36: with an offload in the background, they lined up at
  the bottom): background tasks (ratings, folder computations, export, copies and offload
  in the background, `TriApp::tasks`) gathered into one global bar (number of tasks,
  mean progress) at the bottom of the metadata panel, or in the status bar when it is
  hidden; a click unfolds the details, one bar per task above (click on a copy or an
  offload: its dialog; "×": interrupt the export). Folded (feedback of 2026-10-25), the
  global bar is a **thin grey bar** without text (`[gui] tasks_folded_height`, text on
  hover); it stays in place once unfolded, a second click folds. A single task: the same.
- "Camera JPEG" becomes **"Camera render"**, the counterpart of "RAW render".
- Thumbnails' **RAW badge** in the accent colour (the light blue was hard to read; on the
  selected cell, whose background is the accent, its text colour); thumbnails' stars in
  a lightened accent (`marks::chip_star_color`), readable on their dark chip. "Hide
  blurry" filter: a **triangle** (sharpness) instead of the crossed-out drop; likewise
  next to the sharpness map slider in full screen.
- **Thumbnails, afternoon** (few spelled-out words): under the thumbnail, the name
  **without extension** and the **format** on the right (RAW, JPEG, TIFF, PNG, HEIF…:
  `format_label`), blurry photo = **drop outline**, all in the cell's text colour; no
  more dark rectangle under the ratings: **stars bottom left, flag bottom right, colours
  top right**, each mark ringed in dark (`marks::thumbnail_notes`). **Clipping** moves to
  the top left: a right triangle ringed in dark, right half white (blown) or left half
  (blocked), `marks::clip_badge` (the magenta dot looked like a purple colour label, then
  a small histogram was unreadable); a click opens the large histogram. **Versions** move
  into the caption, left of the drop (crop icon and count, no background); a click starts
  cropping. Hand cursor over both; the name is shortened to the room left.
- **Thumbnail info can be hidden** (`Shift+T`, rating bar button): name, format, ratings
  and badges disappear, the image takes the text's room; `[gui.grid] info`, remembered.
- **Thumbnail size**: slider at the bottom next to ‹ › (logarithmic,
  `[gui.grid] cell_min` / `cell_max`, double-click: `cell`), "all on screen" button
  (`grid::fit_all`: the largest size that shows the whole filtered folder); on the
  keyboard, like Lightroom, `+` / `-` (a quarter more or less, `grid::step_cell`) and
  `*`; with the mouse, Ctrl / Cmd + wheel or a pinch on the touchpad, continuously
  (2026-10-24, issue #44). Size remembered between launches. Controls shown only if the rating bar has room
  (otherwise they covered the ratings).
- **Clear filters**: a crossed-out funnel at the far right of the filter bar, greyed out
  without a filter, the image count just to its left.
- **"Well exposed" and "invert" filters** (late afternoon): sun (`f w`) removing images
  blown or blocked beyond the badge thresholds (`tri::exposure::ClipFilter`; fractions
  read from the analysis cache for the whole folder, an image not yet analysed stays;
  grid refiltered when the histograms are done, and since 2026-10-25 as they are
  computed, at most every `[gui] analysis_refilter_ms` (5 s), sharpness too; turning the
  filter on during the analyses says so in the status bar, issue #73); ◐ (`f i`) shows what the **content**
  filters (ratings, sharpness, exposure) remove, file type and name search still
  applied (`tri::filter::invert`): "blurry only" is "sharp" inverted, the drop and `f B`
  are gone. Triangle and sun in the accent when on, grey otherwise.
- **"Blown" and "crushed" sliders** (2026-10-24, issue #48: photos on a black background
  were excluded): next to the sun, when the filter is on; same logic as the blur one
  (centre = badge thresholds, to the right stricter, double-click: the centre), one per
  side; fully to the left, that side no longer excludes anything. Thresholds multiplied
  or divided by `[gui.histogram] strictness_span` (`tri::exposure::clip_threshold`),
  remembered per folder; the badges follow. Since 2026-10-25 (user feedback: two sliders
  with grey labels were hard to read): **a single slider**, and on its left a switch with
  the clipping badges' icon (white triangle: blown, black: crushed) that chooses the side
  being set.
- **Crop**: presets on `Tab` / `Shift+Tab` (`p` was also the sharpness map, which stayed
  on; `p` turns it off during cropping); egui moved the focus to the bar there, given
  back at the end of the frame. Image name removed from the rating
  bar and, in the grid, from the status bar (redundant, several images can be selected).

## Tests

**Interface tests** with [egui_kittest](https://crates.io/crates/egui_kittest) (egui's
official test harness): the application runs without a window, on a temporary folder and
cache, and receives simulated keys. Checked: the help opens with F1 and `?` (with or
without Shift) and closes with F1, Esc closes it, it is modal, it displays the labels and
not the identifiers, arranged by theme; the search filters ("pnd"), "q" typed in the
search does not quit, and the search is empty on reopening. Full screen: opening with
Enter and returning with Esc, image-by-image navigation (up/down without effect), source
toggle, standard preview actually loaded and displayed, 1:1 actually computed then
displayed, kept on the next image, zoom kept when toggling the source. These tests found
a missing glyph ("⏴") in the help, and the lack of closing with Esc.

Full screen logic (`view.rs`, unit tests): source choice and fallback to what is ready,
labels, fit, placement at 100 %, panning limits, clicked point kept under the cursor.

The interface tests must be in the package (`src/`, `#[cfg(test)]`) and not in `tests/`:
`tri-gui` is a binary, its modules cannot be imported from `tests/`.

Rating with the mouse (2026-10-17): clicks in the bar (3 stars, reject, green, then
removal of stars and flag by clicking again), read back from disk; Shift+N hides the bar;
right click on the third thumbnail: it is selected, the dialog opens; the mouse moves over
the first thumbnail without changing the selection; "4 stars" and the 8 key rate the
third image; → does not navigate; Esc closes the dialog without leaving the grid.

Multiple selection (2026-10-17): see [selection.md](selection.md#tests).

Sharpness (2026-10-17): see [sharpness.md](sharpness.md#tests).

Dialogs (2026-10-17): the "Rename" dialog, grabbed by its title and dragged, follows the
pointer and stays open.

Comparison (2026-10-17): `c` refused with a single image (hint in the status bar); three
images in visual mode then `c`: the current image stays active, the arrows stay within
the comparison, the rating goes only to the active image, `Space` eliminates it
(comparison and selection), Esc goes back to the grid keeping the selection, Enter opens
full screen. Layout (`compare.rs`): two landscapes side by side in a wide window, one
under the other in a tall window, portraits, last row centred, bounded neighbours.
Engine: `tests/scheduler.rs` checks that all compared images are served, the active one
first, and that going back to full screen drops the others.

File actions (2026-10-17): `F2`, the typed name replaces the selected name ("q" does not
quit), the dialog announces the image and its pair renamed, Enter renames on disk and
keeps the image selected; a taken name is refused (nothing changes), Esc only closes the
dialog; `Delete` then Esc deletes nothing, `Backspace` then Enter deletes (permanently, in
the temporary folder) and selects the next one; the buttons of the right-click dialog
open the action for the clicked image.

Auto-advance (2026-10-17): disabled by default; with Shift+A, a rating moves to the next
image skipping the other image of the pair (also rated, read back from disk), a colour
too, the last image stays selected; under "without rejected", rejecting img0 leaves the
selection on img1 (checked: without the rule, the test fails on img2).

Offload (2026-10-17): fake card (two JPEGs in `DCIM`) declared as a volume, "offload"
icon, inventory ("2 to copy"), copy ("2 files copied and verified"), "Open the folder":
the session opens flat, history written. This test found a missing glyph ("→") in the
dialog.

Metadata (2026-10-17): the "Dimensions" row of the selected image appears after reading
in the background (64 × 48), `Shift+M` hides the panel.

Exposure (2026-10-17): a white image added to the folder gives a saturated share
> 99 % (badge); in full screen, histogram shown by default, `i` hides it, `Shift+I`
switches to linear, `w` shows the zones (mask as a texture). Unit tests (`histogram.rs`):
grouping into columns without losing pixels, placement in EV (white in the last column,
black in the first), mask colours, numbers in French format.

Filters (2026-10-17): `f Shift+2`, `f 0`, `f u` (twice: removed), `f 9`, `f °`,
`f Esc`; rejecting under "without rejected" selects the next image; bar hidden / shown by
Shift+F, clicks on the JPEG and RAW toggles, Clear.

Rating (2026-10-17): Shift+3, Shift+X, 8 then "°" (`Text` event) write the sidecar of the
selected image (read back from disk: 3 stars, rejected, purple only), the next image is
rated separately; a malformed sidecar is reported and left intact.

Package unit tests (`cargo test -p tri-gui`): unique and prefixed action identifiers,
rating keys with or without Shift, Ctrl+U / Ctrl+D (also in the Linux form, Ctrl +
`command`), `u` free, "°" character, reading of shortcuts from the file ("Maj+P", "G",
"g g", "Flèche gauche", "+", "°"...) and rereading of all default keys, replacement by
`[keys]`, conflicts and shadowed sequences reported, width of a rating row, complete
profiles without duplicates, union of profiles, system modifiers ignored, key names and
help labels, reversible identifiers; columns, navigation (edges, columns, incomplete last
row, pages, empty grid), scrolling; LRU (eviction, replacement, minimum capacity); name
truncation, status updates; settings (`settings.rs`): example file equal to the default
values, unknown sections and invalid values reported (theme, profile, colour), engine
section passed on, empty profile list. The rendering is checked by automatic screenshot
(also with a configuration file: `tri-gui <dossier> --config <fichier>`).
