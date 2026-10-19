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
action-core-filter-cycle = Type filter: all, RAW, flat
action-core-filter-stars-0 = Filter: any stars
action-core-filter-stars-1 = Filter: at least 1 star
action-core-filter-stars-2 = Filter: at least 2 stars
action-core-filter-stars-3 = Filter: at least 3 stars
action-core-filter-stars-4 = Filter: at least 4 stars
action-core-filter-stars-5 = Filter: 5 stars
action-core-filter-flag-accepted = Filter: picked only
action-core-filter-flag-not_rejected = Filter: hide rejected
action-core-filter-flag-rejected = Filter: rejected only
action-core-filter-color-red = Filter: red (add / remove)
action-core-filter-color-yellow = Filter: yellow (add / remove)
action-core-filter-color-green = Filter: green (add / remove)
action-core-filter-color-blue = Filter: blue (add / remove)
action-core-filter-color-purple = Filter: purple (add / remove)
action-core-filter-clear = Clear all filters
action-core-filter-blurry = Filter: hide blurry
action-core-filter-only_blurry = Filter: blurry only
action-core-view-filter_bar = Show / hide the filter bar
action-core-view-rating_bar = Show / hide the rating bar
action-core-rate-auto_advance = Auto-advance after rating: on / off
action-core-select-toggle = Select / deselect the image
action-core-select-visual = Visual mode: select a range (v to confirm)
action-core-view-compare = Compare the selected images side by side
action-core-view-peaking = Show / hide sharpness (sharp edges, at 1:1)
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
search-folder-tip = Search a folder ({ $keys }): among the subfolders of the pointed one; “~/…” from the home folder, “/…” from the root or a typed path. ↑ ↓ choose, Enter opens, Esc closes.
search-folder-in = in { $base }
search-folder-running = searching…
search-folder-none = no folder found
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
action-core-file-export = Export…
action-core-file-delete_stack = Delete the whole stack
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
action-core-stack-choose = Make it the stack cover (its pick)
stack-chosen = { $name }: cover of its stack
stack-position = stack: { $n } / { $total }
stack-slider-tip = Similarity: to the right, larger stacks; while adjusting, stacks are expanded ({ $keys }: fine settings)
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
action-core-metadata-open = Metadata: open (large histogram)
action-core-metadata-select_up = Metadata: extend the selection up
action-core-metadata-select_down = Metadata: extend the selection down
action-core-metadata-copy_all = Metadata: copy everything
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
histogram-photo-tip = Analysis photo (clipped highlights / shadows on the grey image): show / hide ({ $keys })
action-core-histogram-all = Large histogram: all channels
action-core-view-clipping = Show / hide clipped highlights and shadows
action-core-view-metadata = Show / hide the metadata panel
action-core-view-open = Open full screen
action-core-view-zoom = Zoom 100 % / fit
action-core-view-zoom_in = Zoom: next step up
action-core-view-zoom_out = Zoom: next step down
action-core-view-zoom_fit = Zoom: fit (in the grid and the folders, / searches)
action-core-view-zoom_100 = Zoom: 100 %
view-info-zoom = { " · " }{ $percent } %
view-loupe-loading = loading 100 %…
action-core-view-source = Switch camera JPEG / RAW render
action-core-view-pan_left = Zoomed image: move left
action-core-view-pan_right = Zoomed image: move right
action-core-view-pan_up = Zoomed image: move up
action-core-view-pan_down = Zoomed image: move down
action-core-view-back = Close / go back
action-core-app-help = Show or hide this help
action-core-app-settings = Settings
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

## Panels, ejection, ratings

panel-no-image = No image selected.
histogram-enlarge = Enlarge ({ $keys })
histogram-computing = histogram being computed
panel-reading = reading…
folders-title = Folders
folders-refresh = Refresh
folders-refresh-tip = Re-read volumes and folders (F5). A plugged-in drive also shows up by itself within a few seconds.
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
eject-failed = "{ $name }" not ejected: { $error }
cache-unavailable = cache unavailable: { $error }

## Comparison, full screen, status bar

