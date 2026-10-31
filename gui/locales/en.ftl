# Interface catalogue — English.
# Every identifier must also exist in fr.ftl (checked by the tests).

## Actions (help, tooltips, buttons): action-<action identifier>

action-core-nav-prev = Previous image
action-core-nav-next = Next image
action-core-nav-up = Row above
action-core-nav-down = Row below
action-core-nav-first = First image
action-core-nav-last = Last image
action-core-nav-page_up = Previous page
action-core-nav-page_down = Next page
action-core-filter-cycle = Type filter: all, RAW, JPEG…
action-core-filter-stars-0 = Filter: any stars
action-core-filter-stars-1 = Filter: 1 star (at least, exactly or at most: f =)
action-core-filter-stars-2 = Filter: 2 stars (at least, exactly or at most: f =)
action-core-filter-stars-3 = Filter: 3 stars (at least, exactly or at most: f =)
action-core-filter-stars-4 = Filter: 4 stars (at least, exactly or at most: f =)
action-core-filter-stars-5 = Filter: 5 stars (at least, exactly or at most: f =)
action-core-filter-stars-op = Star filter: at least, exactly, at most
action-core-filter-flag-accepted = Filter: picked only
action-core-filter-flag-not_rejected = Filter: hide rejected
volume-back = Drive back: folder read again, computations resumed.
action-core-filter-flag-rejected = Filter: rejected only
action-core-filter-color-red = Filter: red (add / remove)
action-core-filter-color-yellow = Filter: yellow (add / remove)
action-core-filter-color-green = Filter: green (add / remove)
action-core-filter-color-blue = Filter: blue (add / remove)
action-core-filter-color-purple = Filter: purple (add / remove)
action-core-filter-clear = Clear all filters
action-core-filter-blurry = Filter: hide blurry
action-core-filter-well_exposed = Filter: well exposed (neither blown nor blocked)
action-core-filter-stack = Filter: only the current stack, fully expanded (again: everything)
action-core-filter-invert = Filter: invert (what the filters remove)
action-core-filter-developed = Filter: developed (on / off; software to check in the list)
action-core-grid-render = Next thumbnail render: camera JPEG, latest development, a software, LibRaw
action-core-filter-sort = Next order: name, shooting date, modification date
action-core-filter-sort_reverse = Reverse the order (ascending / descending)
sort-name = Name
sort-captured = Shooting date
sort-modified = Modified
sort-notice = Order: { $key }, { $direction }
filter-analysis-partial = Analyses in progress: filter applied to the { $done } / { $total } photos already analysed; the grid fills in as they are computed.
sort-ascending = ascending
sort-descending = descending
action-core-view-filter_bar = Show / hide the filter bar
action-core-view-image_info = Focus mode: show / hide the info and ratings on the image
action-core-view-background = Next background colour (theme, dark grey, black, white)
action-core-view-rating_bar = Show / hide the rating bar
action-core-view-grid_info = Show / hide the thumbnail info (name, format, ratings, badges)
action-core-rate-auto_advance = Auto-advance after rating: on / off
action-core-select-toggle = Select / deselect the image
action-core-select-visual = Visual mode: select a range (v to confirm)
action-core-view-compare = Compare the selected images side by side (no selection: the collapsed stack, or the photo and its developments)
action-core-view-peaking = Show / hide sharpness (sharp edges)
action-core-view-sharpness_lenient = More lenient sharpness (fewer blurry photos, more sharp edges)
action-core-view-sharpness_strict = Stricter sharpness (more blurry photos, fewer sharp edges)
action-core-view-peaking_color = Sharpness: next colour
action-core-view-overlay = Next overlay (grids, then yours: text, logo, format)
overlay-shown = Overlay: { $name }
overlay-none = Overlay: none
overlay-error = Overlay { $file }: { $error }
action-core-view-overlay_orient = Overlay: turn the lines (spiral eye from corner to corner)
overlay-orientation = Overlay: orientation { $n } / 4
action-core-search = Search: in the grid, by file name; in the folder panel, a folder (also /)
search-grid-hint = Search ({ $keys })
search-folder-hint = Search… ~/ or /
search-folder-tip = Search a folder ({ $keys }): among the subfolders of the pointed one; “~/…” from the home folder, “/…” from the root or a typed path; “\\nas\photo”, “smb://nas/photo”, “ftp://nas”, “sftp://…”, “davs://…” for a network server. ↑ ↓ choose, Enter opens, Esc closes.
search-folder-in = in { $base }
search-folder-running = searching…
search-folder-none = no folder found
search-folder-connect = Connect to "{ $name }"
search-folder-address = address of a server: \\server\share, smb://server/share; also afp://, ftp://, sftp://, dav://, davs://, nfs://
search-folder-shares-running = looking for the shares of { $host }…
search-folder-shares-failed = cannot list the shares of { $host }: { $error }
search-folder-shares-none = no share on { $host }
search-folder-shares-enter = Enter: shares of { $host }
filter-summary-name = “{ $query }”
action-core-view-focus_mode = Focus mode: full screen, no panels nor bars
focus-mode-on = Focus mode ({ $keys } to leave)
action-core-view-focus_point = Show / hide the focus point
action-core-select-all = Select all
action-core-select-clear = Deselect all
action-core-file-rename = Rename…
action-core-file-delete = Move to trash…
action-core-file-reveal = { $os ->
    [macos] Show in Finder
    [windows] Show in Explorer
   *[other] Show in the file manager
}
action-core-file-edit = Open in the external editor
action-core-file-copy = Copy to…
action-core-file-export-report = Export report
action-core-file-export = Export…
action-core-view-crop = Crop (new version)
action-core-view-version_next = Next version of the image (original, then each version)
action-core-crop-move-left = Crop: move the frame left
action-core-crop-move-right = Crop: move the frame right
action-core-crop-move-up = Crop: move the frame up
action-core-crop-move-down = Crop: move the frame down
action-core-crop-size-left = Crop: right edge inwards
action-core-crop-size-right = Crop: right edge outwards
action-core-crop-size-up = Crop: bottom edge up
action-core-crop-size-down = Crop: bottom edge down
action-core-crop-ratio-free = Crop: free ratio
action-core-crop-ratio-original = Crop: image ratio
action-core-crop-ratio-square = Crop: 1:1
action-core-crop-ratio-5_4 = Crop: 5:4 (4:5)
action-core-crop-ratio-3_2 = Crop: 3:2 (2:3)
action-core-crop-ratio-16_9 = Crop: 16:9 (9:16)
action-core-crop-ratio-4_3 = Crop: 4:3 (3:4)
action-core-crop-rotate = Crop: portrait or landscape
action-core-crop-save = Crop: save the version
action-core-crop-preset = Crop: next preset
action-core-crop-preset_prev = Crop: previous preset
action-core-file-delete_stack = Delete the whole stack
action-core-file-delete_photo = Delete the photo with its developments…
action-core-file-copy_stack = Copy the whole stack to…
stack-actions-need-stacks = Stacks are off ({ $keys }): this action is for a whole stack
action-plugin-airdrop-send = Send with AirDrop
send-started = { $name }: { $n ->
    [one] { $n } image to send
   *[other] { $n } images to send
}
action-core-file-move = Move to…
action-core-edit-undo = Undo the last change
action-core-edit-redo = Redo
action-core-edit-history = History of changes
action-core-stack-toggle = Expand / collapse the stack
action-core-stack-all = Expand / collapse all stacks
action-core-stack-settings = Grouping of similar images
action-core-stack-mode = Stacks (grid; full screen: left / right arrows inside the stack, up / down from stack to stack)
action-core-dev-toggle = Unfold / fold the photo's developments
action-core-dev-next = Next development of the photo
action-core-dev-fold_all = Fold every development stack
action-core-dev-prev = Previous development of the photo
action-core-stack-choose = Make it the stack cover (its pick)
action-core-stack-first = First image of the stack
action-core-stack-last = Last image of the stack
action-core-stack-enter = Entering a stack: its first photo (on) or its cover
## A photo's developments (darktable, RapidRAW; 2026-10-30)
dev-count = { $n } dev.
dev-count-tip = { $n ->
    [one] { $n } development
   *[other] { $n } developments
  } · { $keys }: unfold / fold
