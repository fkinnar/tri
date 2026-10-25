# Changelog

*[Version française](CHANGELOG.fr.md)*

## Unreleased

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