rating-not-saved = rating not saved: { $error }
compare-needs-two = compare: select at least 2 images (Space, v, Ctrl+A, Shift+click)
compare-limited = comparison limited to the first { $max } ([gui.compare] max)
view-info-selected = { " " }· selected
view-info-peaking = { " " }· sharpness
view-info-peaking-computing = { " " }· computing sharpness
view-info-peaking-one-to-one = { " " }· sharpness: at 1:1 (z)
view-info-sensor = { " " }({ $percent } % of the sensor)
view-info-preview-computing = { " " }· preview being computed
view-info-raw-pending = { " " }· RAW render in progress
view-info-full-computing = { " " }· computing 100 %
view-info-one-to-one-loading = { " " }· loading 1:1
view-hint-raw = b: camera JPEG / RAW   z: 1:1   i: histogram   w: zones   Esc: grid
view-hint-flat = z: 1:1   i: histogram   w: zones   Esc: grid
status-no-folder = no folder
status-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
} ({ $raws } RAW) · filter: { $filter }
status-selected = { $n } selected
status-visual-mode = { " " }· visual mode
progress-thumbnails = thumbnails
progress-embedded = camera JPEG
progress-previews = previews
progress-histograms = histograms
status-key-problems = { $n ->
    [one] shortcuts: { $n } problem
   *[other] shortcuts: { $n } problems
}
status-invalid-config = invalid configuration: default settings
status-show-offload = Show the offload
status-retry-eject = Retry ejecting
status-dismiss = dismiss
status-open-session = Open the session "{ $name }"

## Dialogs, rating bar, filters, help