dev-clone = clone { $n }
dev-copy = copy { $n }
dev-none = This photo has no development (darktable, RapidRAW)
dev-before = Before: the camera JPEG instead of the development (b: back)
dev-after = After: the development
dev-preferred = { $name }: cover of the photo
dev-preferred-none = No preferred development any more: the stack folds on the latest one
dev-fold-tip = Fold the developments ({ $keys }) · Alt+click: fold and keep this one as the cover
dev-folded-already = Every development stack is folded
dev-badge-tip = Development: { $name }
dev-badge-tip-pending = Development: { $name } (the photo's thumbnail until it is rendered)
view-info-development = before ({ $name } pending or missing)

stack-chosen = { $name }: cover of its stack
stack-position = stack: { $n } / { $total }
stack-slider-tip = Stack strictness: in the middle, the value computed from the folder; to the right stricter (smaller stacks), to the left more lenient; double-click: back to the middle; hold Shift while adjusting to expand the stacks ({ $keys }: fine settings)
blur-slider-tip = Blur strictness: to the right stricter (more blurry photos), to the left more lenient; in the middle, the settings. Double click: back to the middle
clip-slider-over = blown
clip-slider-over-tip = Blown photos excluded: to the right stricter, to the left more lenient; fully to the left, blown highlights no longer exclude anything. In the middle, the settings. Double click: back to the middle
clip-slider-under = crushed
clip-slider-switch = Slider: { $side } (click: switch to the other side)
clip-slider-under-tip = Crushed photos excluded: to the right stricter, to the left more lenient; fully to the left, crushed blacks no longer exclude anything (black backgrounds). In the middle, the settings. Double click: back to the middle
sharpness-strictness = Sharpness strictness: { $value } % (0: the settings)
stack-icon-tip = Right click: fine settings ({ $keys })
action-core-panel-left = Keyboard to the left panel
action-core-panel-right = Keyboard to the right panel
action-core-panel-next = Keyboard to the next panel
action-core-folders-up = Folders: folder above
action-core-folders-down = Folders: folder below
action-core-folders-expand = Folders: expand (or enter)
action-core-folders-collapse = Folders: collapse (or go up)
action-core-folders-open = Folders: open the folder
action-core-folders-first = Folders: first folder
action-core-folders-last = Folders: last folder
action-core-metadata-up = Metadata: line above
action-core-metadata-down = Metadata: line below
action-core-metadata-copy = Metadata: copy the value
action-core-folders-offload = Folders: offload the removable drive
action-core-folders-eject = Folders: eject the removable drive
action-core-folders-create = Folders: new folder
action-core-folders-rename = Folders: rename the folder
action-core-folders-delete = Folders: delete the folder
action-core-folders-favorite = Folders: add to or remove from favourites
action-core-folders-favorite_name = Folders: name the favourite
action-core-folders-connect = Folders: connect to a network share
action-core-folders-forget = Folders: forget the remembered network share
action-core-folders-copy = Folders: copy the folder to…
action-core-folders-move = Folders: move the folder to…
action-core-metadata-open = Metadata: open (large histogram)
action-core-metadata-select_up = Metadata: extend the selection up
action-core-metadata-select_down = Metadata: extend the selection down
action-core-metadata-copy_all = Metadata: copy everything
action-core-metadata-section_next = Right panel: next section
action-core-metadata-section_prev = Right panel: previous section
action-core-metadata-activate = Right panel: use the pointed setting
action-core-metadata-left = Right panel: setting to the left (fold the section)
action-core-metadata-right = Right panel: setting to the right (unfold the section)
action-core-folders-section_next = Folders: next section
action-core-folders-section_prev = Folders: previous section
action-core-view-folders = Show / hide the folder panel
action-core-view-refresh_folders = Refresh folders (drive plugged in)
action-core-view-histogram = Show / hide the histogram
action-core-view-histogram_scale = Histogram: EV / linear
action-core-view-histogram_large = Large histogram
action-core-view-histogram_channel = Large histogram: next channel
action-core-histogram-red = Large histogram: red only
action-core-histogram-green = Large histogram: green only
action-core-histogram-blue = Large histogram: blue only
action-core-histogram-photo = Large histogram: show / hide the analysis photo
action-core-histogram-camera = Large histogram: show / hide the camera JPEG curve
histogram-photo-tip = Analysis photo (clipped highlights / shadows on the grey image): show / hide ({ $keys })
action-core-histogram-all = Large histogram: all channels
action-core-histogram-luminance = Large histogram: luminance only
action-core-view-clipping = Show / hide clipped highlights and shadows
action-core-view-metadata = Show / hide the metadata panel
action-core-view-open = Open full screen
action-core-view-zoom = Zoom 100 % / fit
action-core-view-zoom_in = Zoom: next step up (in the grid: larger thumbnails)
action-core-view-zoom_out = Zoom: next step down (in the grid: smaller thumbnails)
action-core-view-zoom_fit = Zoom: fit (in the grid and the folders, / searches)
action-core-view-zoom_100 = Zoom: 100 % (in the grid: all thumbnails on screen)
view-info-zoom = { " · " }{ $percent } %
view-loupe-loading = loading 100 %…
action-core-view-source = Switch camera render / RAW render
action-core-view-pan_left = Zoomed image: move left
action-core-view-pan_right = Zoomed image: move right
action-core-view-pan_up = Zoomed image: move up
action-core-view-pan_down = Zoomed image: move down
action-core-view-back = Close / go back
action-core-app-help = Show or hide this help
action-core-app-settings = Settings
action-core-app-check_update = Check for updates
action-core-app-report_issue = Report a problem (prefilled GitHub form)
action-core-app-quit = Quit
action-core-rate-star-0 = Remove stars
action-core-rate-star-1 = 1 star
action-core-rate-star-2 = 2 stars
action-core-rate-star-3 = 3 stars
action-core-rate-star-4 = 4 stars
action-core-rate-star-5 = 5 stars
action-core-rate-color-red = Red label (add / remove)
action-core-rate-color-yellow = Yellow label (add / remove)
action-core-rate-color-green = Green label (add / remove)
action-core-rate-color-blue = Blue label (add / remove)
action-core-rate-color-purple = Purple label (add / remove)
action-core-flag-accept = Pick
action-core-flag-reject = Reject
action-core-flag-none = Neither picked nor rejected

## Help groups

group-navigation = Navigation
group-selection = Selection
group-view = Display
group-rating = Rating
group-filters = Filters
group-panels = Panels
group-files = Files
group-edit = Undo
group-app = Application
group-crop = Crop
crop-bar-title = Crop
crop-ratio-free = Free
crop-ratio-original = Original
crop-portrait = Portrait
crop-landscape = Landscape
crop-name = Name
crop-profile = Export
crop-profile-none = None
crop-save = Save
crop-saved = Version "{ $name }" saved
crop-name-taken = This name is taken (another version, or “Original”: the uncropped image)
crop-name-original-ratio = Crop
crop-name-taken-notice = Version not saved: the name "{ $name }" is taken
crop-failed = Version not saved: { $error }
crop-preset-none = Preset
crop-preset-tip = Crop preset: ratio, name and export preset (p: next)
crop-preset-save = Save as crop preset
crop-preset-save-tip = Save as crop preset: ratio, name and export of the current frame, kept in the settings
crop-preset-saved = Preset "{ $name }" saved
preset-story = Story
preset-post = Post
versions-current = Versions: { $name }
versions-original = Original
versions-edit = Edit
versions-delete = Delete
versions-deleted = Version "{ $name }" deleted

## Panels, ejection, ratings

panel-no-image = No image selected.
histogram-enlarge = Enlarge ({ $keys })
histogram-camera-tip-short = Grey line: camera JPEG
histogram-camera-row = Camera JPEG
histogram-camera-row-tip = Grey line: the camera JPEG, measured under its own white (255) and at its own height, to see what the camera did with the highlights and shadows. Figures: { $channel }.
histogram-camera-button-tip = Camera JPEG curve: show / hide ({ $keys })
histogram-computing = histogram being computed
panel-name = Name
panel-reading = reading…
folders-title = Folders
folders-section-devices = Devices
folders-section-drives = Drives
folders-section-network = Network
folders-section-personal = Personal folders
folders-section-favorites = Favourites
folders-section-system = System
folders-refresh = Refresh
folders-refresh-tip = Re-read volumes and folders (F5). A plugged-in drive also shows up by itself within a few seconds.
folders-subfolders = Subfolders
folders-subfolders-tip = Also show the images of subfolders ({ $n } levels at most, [gui.folders] subfolders_depth)
folders-collapse = Collapse
folders-expand = Expand
folders-unreadable = unreadable
folders-fixed-drive = Fixed drive ([places] fixed): no ejection from tri
folders-offload = Offload the card…
eject = Eject
cancel = Cancel
eject-question = Eject "{ $name }"?
eject-open-folder = The open folder is on this volume: it will be closed.
ejecting = Ejecting "{ $name }"…
ejected = "{ $name }" ejected: you can remove it.
status-clear-tip = Click to clear the message
mounting = Mounting "{ $name }"…
mounted = "{ $name }" mounted.
mount-failed = "{ $name }" could not be mounted: { $error }
folders-device-tip = Not mounted: click to mount it.
folders-server-tip = Remembered network share, not connected ({ $address }): click to connect.
folders-neighbour-tip = Server on the network ({ $address }): a click shows its shares, or connects.
folders-connect-tip = Type the address of a server (\\nas\photo, smb://nas/photo, ftp://nas…); the password is asked for if needed, and the server then stays under "Network".
connecting = Connecting to "{ $name }"…
connected = Connected to "{ $name }".
connect-failed = Could not connect to "{ $name }": { $error }
folder-opening = Opening "{ $name }"…
folder-opening-found = { $count ->
    [0] No image found yet
    [one] 1 image found
   *[other] { $count } images found
}
folder-opening-why = Slow folder (network, disk): its list is read in the background.
folder-open-cancelled = Opening "{ $name }" cancelled.
login-title = Connect to "{ $name }"
login-why = The server { $address } asks for an account.
login-refused = User name or password refused: check them and try again.
login-user = User
login-domain = Domain
login-domain-hint = WORKGROUP (most often)
login-password = Password
login-remember = Remember the password
login-connect = Connect
login-cancelled = Connection to "{ $name }" given up.
connected-not-saved = Connected to "{ $name }", but the password could not be remembered: { $error }
server-forgotten = Share "{ $name }" forgotten.
eject-failed = "{ $name }" not ejected: { $error }
cache-unavailable = cache unavailable: { $error }

## Comparison, full screen, status bar

rating-not-saved = rating not saved: { $error }
compare-needs-two = compare: select at least 2 images (Space, v, Ctrl+A, Shift+click), or stand on a stack or a developed photo
compare-limited = comparison limited to the first { $max } ([gui.compare] max)
view-info-selected = { " " }· selected
view-info-peaking = { " " }· sharpness
view-info-peaking-computing = { " " }· computing sharpness
view-info-sensor = { " " }({ $percent } % of the sensor)
view-info-resolution = { " · " }{ $width } × { $height }
view-info-preview-computing = { " " }· preview being computed
view-info-raw-pending = { " " }· RAW render in progress
view-info-full-computing = { " " }· computing 100 %
view-info-one-to-one-loading = { " " }· loading 1:1
view-hint-raw = b: camera render / RAW   z: 1:1   i: histogram   w: zones   Esc: grid
view-hint-flat = z: 1:1   i: histogram   w: zones   Esc: grid
status-no-folder = no folder
status-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
} ({ $raws } RAW{ $devs ->
    [0] { "" }
   *[other] , { $devs } dev.
}) · filter: { $filter }
status-selected = { $n } selected
status-visual-mode = { " " }· visual mode
progress-notes = ratings
progress-thumbnails = thumbnails
progress-embedded = camera render
progress-previews = previews
progress-histograms = histograms
status-key-problems = { $n ->
    [one] shortcuts: { $n } problem
   *[other] shortcuts: { $n } problems
}
status-invalid-config = invalid configuration: default settings
status-show-offload = Show the offload
tasks-global = { $n ->
        [one] 1 task running
       *[other] { $n } tasks running
    } · { $percent } %
tasks-unfold = Click: show the details
tasks-fold = Click: fold the details
status-retry-eject = Retry ejecting
status-dismiss = dismiss
status-open-session = Open the session "{ $name }"

## Dialogs, rating bar, filters, help

close = Close
close-tip = Close (Esc)
histogram-scale-ev = in EV
histogram-scale-linear = linear
histogram-scale-tip = Scale { $current }; click or { $keys }: { $other }
or-escape = or Esc
grid-fit = All thumbnails on screen
grid-size = Thumbnail size
files-with-pairs = With the other file of the RAW+JPEG pair
files-with-pairs-tip = Untick to act only on the chosen images (e.g. delete the JPEGs and keep the RAWs)
help-copy-version = Copy the version and system (for a report)
help-version-copied = Version and system copied
help-report = Report a problem
help-report-tip = Opens the GitHub form, prefilled with the version and system
update-tip = Asks the project's Releases page (nothing is sent without this click)
update-title = Update
update-checking = Checking…
update-latest = You have the latest version ({ $version }).
update-available = Version { $version } available (you have { $current }).
update-open-page = Open the download page
download-nightly = Download the nightly build
download-nightly-tip = The nightly build: today's state, with the latest fixes and features, tested automatically but not published yet. To try what just arrived.
download-release = { $stage ->
    [alpha] Download the latest alpha
    [beta] Download the latest beta
   *[other] Download the latest version
}
download-release-tip = { $stage ->
    [alpha] The latest published alpha: a numbered version, with its notes, steadier than the nightly build but less recent.
    [beta] The latest published beta: a numbered version, with its notes, steadier than the nightly build but less recent.
   *[other] The latest published version, with its notes: steadier than the nightly build, but less recent.
}
download-searching = Looking for the package to download…
download-started = Downloading in the browser: { $name }
download-page = No package for this system: page of version { $version } opened
download-title = Download tri { $version }
download-choose = Several packages for this system:
download = Download
download-format = { $format ->
    [appimage] A single file to run, no installation, for every distribution
    [deb] Debian, Ubuntu, Linux Mint package: installed by the system
    [targz] Archive with its install script (install.sh)
    [msi] Windows installer
    [zip] Archive to unpack, no installation
    [dmg] Disk image: drag tri into Applications
   *[other] Package for this system
}
rating-bar-title = Rating
filter-bar-title = Filter
rating-bar-applies = ratings apply to the { $n } selected images
filter-stack-none = f c: stand on a stack (stacks on)
stack-enter-cover = Entering a stack: its cover
stack-enter-first = Entering a stack: its first photo
stack-enter-needs-stacks = { $now } (in full screen, stacks on: { $keys })
filter-summary-stack = stack of { $n }
filter-summary-stars = { $n ->
    [5] 5 stars
    [one] { $n } star and more
   *[other] { $n } stars and more
}
filter-summary-stars-exactly = { $n ->
    [one] exactly { $n } star
   *[other] exactly { $n } stars
}
filter-summary-stars-at-most = { $n ->
    [one] { $n } star or fewer
   *[other] { $n } stars or fewer
}
stars-op-at-least = Star filter: at least
stars-op-exactly = Star filter: exactly
stars-op-at-most = Star filter: at most
filter-summary-or = { " or " }
filter-all = all
filter-flat = JPEG…
filter-bar-flat = JPEG…
filter-bar-raw-tip = RAW: shown (on) or hidden
filter-bar-flat-tip = JPEG and other images (TIFF, PNG, HEIC…): shown (on) or hidden
filter-bar-clear = Clear
filter-bar-count = { $shown }/{ $n }
badge-over = { $percent } % of pixels clipped (blown); click: large histogram
badge-under = { $percent } % of pixels crushed; click: large histogram
badge-versions = { $n ->
    [one] 1 version (crop); click: crop
   *[other] { $n } versions (crops); click: crop
}
help-title = Shortcuts
help-search-hint = Search (e.g. "fpn", "star 3")…
help-no-match = No matching action.
help-close-hint = Esc or F1 to close.
empty-folder-gone = The folder "{ $name }" is no longer there (drive unplugged?)
empty-folder-gone-back = It will open again by itself when the drive is back.
empty-no-folder = Choose a folder in the left panel (Shift+D shows it).
empty-folder = No image in this folder.
filter-toast = { $n ->
    [one] 1 image
   *[other] { $n } images
}
filter-toast-none = No images
develop-toast = { $n ->
    [one] 1 changed development to redo ({ $done }/1)
   *[other] { $n } changed developments to redo ({ $done }/{ $n })
}
develop-toast-done = { $n ->
    [one] 1 development redone
   *[other] { $n } developments redone
}
empty-filtered = No image for these filters (f then Esc to clear them).
color-red = red
color-yellow = yellow
color-green = green
color-blue = blue
color-purple = purple

## Flag filters

flag-filter-accepted = picked
flag-filter-not-rejected = without rejected
flag-filter-rejected = rejected

## Rename, trash

rename-title = Rename "{ $name }"
rename-becomes = { $from } becomes { $to }
rename-copied = { $from } copied as { $to } (shared with the pair)
rename-button = Rename
enter-or-escape = Enter / Esc
export-title = { $n ->
    [one] Export 1 image
   *[other] Export { $n } images
}
export-title-version = Export the "{ $name }" version
export-contents-developed = { $n ->
    [one] 1 RAW, developed by { $engine }
   *[other] { $n } RAW, developed by { $engine }
}
export-contents-libraw = { $n ->
    [one] 1 RAW, basic rendering (LibRaw)
   *[other] { $n } RAW, basic rendering (LibRaw)
}
export-contents-embedded = { $n ->
    [one] the camera render of 1 RAW
   *[other] the camera render of { $n } RAW
}
export-contents-flat = { $n ->
    [one] 1 already developed image
   *[other] { $n } already developed images
}
export-contents-paired = { $n ->
    [one] 1 camera JPEG (pair, suffixed name)
   *[other] { $n } camera JPEGs (pairs, suffixed names)
}
export-profile = Export preset
export-profile-tip = Up / down arrows: preset · Tab: folder · Enter: export
export-destination = Folder
export-create = will be created
export-goes-to = Destination: { $dest }
export-no-developer = No development software found: base render by LibRaw.
export-rapidraw-srgb = RapidRAW renders in sRGB: the “{ $color }” space is converted from sRGB.
export-passthrough = Without recompression
export-passthrough-tip = Original JPEG copied as is, camera render written as is (orientation in the EXIF). An image to shrink (preset size) is re-encoded.
export-button = Export
export-profile-edit = Editing
export-profile-asis = JPEG as is
export-profile-web = Web
export-format-jpeg = JPEG { $quality }%
export-format-tiff = TIFF { $bits }-bit
export-format-png = PNG { $bits }-bit
export-color-image = original color space
export-color-linear = { $name } (linear)
export-sharpen-screen = screen sharpening
export-sharpen-print = print sharpening
export-size-full = full resolution
export-size-max = { $side } px max
export-busy = An export is already running.
export-done = { $n ->
    [one] 1 image exported to { $dest }
   *[other] { $n } images exported to { $dest }
}
export-done-passthrough = { " " }({ $n } without recompression)
export-partial = { $n } exported, { $failed } failed (unreadable by Darktable?), to { $dest }
export-cancelled = Export stopped (files already written are kept).
export-progress = Export “{ $profile }” { $done }/{ $total }
export-cancel = Stop the export
export-report-title = Export report
export-report-empty = No export yet.
export-report-version = version "{ $name }"
export-report-summary = { $total ->
    [one] { $written } file from 1 image
   *[other] { $written } files from { $total } images
}
export-report-passthrough = ({ $n } without recompression)
export-report-now = just now
export-report-minutes = { $n } min ago
export-report-hours = { $n } h ago
export-report-days = { $n ->
    [one] 1 day ago
   *[other] { $n } days ago
}
export-report-months = { $n ->
    [one] 1 month ago
   *[other] { $n } months ago
}
export-report-years = { $n ->
    [one] 1 year ago
   *[other] { $n } years ago
}
export-report-files = { $n ->
    [one] 1 file
   *[other] { $n } files
}
export-report-errors = { $n ->
    [one] 1 failure
   *[other] { $n } failures
}
export-report-reveal = Show in the file manager
export-report-clear = Clear the list
export-report-open = Export report
rename-failed = cannot rename: { $error }
transfer-title = { $n ->
    [one] { $n } image to…
   *[other] { $n } images to…
}
transfer-files = { $n ->
    [one] { $n } file (XMP sidecars included)
   *[other] { $n } files (XMP sidecars included)
}
transfer-shared = { $name } is shared with an image that stays: copied
transfer-create = the folder will be created
transfer-copy = Copy
transfer-move = Move
transfer-confirm = Confirm
transfer-hint = Tab: complete · up / down arrows: recent destinations
transfer-recent = Recent
transfer-bring-cache = Bring the cache along
transfer-bring-cache-tip = Thumbnails, previews and analyses follow the folder: nothing to compute again
transfer-progress = { $done } / { $total } files
transfer-background = Continue in the background
transfer-status = copying to "{ $name }" { $done }/{ $total }
transfer-done = Copy / move done
status-show-transfer = Show the copy
transfer-failed = copy / move failed: { $error }
transfer-folder-title = Folder "{ $name }" to…
drag-images = { $n } images
drag-move = move (⌥: copy)
drag-move-ctrl = move (Ctrl: copy)
drag-copy = copy
groups-title = Grouping of similar images
groups-enabled = Group into stacks
groups-threshold = Strictness (middle: computed from the folder)
groups-threshold-tip = To the right: only bursts; to the left: larger stacks.
groups-threshold-value = Threshold: { $threshold } (folder centre: { $centre })
groups-gap = Maximum time gap
groups-gap-tip = Two images taken further apart are never grouped (0: no limit).
groups-summary = { $images } images: { $stacks } stacks, { $groups } groups in all
groups-computing = signatures: { $done } / { $total }
groups-default = Back to the middle
history-title = History of changes
history-start = State at launch
history-empty = No changes in this session.
history-undo = Undo
history-redo = Redo
history-undone = Undone: { $label }
history-redone = Redone: { $label }
history-refused = refused: { $error }
focus-folders = keyboard: folders (Esc: images)
focus-metadata = keyboard: metadata (Esc: images)
metadata-copied = { $label } copied to the clipboard
metadata-copied-lines = { $n } lines copied to the clipboard
metadata-copy-line = { $label }: { $value }
folders-not-removable = "{ $name }" is not a removable drive
offload-source = Card:
folders-no-removable = no removable drive to offload
settings-title = Settings
settings-search-hint = Search a setting (name, section, description)…
settings-file = file: { $path }
settings-bold = (bold: set in this file)
settings-in-file = Set in your configuration file: "default" removes it.
settings-save-failed = cannot save { $path }: { $error }
settings-keys-doc = Shortcuts of each action: the chosen set's ([gui] profile), or those written here, which replace them for that action (in bold). Click a shortcut to remove it; “record…” adds the next key (or sequence) typed; the field takes a written shortcut (“Shift+P”, “g g”, “°”). Conflicts are reported.
loupe-key-unknown = unknown loupe key: “{ $key }”
loupe-key-taken = [gui.zoom] loupe_key: “{ $key }” is also used by an action
settings-keys-none = no shortcut
settings-keys-remove = Remove “{ $keys }”
settings-keys-record = record…
settings-keys-record-tip = Type the shortcut (a sequence: one key after the other, Escape included); it is added once the sequence delay passes with no new key ([gui] key_sequence_timeout_ms).
settings-keys-recording = type the shortcut… { $keys }
settings-keys-type-hint = or type: g g
settings-keys-default = Back to the chosen set's shortcuts
settings-reset = default
settings-default = default value: { $value }
settings-restart = on next launch
settings-section-restart = Engine settings: applied on next launch.
settings-no-path = unknown configuration file location (--config FILE)
settings-section-preview = Previews
settings-section-render = RAW rendering
settings-section-cache = Cache
settings-section-scheduler = Background work
settings-section-xmp = Ratings (XMP)
settings-section-keys = Shortcuts
settings-section-develop = Darktable rendering
settings-section-export-profile-user = Export preset “{ $id }”
settings-section-versions-preset-user = Crop preset “{ $id }”
settings-section-export-profiles = Export presets
settings-section-versions-presets = Crop presets
settings-member-new = New…
settings-member-name = Name:
settings-member-create = Create
settings-member-create-tip = Copy of the one shown, under this name
settings-member-rename = Rename…
settings-member-rename-ok = Rename
settings-member-rename-tip = New name for the one shown
settings-member-taken = This name is already taken
settings-member-delete = Delete
settings-member-builtin = Provided with tri: editable, not deletable
settings-profile-none = None
settings-section-versions-presets-story = Crop preset “Story”
settings-section-versions-presets-post = Crop preset “Post”
settings-section-export = Export
settings-section-export-sharpening = Output sharpening
settings-section-export-profiles-edit = Export preset “Editing”
settings-section-export-profiles-asis = Export preset “JPEG as is”
settings-section-export-profiles-web = Export preset “Web”
settings-section-watch = Folder watching
settings-section-update = Updates
settings-section-places = Places
settings-section-exposure = Exposure
settings-section-offload = Card offload
settings-section-remote = Local copy of a server
settings-section-neighbourhood = Network neighbourhood
settings-section-files = Files
settings-section-sharpness = Sharpness
settings-section-similarity = Grouping
settings-section-i18n = Language
settings-section-data = Data
settings-section-gui = Interface
settings-section-gui-grid = Grid
settings-section-gui-view = Full screen
settings-section-gui-zoom = Zoom and loupe
settings-section-gui-dialogs = Dialogs
settings-section-gui-compare = Compare
settings-section-gui-crop = Crop
settings-section-gui-folders = Folder panel
settings-section-gui-histogram = Histogram
settings-section-gui-sharpness = Sharpness map
settings-section-gui-metadata = Metadata panel
settings-section-gui-colors = Colours
delete-to-trash = Move to trash
delete-permanently = Delete permanently
delete-trash-question = { $n ->
    [one] Move { $n } file to trash?
   *[other] Move { $n } files to trash?
}
delete-permanently-question = { $n ->
    [one] Delete { $n } file permanently?
   *[other] Delete { $n } files permanently?
}
delete-kept-shared = { $name } kept (shared with the pair)
delete-irreversible = Cannot be undone.
delete-failed = cannot delete: { $error }
delete-developments = Remove
delete-developments-question = { $n ->
    [one] Remove the development from its XMP?
   *[other] Remove { $n } developments from their XMP?
}
delete-stripped = { $name }: darktable history removed, notes kept
folder-new-title = New folder in "{ $name }"
folder-new-button = Create
folder-rename-title = Rename the folder "{ $name }"
folder-delete-trash = Move the folder "{ $name }" to the trash?
folder-delete-permanently = Permanently delete the folder "{ $name }"?
folder-contents = { $files ->
    [0] Empty folder
    [one] 1 file
   *[other] { $files } files
}{ $dirs ->
    [0] {""}
    [one] , 1 subfolder
   *[other] , { $dirs } subfolders
}, everything included.
folder-failed = { $error }
folders-place-root = "{ $name }" is a place: it cannot be renamed, moved or deleted
folder-menu-new = New folder…
folder-menu-rename = Rename…
folder-menu-move = Move to…
folder-menu-copy = Copy to…
folder-menu-delete = Delete…
folder-menu-offload = Offload the card…
folder-menu-eject = Eject
folder-menu-mount = Mount
folder-menu-connect = Connect
folder-menu-forget = Forget this share
folder-menu-offload-here = Offload a card here…
folder-menu-cache-folder = Keep the cache in this folder
folder-menu-cache-disk = Keep the cache on this drive
folder-menu-cache-release = No longer keep the cache here
folder-menu-cache-tip = Thumbnails, previews and analyses of the photos of this folder (subfolders included) stored in .tri-cache: they travel with it, found again on another computer
cache-kept-here = Cache kept in { $place }
cache-released = Cache removed from { $place }
cache-kept-failed = Cache kept in the user's folder: { $error }
cache-move-failed = Cannot move the cache: { $error }
watch-reloaded = { $count ->
    [one] One image added, removed or changed elsewhere: folder reloaded
   *[other] { $count } images added, removed or changed elsewhere: folder reloaded
}
watch-notes = { $count ->
    [one] Notes of one image changed elsewhere: reloaded
   *[other] Notes of { $count } images changed elsewhere: reloaded
}
folder-menu-favorite-add = Add to favourites
folder-menu-favorite-remove = Remove from favourites
folder-menu-favorite-name = Name the favourite…
favorite-name-title = Name of the favourite "{ $name }"
favorite-name-button = Name
favorite-named = Favourite named "{ $name }"
favorite-not-favorite = "{ $name }" is not a favourite (m to add it)
folders-favorite = Favourite
folders-favorite-added = "{ $name }" added to favourites
folders-favorite-removed = "{ $name }" removed from favourites
folders-favorite-error = Favourites not saved: { $error }

## What is shown

shown-thumbnail = Thumbnail (preview being computed)
shown-embedded = Camera render
shown-raw = RAW render
shown-raw-full = RAW render · 100 %
shown-embedded-full = Camera render · 1:1
shown-image-full = Image · 100 %
shown-exposure = Exposure analysis
shown-developed = Developed in { $engine }
view-info-developed-pending = { " · " }development in progress…
action-core-develop-render = Develop the shown image (in the software of its development)
action-plugin-darktable-develop = Develop in Darktable (the shown image)
action-plugin-rapidraw-develop = Develop in RapidRAW (the shown image)
action-plugin-darktable-open_folder = Folders: open the pointed folder in Darktable
action-plugin-rapidraw-open_folder = Folders: open the pointed folder in RapidRAW
action-core-develop-folder = Develop the whole folder in the background (each image in the software of its development)
folder-menu-develop = Develop the whole folder…
folder-menu-open-in = Open in { $name }
folder-open-in-missing = { $name } cannot open a folder, or is not installed
folder-open-in-running = { $name } is already open: close it first (on Windows, a folder cannot be passed to it then)
develop-batch-unavailable = Darktable not found: setting [develop] darktable
develop-batch-busy = a folder is already being developed
develop-batch-progress = developing "{ $name }" { $done }/{ $total }
develop-batch-done = "{ $name }" developed: { $rendered } rendered, { $cached } already done, { $failed } failed
develop-batch-error = { " " }({ $error })
develop-batch-cancelled = { " (interrupted)" }
develop-batch-cancel = Interrupt the development
view-info-developed-none = { " · " }not developed yet ({ $keys })
develop-not-found = no development software found (Darktable, RapidRAW: [develop] settings)
develop-not-installed = { $engine } is not installed (or not found: [develop] settings)
view-back-grid = Back to the grid
grid-render-embedded = Camera JPEG
grid-render-latest = Latest development
grid-render-libraw = RAW render (LibRaw)
grid-render-libraw-short = LibRaw
grid-render-badge = Developed in { $engine }
grid-render-badge-libraw = RAW render (LibRaw): no development from this software
grid-render-tip = Thumbnail render
filter-settings-in-panel = Settings: "Filters" section of the right panel ({ $keys })
panel-section-histogram = Histogram
panel-section-filters = Filter
panel-section-grid = Grid
panel-section-exif = EXIF
panel-filter-sharp = Sharp
panel-filter-exposed = Well exposed
panel-filter-developed = Developed
panel-clip-over = Blown
panel-clip-under = Crushed
panel-grid-render = Thumbnail render
panel-grid-render-filtered = For now, the "Developed" filter chooses the render.
panel-grid-sort = Sort
panel-grid-stacks = Stacks
panel-grid-stacks-on = Stack automatically
panel-grid-stacks-tip = Similar photos in a row (burst, same framing) form a stack, folded under its best photo.
panel-grid-similarity = Similarity
panel-grid-unfold-all = Unfold all
panel-grid-fold-all = Fold all
panel-grid-grouping = Fine settings…
panel-grid-files = Files
panel-grid-files-allow = File actions
panel-grid-files-tip = Rename, copy, move and delete images and folders from tri. Forbidden by default: darktable, Lightroom, Capture One… expect their images where they are. [files] allow_actions
files-actions-off = File actions are off: tick "File actions" (right panel, Grid) or [files] allow_actions.
files-library-warning = tri acts on the files themselves: a software that keeps them in its library or catalogue (darktable, Lightroom, Capture One…) will no longer find them there.
slider-lenient = more lenient
slider-strict = stricter
filter-developed-none = not developed
filter-developed-engine = developed in { $engine }
filter-developed-tip = Development software: the images developed in one of the checked software, shown in the latest one; none checked: the images without development, in their LibRaw render
filter-developed-none-hint = None checked: not developed (LibRaw)
develop-with = Develop with
develop-with-choose = choose…
develop-with-libraw = RAW render (LibRaw)
develop-folder-title = Develop "{ $folder }"
develop-folder-each = Each image's own development (otherwise the RAW render)
develop-folder-only = All in { $engine } (the images it developed)
develop-folder-go = Develop
develop-stale = Development changed in { $engine }: { $keys } to render it again.
shown-image = Image

## Histogram

histogram-indicators = { $source } · headroom { $headroom } EV
histogram-zero-ev = 0 EV

## Offload

offload-history-unreadable = unreadable history: { $error }
offload-status = offloading "{ $name }" { $done }/{ $total }
offload-summary = "{ $name }": { $copied } copied, { $skipped } skipped, { $errors } errors
offload-summary-cancelled = , interrupted
offload-title = Offload "{ $name }"
offload-planning = listing the card…
offload-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
}
offload-known = , { $n } of which already offloaded
offload-already-there = , { $n } already in the destination
offload-to-copy = { " " }; { $n } to copy ({ $mb } MB).
offload-ignored = { $n ->
    [one] { $n } other file (video, camera file) is not copied.
   *[other] { $n } other files (videos, camera files) are not copied.
}
offload-root = Root folder
offload-session = Session folder
offload-session-tip = Suggested: YYYY-MM-DD_ (date of the oldest image), to complete ("_Self") or replace entirely; the year, if wanted, goes in the root folder.
offload-destination = Destination: { $path }
offload-raw-only = RAW only
offload-raw-only-tip = The camera JPEGs stay on the card.
offload-open-after = Open the folder when done
offload-previews = Compute the previews during the copy
offload-previews-tip = Thumbnails, camera JPEGs, histograms, sharpness and grouping of each photo as soon as it is copied: the session is ready sooner.
offload-eject-after = Eject the card when done
offload-delete-after = Delete copied and verified files from the card
offload-delete-after-tip = Those copied and verified during this offload, and those already identical in the destination. Left: errors, images left behind, videos and camera files.
offload-delete-left = Also delete the JPEGs left on the card
offload-delete-left-tip = They are not copied: they will be lost.
offload-start = Offload
offload-progress = { $done } / { $total } files · { $mb } / { $total_mb } MB
offload-background = Continue in the background
offload-interrupt = Interrupt
offload-done = { $verified ->
    [yes] { $copied } files copied and verified
   *[no] { $copied } files copied
}, { $skipped } skipped (already offloaded), { $errors } errors.
offload-done-cancelled = { " " }— interrupted
offload-left = { $n } JPEGs left on the card.
offload-deleted = Copied files deleted from the card.
offload-not-deleted = { $n } files could not be deleted.
offload-ejecting = Ejecting the card: see the status bar.
offload-open = Open the folder
offload-eject = Eject the card

