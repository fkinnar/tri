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
  arranged in titled sections separated by a line (2026-10-20); titles in bold, larger than the
  folders since 2026-10-26, `[gui.folders] section_scale`, #120): devices (cards, keys,
  external drives, mounted or not: an unmounted one is greyed, a click or `o` mounts
  then opens it, right click "Mount" / "Eject", 2026-10-26, #113), fixed drives,
  network (with the remembered shares; "server" icon of the header or `c`: connect to a
  share by its address, `\\nas\photo` or `smb://nas/photo`, Shift+C or right click
  forgets it, 2026-10-28; a second click on the icon or the cross at the end of the field
  closes the search, 2026-11-05; see [servers.md](servers.md)), personal folders (with Desktop, Documents, Downloads, Pictures), favourites,
  then only the system (root, collapsed); **Subfolders** icon next to the title (2026-10-20,
  remembered; a checkbox until 2026-10-28, Refresh moved to the right edge): images of subfolders are shown too (`[gui.folders] subfolders_depth`
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
    `f f` next type, `f 1`…`f 5` at least n stars (or exactly, or at most: `f =` or the
    ≥ / = / ≤ button before the stars of the bar, 2026-10-26, #165), `f 0` any stars,
    `f p` picked,
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
  stays short. A click on a status bar message clears it, errors included (2026-10-26,
  #157: a red error stayed).
- **Focus point** (2026-10-17): marker of the autofocus point in full screen and in
  comparison (`m`), read from the RAW (Fujifilm for now); square, cross, circle or heart
  (`[gui.view] focus_marker`), colour `[gui.colors] focus`, dark outline underneath to
  stay visible on any background.
- **Position mark** (2026-11-04, user's idea: going through stacks, one got lost): on
  each image change, in the grid and in full screen, a small frame says where one is,
  "Photo 12 / 94", with stacks "Stack 3 / 20 · 2 / 5" (the stack, then the photo in the
  stack; a photo alone: "Stack 3 / 20"), and "Dev. 2 / 3" when reviewing the developments
  (`review_developments`), which count as one photo; in full screen, a click on the
  image or Enter shows it again (`Stacks::photo_number`, `stack_number`,
  `development_number`). `[gui.position] seconds` (2; 0: never; -1: always; fades out at
  the end) and `place` (`center`, `top-left`, `top-right`, `bottom-right`, `bottom-left`).
- **End of the stack or of the list** (`[gui.end]`, #307, 2026-11-05): in full screen,
  ← / → that no longer move say so for a moment, in the same frame as the position mark
  ("End of the stack", "Start of the stack", "End of the selection", "Start of the
  selection", the latter also at the end of the last stack; `TriApp::at_end`, `end_mark`),
  in the centre by default.
- **Notes on the image** (`[gui.notes]`, 2026-11-04, user feedback): in full screen, the
  image's notes (flag, stars, colours) in the same frame as the position mark
  (`mark_frame`, `mark_opacity`), on the same principle: display time after a change of
  image or notes (`seconds`, 2; 0: never; -1: always; click on the image or Enter: shown
  again) and place (`place`, `bottom-left`), bottom bar shown or not. At the same place as
  the position mark, the notes stay and the mark goes next to them (above at the bottom,
  below at the top and in the centre).
- **Filters and sort per folder** (2026-10-30, `state::FolderView`): each folder's
  filters (RAW / JPEG, stars and their comparison, colours, flags, sharpness, exposure,
  developed, inversion), sort order and, since 2026-10-31, the **thumbnail render**
  (`f d`: camera JPEG, cover, a software, LibRaw; `[gui.grid] render` is the value of a
  folder never set, not the previous folder's: one culls in camera JPEG and comes back to
  a processed folder in cover) are kept in `folder-views` in the data folder (one JSON
  line per folder, written when they change) and restored when it opens, at launch too;
  a software disabled or uninstalled since gives way to the cover, or to the camera JPEG
  without any software (2026-11-05); a folder with nothing kept opens without filters, in the order kept for all. The name
  search is not part of it. `[gui] remember_folder_view = false`: as before, the
  previous folder's filters stay. **In full screen too**, the filter bar (Shift+F) and the
  whole right panel (filters, grid): ratings are given in passes (1 star, filter, 2
  stars…) without going back to the grid.
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
  place its window. Maximizing when the window is created is ignored there (feedback of
  2026-10-26): it is asked again on the first frame if the window is not maximized.
- **Key hints** at the top right of the full screen view ("b : Camera render / RAW,
  z : 1:1…"): hidden by default since 2026-10-17 (it overlapped the image, F1 opens the
  full help with search); `[gui.view] key_hints = true` to bring it back.
- **1:1 from the grid** (2026-10-17): `z` (keyboard only since 2026-11-07: in the grid,
  the bar's "1:1" button gave way to the adjustable loupe) opens the selected
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
  already cached), and a **Luminance** row when it is chosen, like its curve (2026-10-28;
  on a RAW, computed per block of the mosaic, which is stated: see `exposure.md`).
  **Analysis tool** (user's idea, 2026-10-18): five dots at the bottom right of the large
  histogram (red, green, blue, luminance — a half-light disc, `l`, 2026-10-28, colour
  `[gui.colors] hist_luminance` —, white = all; `Shift+C` cycles through them and opens
  the large histogram) show only one channel, its single statistics row and its zones
  (luminance: those of all channels, the mask being per channel). To the
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
  2026-10-17): **movable** by dragging their **title** (across the dialog's whole width;
  at first by their background too, until the feedback of 2026-11-01: in the settings
  window, dragging a dot slightly off moved the window; a dialog without a title can
  still be grabbed anywhere), larger and more airy
  (`[gui.dialogs]`: margins, spacing, widths; title colour `[gui.colors] title`,
  underlined by a separator line).
  egui's `Modal` cannot be moved (its area also covers the greyed background): `Dialog`
  keeps the position and shifts it while dragging; texts in it are not selectable
  (otherwise dragging the title selected text). Buttons and fields keep priority.
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
- **Side-by-side comparison** (2026-10-17, `c`, `gui/src/compare.rs` for the layout;
  a bottom-bar button since 2026-11-07, lit in comparison where it leaves it, and the
  back-to-grid button in comparison too; **opened from full screen, it goes back to it**
  on the active image, "Back to full screen" button (#376); **without a selection or a stack, all the shown
  images**, at most `[gui.compare] max` around the current one; grid cursor hidden and
  nothing selected: nothing is highlighted, so everything, and `=` acts at once; when
  leaving, the grid comes back as it was, cursor hidden on the image before,
  `compare_from`):
  the selected images, or with no selection those of the current stack (2026-10-25,
  issue #52: `c` on a collapsed stack; Esc comes back to its cover; **2026-10-31: the
  burst stack if it is collapsed, else the photo and its developments** (camera JPEG,
  darktable, clones, RapidRAW copies; `TriApp::compare_stack`), else the burst stack; a
  burst compares each photo through the cover of its development stack, never the same
  photo twice; Esc on a hidden development shows it in place of its cover, like
  `Shift+W`; for a stack larger
  than the limit, `f c` shows the whole stack, expanded, in the grid (a single stack, not
  regrouped apart: #163), `f c` again or
  "Clear" gives everything back, and a stack that was collapsed collapses again; with
  stacks on, a stack button in the filter bar does the same, lit in the accent colour
  while looking at a stack (2026-11-06), greyed out outside a stack; with stacks on, a single photo is a one-image stack and compares
  too, 2026-10-26, #167) (at least 2, at most `[gui.compare] max`, 21 by default since 2026-11-06 (6 before): each keeps
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
- **"Comparison limited to N images"** (2026-11-07, the user's idea): beyond
  `[gui.compare] max`, a bubble says so, set like the position mark (`limit_seconds`,
  `limit_place`; `TriApp::compare_limit_mark`); time 0: the status bar says it.
- **Multiple selection** (2026-10-17, engine: `tri::selection`, see `selection.md`):
  `Space` marks, `v` visual mode (range following the cursor, `v` confirms, Esc cancels),
  `Ctrl+A` / `Ctrl+Shift+A`, Shift+click (range), Cmd/Ctrl+click (add / remove); Esc
  deselects before leaving full screen. Ratings, trash and editor apply to the visible
  selected images. Orange frame (`[gui.colors] marked`).
- **Developed in darktable** (2026-10-19, see `develop.md`; a whole folder in the background:
  `Ctrl+Shift+B` or the right click on a folder, 2026-10-25, issue #69; the folder itself
  opens in Darktable through the right click "Open in Darktable" or `d` in the folder
  panel, 2026-10-28): `Shift+B` or the aperture
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
  session. Exported image: what you are looking at (grid: the software of the developed
  render, otherwise LibRaw; full screen: the displayed source; a development, always by
  its own software), changeable in the dialog (`export::Render`,
  `TriApp::export_engine`).
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
  (`core.view.image_info`, `[gui.view] image_info`) hides or shows them, remembered;
  also the notes and the position mark drawn on the image. An "i" icon in the bottom bar,
  left of the 1:1, shows this state (in the accent colour when shown) and changes it with
  the mouse (2026-11-05).
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
- **Accent colour** (asked on 2026-11-04: it could not be set): `[gui.colors] accent`
  (dark and neutral grey themes) and `accent_light` (light theme), egui's by default.
  `Theme::apply` makes it egui's selection (`visuals.selection`); everything that "takes
  the accent" reads it there (stars, `focus_line = "accent"`, active buttons).
  The text on it is the accent much lighter (dark accent) or much darker (light accent):
  readable on a yellow. The colours that can follow the accent (`focus_line`, `stars`:
  `"accent"` or `"#rrggbb"`) get an "Accent colour" box in the settings window; unticked,
  the colour picker, starting from the current accent (feedback of 2026-11-05: the word
  "accent" to type in a field).
- **Search** (2026-10-19): `/` in the grid and in the folders panel (in full screen, `/`
  stays "fit"), Ctrl+F everywhere. **Grid**: field at the right of the filter bar, just
  before the counter (#241, 2026-10-31; after the filters before; `[gui.grid] search_width`; narrow window, issue #71: the sort, the thumbnail render and the
  number of images go into a "…" menu just before invert and clear, always visible, their
  lists as submenus (`bar_choice`, #181: a drop-down list opened from a menu closed with it), and the field shrinks
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
- **Development software settings hidden without it** (2026-11-04: most users have
  none): without darktable, RapidRAW, RawTherapee or ART, the settings window hides
  `[develop] default`, `[gui.grid] develop` and `develop_toast_ms`, `[gui.view]
  review_developments`, `[gui.colors] development_mark`, `[develop] high_quality` without
  darktable, and each `[sidecars.<software>]` without its software
  (`SettingsWindow::hidden`); the software paths stay, to point to one installed
  elsewhere. The View section of the panel then has no "Developments".
- **Development software in the settings** (2026-11-04, the user's idea: AppImages left
  behind after an uninstall kept the software "installed", with no way to see it): each
  software of `[develop]` has an **Auto / Manual / Disabled** list (`DeveloperMode`, value
  in the file: `"auto"`, a path or `"flatpak"`, `""`). Auto shows below where it was found
  (program, AppImage or Flatpak command; `Developers::found`) or "Not found"; Manual, a field pre-filled with the path found and "Browse…" (opened at the
  folder of the software found, otherwise at the root), "File not found" if the path is
  wrong; Disabled, "darktable disabled" (treated as absent: no renders or exports, settings
  and sidecars hidden). No software found: a message at the top of the section with the
  list of supported ones. Applied at once (2026-11-05; until then "on next launch"): a
  change of `[develop]` looks for the software again (`TriApp::find_developers`), for the
  interface, the engine and the window.
- **"Browse…"** (2026-11-04): every folder path field (`PathField`: copy / move to,
  export, offload, copy from a server, folders of the settings window, `folder` of export
  presets included) has a button opening the system's folder chooser (`rfd`), at the typed
  path or its nearest existing parent, otherwise at the home folder; Tab completion stays.
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
  button is lit only at 100 %. In the configuration, `steps`, `max`, `min` and
  `loupe_scale` are written in **percent** (2026-11-07: `max = 400`, easier to read than
  `4.0`), sorted when read; safeguard: refused outside 1 to 999 %, `max` below 100 % and
  `min` above (`ZOOM_PERCENT`, common-sense bounds, not settings; 999 %: typed entry takes three
  digits). In the settings window, the error under the value is the message alone
  (`tri::settings::parse_message`), without the file line. A typed number is only
  checked on Enter or when leaving the field (`update_while_editing(false)`): "800"
  went through "8", refused, and `max` could not be changed from the keyboard.
  Mouse and trackpad, **continuous**: pinch, or Ctrl / Cmd + wheel, zooms around the
  pointer; scrolling moves the zoomed image. Beyond fit, the 1:1 is what is displayed,
  reduced or enlarged; the scale is shown ("200 %"); it is kept from one image to the
  next, like the position, and also applies in comparison. **Loupe**: in fit view, while
  `loupe_key` is held (`t` by default since 2026-10-19: with Shift, every Shift+… shortcut
  made it appear; a free key or a modifier, a key already bound to an action is reported;
  `<` impossible, egui only receives its character), after `loupe_delay_ms` (no flicker
  on a short press), a square of `loupe_size` points under the pointer shows the image at
  `loupe_scale` (100 %); its 1:1 is loaded for the occasion, the enlarged preview shown
  while waiting ("loading 100 %…"). In **comparison** too since 2026-11-06: the image
  under the pointer (`loupe_index`), drawn over the others.
- **Back to the grid with the mouse** (issue #193): in full screen, a grid icon at the
  far right of the bottom bar (like Esc).
- **"Expand all" button** next to the similarity slider (`Shift+S`).
- **Development stacks** (2026-10-30, `Stacks::with_developments`, engine
  `tri::developments`, see `developments.md`): a photo and its developments (darktable
  and its clones, RapidRAW and its virtual copies) form a stack, **below** the burst
  stacks (an expanded burst stack shows its photos, each folded or not; a collapsed burst
  stack never shows a hidden development). Folded by default: one thumbnail, the
  **cover** (the development stepped to in place, else **according to the thumbnail
  render**, decided with the user on 2026-10-31: "camera JPEG" and "RAW render (LibRaw)",
  the photo itself, the grid no longer changes afterwards; "Cover", the preferred
  development, else the latest; a software, its main development; else the photo), and
  the developments **reel** (the
  "developed" filter's icon, in the cell's text colour, no background: feedback of
  2026-10-31, the coloured "N dev." badge caught the eye; the count is in its tooltip) at
  the top right, below the first colour dot, centred on it and of its size
  (`marks::development_mark_rect`, the dot's own geometry; a burst's image count is at
  the top left; hidden with the thumbnail info, Shift+T: #243); unfolded,
  each thumbnail in a thin frame of the same colour. The displayed name is the photo's,
  the chip next to the format says which one: "DT", "DT · 1", "RR · 2" (the number
  alone, feedback of 2026-10-31).
  `s` unfolds / folds (like a stack: harmonisation of 2026-10-31, see below), `Shift+W` /
  `Shift+Q` go to the next / previous development
  (wrapping; a folded stack shows it instead of its cover, in the grid as in full screen,
  ‹‹ ›› buttons of the bar); `Shift+Enter` on a development makes it the **preferred**
  one (cover; on the photo itself: no preferred one any more), **without toggling**
  (2026-10-31: doing it again removes nothing). Unfolded, each thumbnail carries a "fold"
  badge (`marks::fold`, 2026-10-31, user request: nothing folded with the mouse; the drawing alone, in the text colour): click =
  fold, **Alt+click** = fold and keep this development as the cover; `Shift+S` unfolds
  or folds every stack, bursts and developments. **Same keys for both kinds of stacks**
  (2026-10-31, user request: "the notions are close"): `s`, `(` `)`, `Shift+S`, `d)` `y)`
  and the stack mode (`Ctrl+S`, ← →) target the burst when stacks are on and it holds
  several photos, else the photo's developments (`TriApp::applicable_stack`, the
  `Shift+Enter` rule). `((` opens the next level (the folded burst, else the
  developments) and `))` closes the innermost one (user's idea: parentheses nest like
  stacks; never `[` `]` nor `{` `}`, Alt+Shift on macOS). `core.dev.toggle` and
  `core.dev.fold_all` keep no default key;
  **Two-level deletion** (2026-10-30): `x` / Delete on a development removes only it (a
  clone, a copy or the main RapidRAW one: its file to the trash; the main darktable one:
  its XMP rewritten without the history, notes kept, see `files.md`), the photo stays;
  **Ctrl+Delete** / `dd` (`core.file.delete_photo`) deletes the photo with all its
  developments, even from a development; `Shift+Delete` / `d)` the whole stack. Same
  confirmation dialog; several developments chosen: one ticked checkbox per development,
  untick to keep one. Without developments, Delete and Ctrl+Delete do the same thing.
  `preferred-developments.txt` of the data). In full screen, a development shows **its
  render** (requested by itself when missing, the camera JPEG meanwhile, labelled
  "· clone 1" and "before (… pending)"); `b` toggles before / after there (the camera
  JPEG instead) without touching the photos' source. In the grid, a development's
  thumbnail is always its render, whatever the render chosen for the grid, computed in
  the background even with `[gui.grid] develop = "manual"` (2026-10-31,
  `grid_render::to_render`: developments always, each in its software; photos only with
  `"auto"`, in the software of the chosen render). The status bar counts the photos
  and, apart, the developments ("12 images (10 RAW, 3 dev.)").
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
  (`group-thresholds.tsv` in the data folder; the default: `[similarity] threshold`). Since 2026-11-06 (#306), also **"Slider progression"** (gentler
  to the left above 1; fully to the left, everything is grouped) and **"Weight of pauses"** (to the left the
  content only, to the right pauses cut more), remembered per folder, even when closed
  with the keyboard (see `similarity.md`).
  Second feedback of 2026-10-18: **a single toggle** (`Ctrl+S`, stack button in the
  filter bar and the rating bar, icon in the accent colour when active, always visible; right
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
  2026-10-22; the help writes it `<leader> h`, #175, since it can be changed) (folders,
  images, metadata; a hidden panel
  is shown, folders only exist in the grid). The active panel has a slightly lighter background (darker in the light theme, `[gui.dialogs] focus_panel_tint`; an accent outline until 2026-10-25, issue #78: with the accent cursor, the double frame was heavy; the metadata cursor no longer frames the histogram, issue #77) and a **line at the top** (2026-10-26, #77: the background alone did not show enough; `focus_line` = `"accent"` or `"#rrggbb"`, `focus_line_width`, 0 for none); the line under the keyboard cursor (folders, metadata, search results) is a **discreet grey band** across the whole width (`[gui.dialogs] cursor_tint`, `cursor_row`; an accent frame before 2026-10-25, too loud and cut on the left when the panel scrolled). **Hovering points** (2026-11-09, #383, #385): the mouse moving over a row of the folders or the EXIF puts the band there (a single band, the keyboard carries on from it); in a panel without the keyboard, the band follows the mouse. The open folder keeps its accent background, separately; no more egui background or frame around the hovered name. Everywhere, a hovered control no longer has a white frame, only its light grey background, like the bar icons (`Theme::apply`); the
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
  favourite; offload a card here, or on a removable drive: offload, eject; show in the
  Finder / Explorer, 2026-11-08), shortcut on hover.
  **Right panel** (2026-10-27, `TriApp::side_begin` / `side_end`): ↑ ↓ / `j` `k` also go
  through the **settings** above the EXIF (section titles included), one at a time;
  **Space** / Enter use the pointed setting (check, choose, fold a section, slider to the
  centre, button); ← → / `h` `l` vary it (slider by a step, `[gui.metadata] slider_step`;
  lists to the next choice; title: fold /
  unfold); **Tab** / Shift+Tab (`)` / `(` in vim) go from one section title to the next,
  looping. The same keys go from section to section in the **folder panel** (first row of
  the section; in the middle of a section, Shift+Tab goes back to its start). Tab no longer
  moves egui's focus in a panel that has the keyboard (as in cropping).
  EXIF rows: each row (sharpness included) can be pointed at, Enter opens the histogram
  large; Shift + ↑ ↓ / Shift + `j` `k`
  (or Shift+click) extend the selection, `y` copies it (one row: the value; several:
  "label: value" per row; `Ctrl+C` / `Cmd+C` too), Shift+`y` copies the whole panel; a
  click on a row points at it and gives the keyboard to the panel. **Without a click**
  (#383): the panel's copy shortcut (`y`, `Ctrl+C` / `Cmd+C`, Shift+`y`) copies the row
  under the mouse even when the panel does not have the keyboard
  (`Keymap::panel_action`); elsewhere, the key keeps its effect (`Ctrl+C`: copy the
  image to…). Large histogram open:
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
  with the arrows, progress). Also through icon buttons in the right-click dialog
  (pencil, two sheets, arrow into a folder, trash can, folder, outgoing arrow, and since
  2026-10-31 a sidecar sheet with a dot: generate the rating sidecars, `sidecars.md`;
  label and shortcut on hover; `[gui.dialogs] icon_size`). Rename: the name (without extension) is
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
  stack to stack (↑ / ↓), as when entering a folded stack from the grid (Enter,
  double click; 2026-10-26), we land on its cover or, with `[gui.view] stack_enter =
  "first"`, on its first photo: "|‹" button of the full screen bar, next to the stacks
  switch (always there since 2026-10-25, in the grid as in full screen: it left the
  filter bar with its reorganisation of 2026-10-27 and came back to the bottom bar on
  2026-10-31; "|‹" in the grid too, with stacks on, to see its state, since
  2026-10-26), or Ctrl+Shift+S, remembered from one launch
  to the next (issue #62); a message says which one is on (nothing moves on screen).
  Sidecar write failure: we stay on the image (the error is in the status bar).
- **Rating with the mouse** (2026-10-17, requested by the user: not everyone wants to do
  everything with the keyboard):
  - **rating bar** at the bottom (above the status bar), in the grid and in full screen:
    flags, stars and colours of the selected image, clickable, faded when not set, their
    shortcuts in the tooltip (2026-10-22);
    `Shift+N` shows or hides it (`[gui] rating_bar`); on the right, the "auto-advance"
    button (see below). In full screen, the ratings are also drawn on the image, for a
    few seconds (`[gui.notes]`, see above; issue #8: they stayed there for good, twice). In full screen too, after the
    ratings: the grid's badges, blown / blocked (click: large histogram) and blurry
    (click: sharpness map; issue #9);
  - **click on a thumbnail mark** (#257, 2026-10-31): the stars (bottom left, all five
    slots even empty), the flag (bottom right, neutral ring included) or the colours (top
    right) open a **small picker** of that group, right there (`mark_picker`,
    `marks::thumbnail_mark_zones`, the bar's controls split into `flag_controls` /
    `star_controls` / `color_controls`); a star or a flag closes it, colours stay open
    (several to tick), Escape closes; hand cursor over the marks;
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
- **Right panel in sections** (reorganisation of 2026-10-27, the user's idea;
  `TriApp::side_panel`): the filter bar keeps only what is set with a click (stars, flags,
  sharp / well exposed / developed switches, colours) and the name search, with the image
  count, invert and clear; the **settings** move to the right panel, in titled sections
  like the folder panel's (`section_title`: capitals, bold, accent colour,
  `[gui.folders] section_scale`, a `section_gap` above), folded by a click on the title, in
  the usual order of other software (Lightroom): **Histogram** on top, **Filters** and
  **Grid** (in the grid only), **EXIF** at the bottom. The **folder panel**'s sections fold
  the same way (feedback of 2026-10-27, `Tree::set_folded`: a folded section keeps its
  title, its rows leave `Tree::rows`, so the keyboard cursor too). Folded sections of both
  panels are remembered across sessions (`gui-state.toml`, `panel_folded`: `grid`, `exif`,
  `folders-personal`…).
  - *Filters*: first the files shown, RAW and JPEG (two check boxes without a title, one
    stays checked; in the filter bar until 2026-10-27, then a day in the Grid section;
    feedback of 2026-10-28), then each adjustable filter with a check box (like its bar icon), its settings
    only while it is on: blur strictness; **two sliders** for exposure (blown and crushed,
    instead of one slider and a switch); development software as **flat check boxes**.
    Under each slider, "more lenient" on the left, "stricter" on the right. The tooltip of
    the bar icons says where the settings are.
  - *Grid*: sort (list and direction; at the top of the section, feedback of
    2026-10-27; titled "Order" until 2026-10-28), thumbnail render as a drop-down list (#246, 2026-10-31; radio
    buttons before, instead of the list hidden in the "…" menu of narrow windows, which is gone), stacks (check box,
    similarity, unfold / fold all, fine settings). Staying in the bottom bar (the user's
    choice, 2026-10-27, no duplicates): thumbnail size and info, entering a stack by the
    cover or the first photo.
  - *View* (full screen only, in place of *Grid*: #239, 2026-10-31): the **shown source**
    as a drop-down list (camera render, RAW render, each development already rendered;
    on a development, a "before" check box: `b`), the stacks switch (as in *Grid*;
    feedback of 2026-10-31) and the file actions switch. The
    bottom bar's development buttons (‹‹ ›› : previous / next development of the photo)
    are symmetric (#240).
  - **"RATING"** (bottom bar), **"FILTER"** (top bar) and **"FOLDERS"** (left panel)
    labels, "FILTER" singular like the panel's "FILTER" title: capitals a bit smaller than
    the text, in the accent colour (`bar_label`, `[gui.grid] bar_label_scale`; feedback of
    2026-10-27).
  - After a filter change, the **number of images** shows a moment in the middle of the
    images area (`TriApp::filter_toast`, `[gui.grid] filter_toast_ms`, 0: never; the
    user's idea, 2026-10-27: "JPEG" alone in a folder of RAWs left nothing without it
    being noticed). Not for the name search, whose field already shows the number, nor
    when a folder opens.
  - Keys removed: `[gui.grid] stack_slider` and `clip_slider` (the panel
    sliders take its width); `blur_slider` is now only for the full screen bottom bar.
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
  - **camera JPEG as a thin grey line** (2026-10-27, the user's idea): for a RAW, the curve of
    the camera JPEG (all channels, or the chosen one) over the sensor's, at its own
    height; it shows what the camera did with the highlights and shadows. In EV, each
    below its own white (sensor saturation, the JPEG's 255), over the RAW's range.
    Computed from the cached camera preview (`preview::camera_exposure`, a few tens of ms,
    kept in memory) once it is ready. A discreet grey (2026-10-28: in white it drew the
    eye). Legend in the large histogram: a "Camera JPEG" row under Red, Green, Blue, with
    its figures below its own white (the chosen channel, otherwise the luminance); tooltip of the
    small one. A button (a camera, left of the analysis photo's) and `j` in the large
    histogram show or hide it, remembered between sessions; `[gui.histogram]
    camera_jpeg` (value at the first launch), `camera_width`, colour `[gui.colors]
    hist_camera`;
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
- **Help** (`F1`, closed by Esc or `F1`; `?` shows the "help here", below): each action with an understandable label
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
  and an m happen to be. Revised on 2026-11-06 (help redesign): the help searches the
  labels and keywords of **every language** (`Action::search_texts`: "crop",
  "peaking", "stack" in a French interface), with **keywords per theme**
  (`kw-group-<theme>`: "noter" finds the stars); a **typo** is forgiven in a long
  enough word ("histrogramme", see `fuzzy.md`); an action found in its **label** comes
  before those that only have the word in their keywords ("darktable"). Feedback of the
  same day ("ctrl f" brought up unrelated actions): a query that reads as a **key with
  modifiers** ("ctrl f", "Ctrl+F", "shift p") only shows the actions bound to that key
  alone, with at least those modifiers (`Keymap::actions_for_typed_keys`; help and
  palette); and a **single letter** is not a word: it only finds a single letter (a key,
  "x", or a one-letter word), no longer the "f" inside "filter".
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

Clipboard shortcuts do not arrive as keys: egui-winit replaces them with "cut", "copy",
"paste" (Ctrl / Cmd + X, C, V, and on Windows Shift+Delete, Ctrl+Insert, Shift+Insert).
`Keymap::chords_repeated` translates them back into keys from the modifiers held (issue
#194: Shift+Delete did not delete the stack on Windows, Ctrl+C / Ctrl+X neither copied nor
moved).

Two shortcut sets are shipped, **to choose from** (`[gui] profile`: `"classic"` by
default, `"vim"`, or `"both"` for both together, which do not overlap; drop-down list in
the settings window). Decided on 2026-10-18: a user of classic tools has no use for vim
keys, and vice versa; what is the same for both is written only once, in the common lists
in the common section, what is specific to each set in its own. A test checks that each
set covers all actions, without conflict.

**Default shortcuts file** (2026-11-06, issue #280: no shortcut left in the code):
`gui/resources/keys.toml`: the panel prefix (`leader = "Ctrl+b"`, at the top; `[gui] leader`
replaces it for oneself, empty by default), then sections `[common]`, `[classic]`, `[vim]`
and `[after_leader]` (keys after the panel prefix), in the syntax of `[keys]`, the sections adding up. **Every
action is listed**, `[]` for those without a shortcut (the user's choice: a file to read
at a glance, and complete for whoever writes their own); a test checks it, so every new
action is added there too. It is **shipped next to the program** and read at startup
(`actions::Defaults`, `resource_file`): next to the executable (Windows), `../Resources`
(macOS), `../share/tri` (Linux); changing the default shortcuts no longer needs a
rebuild. A copy is built into the program (`include_str!`), used when the file is
missing (run from `cargo`) or unreadable (reported in the status bar, with the faulty
entries: unknown action or key). Generated on 2026-11-06 from the former Rust tables,
checked to give exactly the same shortcuts; comments in English (shipped file). For
their own shortcuts, users keep `[keys]` (a shipped file is replaced on every update).
The help window scrolls with the profile's navigation keys that type no text (arrows,
pages, Ctrl+U / Ctrl+D). Left in the code are **widget keys**, interface conventions
rather than shortcuts: Enter validates a field, Tab completes a path, Up / Down move in
a suggestion list; and the loupe key, set separately (`[gui.zoom] loupe_key`). The help and the texts that mention a key read it from the table in
effect: they follow the chosen set. For an action that has keys of its own in the chosen
set (`[vim]` or `[classic]`, or `[keys]`), they show only those (2026-11-06: in vim, "x"
deletes, without Delete or Backspace, common and still active; easier to read);
otherwise, the common ones (`Keymap::keys_for`). Arrows and Enter are written
←, →, ↑, ↓, ↵ (Space stays spelled out: ␣ did not read well) when a system font has them (`fonts.rs`: Apple Symbols, DejaVu Sans,
Segoe UI Symbol, added as a fallback to egui's, which has none; nothing bundled;
each font has its scale, its arrows at the text size: Apple Symbols 1.4, DejaVu Sans 1,
Segoe UI Symbol 1.1; `[gui] arrow_font`, and `arrow_font_scale` on top); otherwise spelled out. Tests do not install
it: their texts stay stable. An action bound to an arrow and to an
`hjkl` letter (after the leader, common to both sets) only shows the chosen set's
variant, in the help as in the menu of continuations (`Keymap::other_set_variant`). Special case of the search: in the
grid, "fit" (`/`) starts it (`TriApp::perform`), so the help writes `/` for it, alone in
vim (as in vim), with `Ctrl+f` in classic. The search field is only shown in the grid
(feedback of 2026-11-06: in the enlarged view it is useless, and `/` there goes back to
"fit").

### Menu of continuations (2026-11-06)

The user's idea (help redesign: a newcomer should easily find what to do), like
**which-key** in nvim. A started sequence (`<leader>`, `f`, `<leader> r`, `g`…) left
without a next key for `[gui] key_menu_delay_ms` (400 ms) shows the possible keys and
what they do at the bottom of the screen, in columns; a known sequence typed in one go
does not show it. While the menu is shown, the sequence **no longer expires**
(`Sequencer::hold`): there is time to read; a sub-menu follows at once (`<leader> r`).
**Esc** abandons (`Sequencer::cancel`) without going back, unless it is itself a
continuation (`f Esc` clears the filters). No menu after an ambiguous key (`y` before
`y )`): it acts alone after a short delay. `[gui] key_menu = false` turns it off.

Everything comes from the keymap in force (`Keymap::continuations`): `keys.toml`,
`[keys]`, the chosen set, and the panel that has the keyboard, ranked like `lookup`
(`<leader> r d` develops from the images, opens the folder from the folders). Several
keys for the same thing share a line (`Shift+h / Shift+Left`). A sub-menu is named by
the `[menus]` section of `keys.toml` (`"<leader> i" = "histogram"`: message
`key-submenu-histogram` of the catalogues, otherwise the text as is), failing that by
the most frequent theme of its actions. Screenshot: step `keys:Ctrl+b r` of
`TRI_GUI_ACTIONS`.

### "Help here" (`?`, 2026-11-06)

Second part of the help redesign (the user's idea: a newcomer should easily find what to
do). `?` (`core.app.help_here`) shows over the darkened screen **only the shortcuts of
where you are**, arranged by theme in cards in columns, in large type
(`[gui.dialogs] help_here_text`, `help_here_column`); `F1` remains the full list with a
search. The place (`cheatsheet::Place`): crop, large histogram, folders or metadata panel
if it has the keyboard, otherwise the mode of the images (grid, full screen, comparison).
`cheatsheet::applies` says what applies where: the panel's actions; for the images,
except what only applies to an image shown large (zoom, sharpness map, overlays,
source…) in the grid, crop and versions outside full screen, what only applies to the
grid (thumbnail render, pages, sort…) elsewhere. Actions without a key are left out
(they stay in F1). Modal: `?` or Esc close, `F1` switches to the full list, a click
closes; arrows and pages scroll. At the bottom, a reminder of F1 and of the leader in
force.

### Command palette (2026-11-06)

**Controls as reusable blocks** (proposal of 2026-11-07, the user's idea: "a palette
result should be a block strictly identical to the matching block of the interface"):
`app/controls.rs`. Each control of the right panel is a `Control` (RAW / JPEG boxes,
sharpness / exposure / developed filters with their sliders and boxes, order and
direction, shown render, stacks, review of the developments, file actions), drawn and
applied by one function, `TriApp::control(ui, control, host)`. The panel calls it
(`Host::Panel`: reachable from the keyboard through `side_begin` / `side_end`); so does
the palette, under the row of an action that has a control (`Control::of_action`,
`palette_control`: once per control; `Host::Palette`: outside the panel's keyboard
pointing, `side_detached`). One sees the state and changes it without closing the
palette; the field takes the keyboard back after a click (unless a drop-down is open).
Changing a control changes it in both places.
**From the keyboard** (feedback of 2026-11-07): ↑ / ↓ also go through the items of the
controls shown (`PaletteState::stops`: each row, then the items of its control, taken
from the previous frame); on an item, ← / → change it (previous / next choice, slider by
one step) and Enter activates it, without closing the palette (`PaletteItems`: the same
pointing as the panel, `side_begin` / `side_end`, numbers from `PALETTE_ITEM`). On a
row, ← / → stay with the field. No controls in the history (empty field): one
relaunches there, ↓ goes to the next command. A control without an action of its own
has **its own row**, by its name (`palette::Item::Control`, `ControlRef`, `Control::id`):
"Thumbnail size" in the grid (feedback of 2026-11-07: under "Zoom: next step up" it made
no sense); found only at the start of a word (`tri::fuzzy::starts_words`); Enter gives
it the keyboard.
**Search** (same feedback: "nettes", "taille des vignettes" not found): the labels of
the controls that do an action (`interface_labels`: "Nettes", "Taille des vignettes"…)
count as its name in the ranking; with equal names, an action comes before a setting,
which only opens the configuration.
The stacks include, in the grid and when on, the similarity and the "Unfold all" /
"Fine settings…" buttons (`stacks_settings`). Still outside the system (to convert if
the proposal is kept): the bottom-bar tools (scale control, strictness slider), the filter-bar switches; the sub-headings stay
in the panel (layout). The settings window is of another kind (default values, not the
state): its "configuration" rows in the palette still open the window.

**History** (2026-11-07, the user's choice): with an empty field, the palette shows what
was launched from it, most recent at the top ("Recently launched"); ↑ / ↓ and Enter launch
it again, typing searches, clearing comes back to it. Only what is launched from the
palette (actions and settings opened), without duplicates, at most
`[gui.dialogs] palette_history` (20; 0: none). Kept from one session to the next in the
data folder (`palette-history`, one line per command: `action<TAB>id`,
`setting<TAB>section<TAB>key`; a removed action or an unreadable line is skipped).
`palette::remember`, `history_hits`.

An idea from the help redesign ("too many results to be really useful"): like
Ctrl+Shift+P in VS Code, or the command line of vim. `core.app.palette`: `Ctrl+Shift+P`
in classic, `:` in vim (the user's choice; `:` joins the characters bound for
themselves, whatever the layout). Type a few letters; below, the **actions** of where
you are (`cheatsheet::applies`, without the palette) and **every setting**
(`settings_window::settings_list`, without those the window hides), the best line chosen
(`palette::search`, the same search as the help: every language, typos, keywords; on a
tie, an action before a setting, then the shortest title). Up / Down arrows, Tab,
Ctrl+N / Ctrl+P change line; **Enter runs** the action, or opens the settings window
filtered on the setting (`section.key`); a click too; Esc closes. Each action shows its
keys: shortcuts are learnt along the way, and actions without a shortcut become reachable
from the keyboard. Settings `[gui.dialogs] palette_width`, `palette_max`. Maybe later:
toggle a yes / no right in the palette. Feedback of the same day: a title, a roomier
field, and actions that were missing to be found there: **Sort by…**
(`core.filter.sort_by.<key>`, one per order, without a default shortcut). Then
**prefixes** (the user's idea): `>` only searches the actions (as in VS Code), `?` only
the settings; alone, the prefix lists everything, in the order of the help or of the
file, to browse (`palette::split_prefix`, `Only`; `[gui.dialogs] palette_actions_prefix`
and `palette_settings_prefix`, one character, empty: none). The field's grey hint
recalls them. The palette is drawn in the common dialog frame (`modal_at`, `Dialog`:
colours, margins, title and cross like the others).
The user's first searches that found nothing, each now a test
(`palette::tests::what_the_user_looked_for_is_found`): the label of a panel checkbox
("Empiler automatiquement": `Action::interface_labels`, searched in every language),
the plural ("rendus vignette": `fuzzy` ignores a final "s" or "x"), the RAW / JPEG
switches (actions `core.filter.kind.all|raw|flat`). In the palette as in the help, a
line found by **words** of its title (not by scattered letters) comes first. An action
**greyed out** here ("Developed" without a developed photo in the folder:
`TriApp::action_unusable`) comes after the others in the palette and is refused when
run, with the reason in the status bar. The **loupe** (a held key, set separately) has its
help entry, `Action::Loupe` (`core.view.loupe`), with its key in force; it cannot be
bound (`Action::bindable`): absent from `keys.toml`, `[keys]` and the shortcuts page; run
from the palette, it recalls its key. Each panel checkbox has its action, for the palette
(feedback of the same evening: "passer en revue" only found the default value):
`core.view.review_developments` and `core.file.allow_actions`, without a shortcut, which
toggle the current state; settings lines are called "configuration" there. The palette offers
**everything, unfiltered** (feedback of the same evening: from a side panel, "passer" did
not find the review), with no line limit by default; on equal match, what applies here
(`TriApp::palette_places`: the panel with the keyboard and the images) comes first.

### One shown render (2026-11-06)

**A precise render, directly** (2026-11-07, asked by the user): one action per render,
`core.view.render.<id>` (`embedded`, `cover`, `darktable`, `rapidraw`, `rawtherapee`,
`art`, `libraw`; `Action::Render`), like `SortBy` for sorting. No default key: the
palette finds them ("render darktable"), and `[keys]` can give them one. A missing or
disabled software, or the cover without any software: refused with the reason
(`action_unusable`), greyed out in the palette.

Discussed with the user (the full-screen View section had been confusing since
2026-11-05): "Shown source" and "Development of each photo" become **one list, "Shown
render"**, the same in the grid and full screen: camera JPEG, cover (preferred
development, else the latest), each installed software, RAW render (LibRaw, a
development like any other: without any software, the list is "camera JPEG · RAW
render"). Each photo shows in it, otherwise in its RAW render, like its thumbnail; a flat
image stays itself. **`r` / `Shift+R`**: next / previous render, for the whole folder,
kept from photo to photo (`TriApp::change_render`). Full screen follows through
`sync_source` (render change, opening, folder reopened); the software shown is the
thumbnail's (`developed_level`, grid record cleared on every change, otherwise computed
the same way). On a development under review, `r` stays before / after (camera JPEG).
"Review the developments" stays separate (navigation), greyed out in camera JPEG. Gone:
the per-photo rotation (`Source::next`), the "Development of each photo" list.

### Loupe from the keyboard (2026-11-06)

The user's idea: with the loupe key held, the navigation keys of the chosen set (arrows,
`hjkl` in vim) move the loupe by `[gui.zoom] loupe_step` points, the scrolling keys of a
zoomed image (with Shift) by `loupe_step_large`, **measured in the loupe** (the image at
100 %, `loupe_sizes`; first measured on the shown image, they jumped hundreds of pixels
on a thumbnail; a first press before the loupe is drawn only places it), instead of changing image
(`TriApp::loupe_key_move`, before the other actions). Start: under the pointer if it is
over the image, otherwise at the centre; the aimed point is relative to the image
(`loupe_point`), in the frame recorded when drawing (`loupe_rects`, one entry per image in
comparison). Moving the mouse, or releasing the key, gives the loupe back to the pointer.
**In the grid** too (asked the same evening): each thumbnail records where it is drawn
(`grid_images`); `grid_loupe` puts the loupe on the one under the pointer, otherwise
(keyboard) on the selected one, drawn in the foreground and clipped to the grid; its 1:1
is requested as in full screen (`displayed`: the loupe's image), the thumbnail enlarged
meanwhile.

### Loupe magnification from the keyboard (2026-11-06)

The user's idea. `Ctrl+K` / `Ctrl+J` (`core.view.loupe_zoom_in` / `_out`) change the
magnification by one step (`[gui.zoom] steps`, within `min` / `max`), with the loupe held
or for next time; with the loupe held, `+` / `-` too. `<leader> m`
(`core.view.loupe_scale`) opens an entry without Enter, like a brush's opacity in
Photoshop (`TriApp::loupe_entry_keys`): digits go to the number (not to the stars), three
apply at once, one or two after `loupe_digits_ms` as hundreds or tens
(`loupe_entry_percent`: `2` and `20` = 200 %, `01` = 10 %); a digit typed within the same
time afterwards still completes the number (`loupe_entry_done`: with Shift on an AZERTY
keyboard one types slower, and the next digit changed the rating); 0 is ignored, Esc cancels,
another key applies what was typed then acts. For the session (`TriApp::loupe_scale`);
`loupe_scale` stays the starting value. The wheel, with the loupe held, changes the magnification one step
per notch (`loupe_wheel_step` points; `TriApp::loupe_wheel_input` removes the scrolling:
the grid stays in place). The value shows in the bottom bar (scale control, below).

### Zoom like the loupe (2026-11-07)

**A development at the scale of its photo** (same day): darktable and RapidRAW render at
the preview size; at "100 %" of its own pixels, a development looked ~2.4 times smaller
than the photo next to it in comparison. Zoomed, it takes the size of the photo's 100 %
(`view::developed_zoom_size`, `TriApp::developed_full_size`): its real resolution when
known (darktable: crop read in the sidecar), otherwise the long side of the sensor at the
proportion of the render (exact without a crop; approximate for a crop made in RapidRAW,
RawTherapee or ART, which is not read). Enlarged this way, it carries a small info icon
at the top right (`resampled_mark`, full screen and comparison): on hover, the size of
the render and of the photo, and that its sharpness at this scale is not the file's.

Decided with the user: what applies to the loupe applies to the full-screen zoom.
`<leader> z` (`core.view.zoom_scale`) opens the same entry as `<leader> m`
(`entry_for_zoom`, `TriApp::set_zoom_scale`, centred like `+` / `-`). `Ctrl+K` / `Ctrl+J`
set the loupe if its key is held, otherwise the zoom (one pair of keys, the held key
chooses).

**Back to 100 % and to fit** (the user's idea, 2026-11-07): `<leader> m m` sets the loupe
back to `loupe_scale` (100 %), `<leader> z z` the zoom to 100 %, `<leader> z /` fits it.
Not sequences of the sequencer: `<leader> m` would have become ambiguous (it would wait
for a second `m`, and a digit typed meanwhile would open the entry *and* rate the photo).
The entry recognises, before any digit, the key that opened it (`entry_trigger`, read
with the action; none from the palette); `/` closes the empty entry then acts as always
(fit). In the menu of continuations, `m` and `z` are sub-menus ("+Loupe…", "+Zoom…");
the open entry, still empty, has its own (`TriApp::entry_menu`, same drawing:
`key_menu_area`): the digits, the key retyped (and `*` for the zoom) for 100 %, `/` for
fit; after the menu delay, at once if it was already shown.

**One scale control in the bottom bar** (feedback of 2026-11-07: two controls side by
side, zoom and loupe, were unclear; `TriApp::scale_control`): an icon, −, the value, +,
left of the 1:1 button. It sets the **zoom** (round magnifier, `marks::loupe_icon`), or
the **loupe** while its key is held (square magnifier, `marks::square_loupe_icon`; a
linen tester, tried, could not be read at that size). In the grid, without zoom, it
always sets the loupe, and only when there is room. A click on the **icon** resets (zoom
fitted to the view, loupe to `loupe_scale`); a click on the **value** lets one type it as
a percentage (`ScaleEdit`: Enter applies, Esc or a click elsewhere cancels; the target is
fixed at the click, the loupe key can be released; the field takes the whole keyboard).
Holding the loupe key no longer hides the pointer (its repeat hid it, even over the bar);
a hand over the icon, − and +. The loupe only shows over the visible part of an image
(`painter.clip_rect()`): a thumbnail cut by the bottom bar went on beneath it, and the
loupe stayed while clicking − and +. The image info switch (`Ctrl+I`) is at the far
right of the bar.

### Shortcut review (2026-11-06, issue #280)

Done with the user, once the shortcuts were out of the code (`keys.toml`). The keys
mentioned elsewhere in this document are **those of their date**: the reference is
`gui/resources/keys.toml` and the help (F1). The rule (at the top of `keys.toml`):

- **one letter per domain**, from its **English** name: `i` histogram, `m`
  magnify (loupe), `n` order, `o` overlays, `p` peaking, `r` render, `s` stacks, `t`
  text (infos), `v` versions, `a` AF point, `b` background, `e` external, `w` warnings;
- **the letter alone**: the main action; **Shift+letter**: its variant;
  **Ctrl+letter**: the settings window of the domain;
- **a secondary setting** (changed now and then): `<leader>`, the letter of the domain,
  then the setting (`<leader> i s`: histogram scale); a leader sequence reaches the
  actions of every panel (`Keymap::lookup`: focused panel, then general, then another
  panel);
- except for **everybody's reflexes**: `Ctrl+Z`, `Ctrl+C`, `Ctrl+X`, `Ctrl+A`,
  `Ctrl+F`, `Ctrl+E`, `Ctrl+,`.

Other decisions: the **"both" profile is removed** (read as `classic`, and reported):
classic and vim can thus bind a key differently (folder panel: Finder / Explorer
conventions in classic, nvim-tree in vim). The **loupe** joins `keys.toml` (`loupe =
"m"`, `[gui.zoom] loupe_key` empty by default). **Strict**: a leader or loupe key
already bound to an action is **refused** (the one of `keys.toml` stays, and it is
said); a leader on Space first requires moving the selection in `[keys]`.

| Action | Before | After |
|---|---|---|
| Stacks on / off | `Ctrl+S` | `s` |
| Grouping window | `Ctrl+G` | `Ctrl+S` |
| Expand / collapse the current stack | `s` | `( (` / `) )` |
| Next / previous development | `Shift+W` / `Shift+Q` | `( (` / `) )` and the arrows |
| Large histogram | `Shift+E` | `Shift+I` |
| Histogram scale | `Shift+I` | `<leader> i s` (`s` in the large one) |
| Next channel / one channel | `Shift+C` / (large histogram) | `<leader> i c` / `<leader> i r g b a l` (`c`, `r g b a l` in the large one) |
| Source (camera JPEG, RAW, developed) | `b` | `r` (next shown render, 2026-11-06) |
| Thumbnail render | `f d` | `Shift+R` (previous shown render, 2026-11-06) |
| Develop (image's software / folder) | `Shift+B` / `Ctrl+Shift+B` | `<leader> r r` / `<leader> r Shift+R` |
| Develop in darktable / RapidRAW / RawTherapee / ART | `<leader> d` / `p` / — / — | `<leader> r d` / `p` / `t` / `a` |
| Open the folder in a software (folders) | `d` | `<leader> r d` / `p` / `t` / `a` |
| Next sort / reverse | `f s` / `f r` | `n` / `Shift+N` |
| "Current stack" / "developed" filter | `f c` / `f Shift+D` | `f s` / `f r` |
| New version (crop) | `r` | `c`, then `v` (2026-11-07: crop and version are one) |
| Next / previous version | `n` / — | `v` / `Shift+V` (2026-11-06), then `Shift+V` / `Ctrl+Shift+V` (2026-11-07) |
| Visual mode (range selection) | `v` | `Shift+Space` |
| Compare | `c` | `=` |
| Loupe (held) | `t` | `m` |
| AF point | `m` | `a` |
| Infos on the image | `Ctrl+I` | `t` |
| Background | `Ctrl+L` | `b` |
| Reveal in the file manager | `Shift+R` | `Shift+E` |
| Sharpness map colour / strictness | `Ctrl+P` / `Ctrl+J`, `Ctrl+K` | `<leader> p c` / `<leader> p j`, `p k` (or Down Up) |
| Folder / metadata / filter / rating panels | `Shift+D` / `Shift+M` / `Shift+F` / `Shift+N` | `<leader> Shift+H` / `Shift+L` / `Shift+K` / `Shift+J` (or Shift + arrows) |
| Auto-advance | `Shift+A` | `<leader> Space` |
| Report a problem | `<leader> b` | `<leader> ?` |
| Folders, classic: create / rename / favourite | `a` / `r` / `m` | `Ctrl+Shift+N` / `F2` / `Ctrl+D` |
| Profile | `classic` / `vim` / `both` | `classic` / `vim` |

Idea noted for later: a number after a sequence (`<leader> m 200`: loupe at 200 %).


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
| `core.stack.open` / `core.stack.close` | ( ( / ) ) | ( ( / ) ) |
| `core.stack.enter` | Ctrl+Shift+S | Ctrl+Shift+S |
| `core.dev.next` / `core.dev.prev` | Shift+W / Shift+Q | Shift+W / Shift+Q |
| `core.dev.toggle` / `core.dev.fold_all` | — (`s`, `Shift+S` do it) | — |
| `core.file.delete` | Delete, Backspace | x, Delete, Backspace |
| `core.file.delete_photo` | Ctrl+Delete, Ctrl+Backspace | dd, Ctrl+Delete, Ctrl+Backspace |
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

**Held key** (2026-10-26, #160): only the actions meant to repeat follow the keyboard's
repeat (moves, scrolling, zoom, panels, crop, blur severity: `Action::repeats`); the
others fire once per press (holding `e` opened the editor over and over on Windows).

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
- **Graphics memory** (2026-11-07, crash "wgpu error: Out of Memory": 21 images compared
  at 1:1 on an integrated graphics card, ~100 MB per 26 Mpx 1:1): the large textures
  have a budget, `[gui.view] textures_mb` (1500 MB). When an image arrives
  (`TriApp::make_room`), the least recent ones not shown are dropped; if that is not
  enough, it keeps its preview (`over_budget`, not asked again while it stays shown) and
  the status bar says so. Safety net: a graphics card error no longer stops the program
  (wgpu's default handler did); it is recorded in `crash.log` ("sans plantage") and shown
  in the status bar (`crash::gpu_error`, hooked to `Device::on_uncaptured_error` in
  `main.rs`). The texture in cause leaves the cache (named in the message,
  `egui_texid_Managed(n)`; otherwise the last one loaded) and its image keeps its
  preview: otherwise every frame drawn afterwards failed on it (a try at
  `textures_mb = 2000` on an old laptop: frozen screen, errors in bursts). The session's
  budget goes down to what fitted (`textures_cap_mb`, `TriApp::gpu_failed`), and the
  status bar gives the value, with a "Keep N MB" button that writes it into the
  configuration (`keep_textures_budget`: not enough memory today, likely not more
  tomorrow). In a zoomed comparison, an image without its full
  definition shows its preview at the 1:1 scale (size known or estimated,
  `last_zoom_size`), with a warning at the top right explained on hover. The terminal
  gets one line per session ("détails dans …/crash.log"); a burst of errors (5 s) leaves
  only the first one in the log.
- `TRI_GUI_TRACE=1`: timed trace of the startup steps (configuration read, window opened,
  folder read, tree, first image, engine; added on 2026-10-19 for a startup slowness seen
  on macOS) and of each step of image loading (request to the engine, image ready,
  dropped, reading and decoding, image displayed) on standard error. This is what made it
  possible to measure the delays above. Since 2026-10-29 (interface frozen on a NAS), a
  frame of the interface slower than `[gui] trace_slow_ms` (100 ms) is traced with the
  duration of its steps ("image lente : 850 ms (événements 2, chargements 3, centre
  840)"): a way to find a disk access left in the interface thread.
  Since 2026-11-07, also the keys and text received from the window ("touche reçue :
  Key { key: Num1, … }", "Text("1")") and the shortcuts `tri` reads from them
  ("raccourcis lus"): a way to see what a particular keyboard or system sends.
  It showed that on Linux, egui-winit also sends Shift and Ctrl as keys (`ShiftRight`…):
  Shift, typed before a top-row digit of an AZERTY keyboard, closed the `<leader> m`
  entry and the digit changed the rating. A lone modifier is no longer a shortcut
  (`Chord::from_event`).

**Closing**: background work (engine, image loading, folder and volume watches) stops
without being waited for (`detach`); a thumbnail in progress on a NAS no longer holds the
window open. The cache is written under a temporary name then renamed: nothing stays
half-written in it.

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
  (`grid::fit_all`: the largest size that shows the whole filtered folder; a switch
  since 2026-11-07, `TriApp::toggle_grid_fit`: off, it gives back the size before;
  changing the size by hand turns it off); on the
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
cache, and receives simulated keys. Checked: the help opens and closes with F1, the
"help here" with `?` (with or without Shift), Esc closes it, it is modal, it displays the labels and
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

Comparison (2026-10-17): `c` refused with a single image (hint in the status bar); `c` on
a developed photo compares the photo and its clone, Esc comes back on the active image
shown in place of the cover, a collapsed burst compares its photos without duplicates
(2026-10-31); three
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