close = Close
histogram-scale-ev = in EV
histogram-scale-linear = linear
histogram-scale-tip = Scale { $current }; click or { $keys }: { $other }
or-escape = or Esc
rating-bar-title = Ratings
rating-bar-applies = ratings apply to the { $n } selected images
filter-summary-stars = { $n ->
    [5] 5 stars
    [one] { $n } star and more
   *[other] { $n } stars and more
}
filter-summary-or = { " or " }
filter-all = all
filter-flat = flat
filter-bar-all = All
filter-bar-flat = Flat
filter-bar-clear = Clear
filter-bar-count = { $shown }/{ $n }
badge-over = { $percent } % of pixels clipped (blown)
badge-under = { $percent } % of pixels crushed
help-title = Shortcuts
help-search-hint = Search (e.g. "fpn", "star 3")…
help-no-match = No matching action.
help-close-hint = Esc or F1 to close.
empty-no-folder = Choose a folder in the left panel (Shift+D shows it).
empty-folder = No image in this folder.
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
export-profile = Export profile
export-profile-hint = up / down arrows: profile · profiles in the settings (Ctrl+,), Export section
export-destination = Folder (Tab completes)
export-create = will be created
export-folder-hint = /… absolute · ~/… home folder · otherwise relative to each image (empty or “.”: next to it)
export-goes-to = Destination: { $dest }
export-how = Developed by darktable from their .xmp (without a development: darktable's defaults). Nothing is overwritten.
export-how-libraw = Basic LibRaw rendering (camera white balance, no adjustments), as with the b key; non-RAW images in sRGB only. Nothing is overwritten.
export-no-darktable = darktable missing: basic LibRaw rendering.
export-renderer = Rendering:
export-renderer-libraw = LibRaw (basic)
export-renderer-embedded = Camera JPEG
export-how-embedded = The camera JPEG (the RAW's embedded preview, at its size), in sRGB; non-RAW images as they are. Nothing is overwritten.
export-button = Export
export-profile-edit = Editing
export-profile-web = Web
export-format-jpeg = JPEG { $quality }%
export-format-tiff = TIFF { $bits }-bit
export-format-png = PNG { $bits }-bit
export-color-image = development's color space
export-color-linear = { $name } (linear)
export-size-full = full resolution
export-size-max = { $side } px max
export-busy = An export is already running.
export-done = { $n ->
    [one] 1 image exported to { $dest }
   *[other] { $n } images exported to { $dest }
}
export-partial = { $n } exported, { $failed } failed (unreadable by darktable?), to { $dest }
export-cancelled = Export stopped (files already written are kept).
export-progress = Export “{ $profile }” { $done }/{ $total }
export-cancel = Stop the export
rename-failed = cannot rename: { $error }
transfer-copy-title = { $n ->
    [one] Copy { $n } image to…
   *[other] Copy { $n } images to…
}
transfer-move-title = { $n ->
    [one] Move { $n } image to…
   *[other] Move { $n } images to…
}
transfer-files = { $n ->
    [one] { $n } file (pairs and XMP sidecars included)
   *[other] { $n } files (pairs and XMP sidecars included)
}
transfer-shared = { $name } is shared with an image that stays: copied
transfer-create = the folder will be created
transfer-copy = Copy
transfer-move = Move
transfer-create-copy = Create and copy
transfer-create-move = Create and move
transfer-hint = Tab: complete · up / down arrows: recent destinations
transfer-recent = Recent
transfer-progress = { $done } / { $total } files
transfer-failed = copy / move failed: { $error }
groups-title = Grouping of similar images
groups-enabled = Group into stacks
groups-threshold = Difference allowed between neighbouring images
groups-threshold-tip = Higher: larger stacks; lower: only bursts.
groups-gap = Maximum time gap
groups-gap-tip = Two images taken further apart are never grouped (0: no limit).
groups-summary = { $images } images: { $stacks } stacks, { $groups } groups in all
groups-computing = signatures: { $done } / { $total }
groups-default = Default threshold
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
folders-not-removable = “{ $name }” is not a removable drive
offload-source = Card:
folders-no-removable = no removable drive to offload
settings-title = Settings
settings-search-hint = Search a setting (name, section, description)…
settings-file = file: { $path }
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
settings-keys-default = Back to the profile's shortcuts
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
settings-section-develop = darktable rendering
settings-section-export = Export
settings-section-export-profiles-edit = Export profile “Editing”
settings-section-export-profiles-web = Export profile “Web”
settings-section-watch = Folder watching
settings-section-places = Places
settings-section-exposure = Exposure
settings-section-offload = Card offload
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
folder-new-title = New folder in “{ $name }”
folder-new-button = Create
folder-rename-title = Rename the folder “{ $name }”
folder-delete-trash = Move the folder “{ $name }” to the trash?
folder-delete-permanently = Permanently delete the folder “{ $name }”?
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
folders-place-root = “{ $name }” is a place: it cannot be renamed or deleted
folder-menu-new = New folder…
folder-menu-rename = Rename…
folder-menu-delete = Delete…
folder-menu-offload = Offload the card…
folder-menu-eject = Eject
folder-menu-offload-here = Offload a card here…
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
folders-favorite = Favourite
folders-favorite-added = “{ $name }” added to favourites
folders-favorite-removed = “{ $name }” removed from favourites
folders-favorite-error = Favourites not saved: { $error }

## What is shown

shown-thumbnail = Thumbnail (preview being computed)
shown-embedded = Camera JPEG
shown-raw = RAW render
shown-raw-full = RAW render · 100 %
shown-embedded-full = Camera JPEG · 1:1
shown-image-full = Image · 100 %
shown-exposure = Exposure analysis
shown-developed = Developed in darktable
view-info-developed-pending = { " · " }darktable development in progress…
action-plugin-darktable-develop = Develop in darktable (the shown image)
view-info-developed-none = { " · " }not developed yet ({ $keys })
develop-not-found = darktable not found ([develop] darktable setting)
develop-stale = Development changed in darktable: { $keys } to render it again.
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
filter-bar-only-blurry = blurry only
filter-bar-count-tip = images shown / images in the folder
badge-blurry = blurry
panel-sharpness = Sharpness
panel-sharpness-value = { $score } (× { $folder } the folder, × { $neighbours } the neighbours)
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
stats-raw-luminance-short = RAW: no luminance, green stands in · EV below saturation
stats-raw-luminance = RAW data: each photosite records a single colour, so there is no luminance; green is the best approximation. Values in EV below clipping.
stats-all = All channels
histogram-channel-tip = { $channel } ({ $key }; { $keys }: next channel)