## Keys and shortcuts (shown, and read in [keys] in every language)

keys-error = [keys] { $id }: { $error }
keys-conflict = [keys] "{ $spec }" assigned to { $first } and to { $second }: kept for { $first }
keys-replaces = [keys] "{ $spec }" ({ $action }) replaces the shortcut of { $other }
keys-unknown-key = unknown key "{ $text }"
keys-empty = empty shortcut
key-shift-prefix = Shift+
key-leader = <leader>
key-sequence-then = { " then " }
key-arrow-left = Left arrow
key-arrow-right = Right arrow
key-arrow-up = Up arrow
key-arrow-down = Down arrow
key-home = Home
key-end = End
key-page-up = Page up
key-page-down = Page down
key-escape = Esc
key-enter = Enter
key-space = Space
key-delete = Delete
key-backspace = Backspace

## Settings

settings-bad-color = colour "{ $value }": expected "#rrggbb"

## Sharpness score

progress-sharpness = sharpness
progress-groups = grouping
filter-bar-no-blurry = without blurry
filter-bar-well-exposed = well exposed
filter-bar-inverted = inverted
filter-bar-count-tip = images shown / images in the folder
filter-bar-more = Sort and number of images (the bar lacks room)
badge-blurry = Judged blurry (sharpness score)
badge-blurry-view = Judged blurry (sharpness score); click: sharpness map
panel-sharpness = Sharpness
panel-sharpness-value = { $score } (× { $folder } the folder, × { $neighbours } the neighbours)
panel-sharpness-value-stack = { $score } (× { $folder } the folder, × { $stack } the stack)
panel-sharpness-blurry = { " " }· blurry

