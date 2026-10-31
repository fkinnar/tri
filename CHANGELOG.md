# Changelog

*[Version française](CHANGELOG.fr.md)*

## Unreleased

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