## Histogram statistics

stats-ev = { $value } EV
stats-percent = { $value } %
stats-red = Red
stats-green = Green
stats-blue = Blue
stats-median = Median
stats-mean = Mean
stats-range = Range ({ $low } % to { $high } %)
stats-range-value = { $low } to { $high }
stats-spread = Std. dev.
stats-over = Clipped
stats-under = Crushed
stats-raw-luminance-short = RAW: luminance per block of the mosaic · EV below saturation
stats-raw-luminance = RAW data: each photosite records a single colour; the luminance is computed on each block of the mosaic holding all three (2 × 2 in Bayer, 3 × 3 in X-Trans), with the shot's white balance and the sensor's colours. Values in EV below clipping.
stats-luminance = Luminance
stats-all = All channels
histogram-channel-tip = { $channel } ({ $key }; { $keys }: next channel)

# Hidden search keywords (help, settings: actions::keywords), per setting,
# section or action; added to what is searched.
kw-gui = interface window appearance
kw-gui-grid = thumbnails grid contact sheet
kw-gui-view = full screen viewer image
kw-gui-zoom = magnify loupe 100% enlarge
kw-gui-histogram = histogram exposure clipping blown highlights shadows
kw-gui-sharpness = sharpness focus peaking blur
kw-gui-colors = colours colors theme
kw-gui-folders = folders tree left panel
kw-gui-metadata = exif metadata information right panel
kw-gui-dialogs = dialogs windows modal
kw-gui-crop = crop frame handles
kw-gui-compare = comparison side by side burst
kw-keys = shortcuts keyboard keys bindings
kw-preview = previews cache size render
kw-cache = disk space clear
kw-xmp = sidecar ratings stars darktable lightroom capture one
kw-offload = sd card import copy offload ingest
kw-export = export jpeg tiff output
kw-files = files trash delete rename editor
kw-sharpness = sharpness blur score
kw-similarity = stacks groups bursts similar
kw-i18n = language french english translation
kw-update = update version new release
kw-develop = darktable development render
kw-places = volumes disks cards places eject
kw-watch = watch new files
kw-exposure = exposure histogram blown blocked
kw-gui-profile = vim nvim hjkl shortcut set keyboard classic arrows
kw-gui-leader = tmux prefix panels ctrl+b
kw-gui-theme = dark light neutral grey gray lightroom night mode
kw-gui-auto-advance = next automatically after rating
kw-gui-reopen-last = last folder startup launch
kw-gui-key-sequence-timeout-ms = delay sequence gg
kw-gui-raw-previews = raw render background
kw-gui-filter-bar = filter bar
kw-gui-rating-bar = rating bar stars mouse
kw-gui-history = undo history
kw-gui-grid-cell = thumbnail size
kw-gui-grid-info = name format badges caption hide
kw-gui-grid-rejected-opacity = rejected transparency faded
kw-gui-view-stack-wrap = loop stack wrap
kw-gui-view-focus-point = focus point autofocus af
kw-gui-zoom-max = 400% maximum
kw-gui-zoom-loupe-key = loupe key
kw-gui-histogram-badge-over = blown overexposed threshold badge
kw-gui-histogram-badge-under = blocked underexposed threshold badge
kw-gui-sharpness-colors = peaking colours colors
kw-i18n-language = language french english français
kw-xmp-sidecar = darktable lightroom capture one bridge convention
kw-xmp-multiple-colors = several colours labels
kw-files-trash = trash permanent delete
kw-files-editor = photoshop gimp affinity open with
kw-files-pairs = raw+jpeg pairs together
kw-offload-destination = destination folder root import
kw-offload-eject-after = eject card after
kw-offload-delete-after = erase card clear
kw-develop-darktable = darktable-cli path appimage flatpak
kw-develop-rapidraw = rapidraw path appimage flatpak export
kw-develop-default = preferred default software darktable rapidraw auto
kw-cache-zoom-max-mb = disk size 100% 1:1
kw-preview-standard = preview size full screen resolution
kw-render-auto-bright = automatic brightness
kw-render-camera-wb = white balance camera
kw-sharpness-blur-ratio = blurry threshold severity
kw-similarity-threshold = stacks threshold grouping
kw-similarity-max-gap-s = time gap burst seconds
kw-update-url = github releases
kw-places-fixed = fixed disks eject
kw-places-exclude = hide volumes exclude
kw-places-auto-mount = mount automatically card key device
kw-export-camera-suffix = suffix camera jpeg pair
kw-core-app-settings = preferences configuration options
kw-core-app-help = shortcuts help keys
kw-core-app-check-update = update version
kw-core-app-report-issue = bug report issue github error
kw-core-view-zoom = 100% 1:1 magnify
kw-core-view-source = camera jpeg raw render toggle
kw-core-view-peaking = sharpness focus peaking
kw-core-view-clipping = blown blocked zones exposure warning
kw-core-view-histogram = histogram exposure
kw-core-view-overlay = grid thirds golden ratio overlay
kw-core-view-crop = crop version story instagram
kw-core-view-compare = side by side burst developments
kw-core-view-focus-mode = full screen focus
kw-core-view-focus-point = focus point autofocus af
kw-core-view-grid-info = name format badges hide caption thumbnails
kw-core-file-delete = delete trash remove
kw-core-file-delete-photo = delete photo developments trash
kw-core-file-move = move
kw-core-file-copy = copy duplicate
kw-core-file-export = export jpeg tiff output
kw-core-file-reveal = finder explorer nautilus file manager
kw-core-file-edit = photoshop gimp external editor open with
kw-core-edit-undo = undo
kw-core-edit-redo = redo
kw-core-filter-blurry = blurry sharpness
kw-core-filter-well-exposed = exposure blown blocked sun
kw-core-filter-invert = invert opposite
kw-core-filter-developed = developed development darktable rapidraw software
kw-core-flag-accept = pick chosen keep
kw-core-flag-reject = reject discard
kw-core-select-visual = vim visual mode
kw-core-stack-toggle = stacks bursts group
kw-core-dev-fold-all = developments fold all stacks
kw-core-dev-toggle = developments darktable rapidraw clones virtual copies stack
kw-core-dev-next = developments darktable rapidraw clones virtual copies
kw-core-dev-prev = developments darktable rapidraw clones virtual copies
kw-core-folders-offload = sd card import offload
kw-core-folders-eject = eject card disk
kw-core-search = search find file name
kw-core-panel-next = tmux panels leader
kw-plugin-airdrop-send = send iphone share
kw-core-develop-render = developed render darktable rapidraw
kw-plugin-darktable-develop = darktable developed render
kw-plugin-rapidraw-develop = rapidraw developed render
kw-plugin-darktable-open-folder = darktable open folder lighttable film roll
kw-core-view-background = background colour black white grey lights
kw-core-view-image-info = info focus full screen label
kw-core-folders-favorite-name = rename name favourite label
kw-core-folders-connect = server network share nas smb unc connect mount
kw-core-folders-forget = server network share nas forget remove
kw-core-grid-render = render thumbnails developed development darktable rapidraw libraw jpeg camera grid
kw-core-filter-sort = sort order date name chronological
kw-core-filter-sort-reverse = sort order reverse descending ascending

kw-core-filter-stars-op = stars comparison exactly at most less equal
remote-title = Copy "{ $name }" locally
remote-why = The system cannot open a { $protocol } server directly ({ $address }): its images can be copied to this computer, then sorted here.
remote-scanning = reading the server folder…
remote-summary = { $images } images, { $size } to copy
remote-destination = Into the folder:
remote-notes-local = Only the images and their XMP sidecars are copied; ratings set afterwards stay on the copy, not on the server.
remote-start = Copy
remote-background = In the background
remote-progress = Copying "{ $name }": { $done } / { $total } files · { $size } / { $total_size }
remote-progress-short = { $done } / { $total } files · { $size } / { $total_size }
remote-done = "{ $name }" copied locally
remote-scan-failed = Cannot read "{ $name }": { $error }
remote-copy-failed = Copy of "{ $name }" interrupted: { $error }
size-mb = { $value } MB
size-gb = { $value } GB
size-tb = { $value } TB
decimal-separator = .
