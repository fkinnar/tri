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
action-core-view-folders = Show / hide the folder panel
action-core-view-refresh_folders = Refresh folders (drive plugged in)
action-core-view-histogram = Show / hide the histogram
action-core-view-histogram_scale = Histogram: EV / linear
action-core-view-histogram_large = Large histogram
action-core-view-histogram_channel = Large histogram: next channel
action-core-view-clipping = Show / hide clipped highlights and shadows
action-core-view-metadata = Show / hide the metadata panel
action-core-view-open = Open full screen
action-core-view-zoom = Zoom 100 % / fit
action-core-view-source = Switch camera JPEG / RAW render
action-core-view-pan_left = Zoomed image: move left
action-core-view-pan_right = Zoomed image: move right
action-core-view-pan_up = Zoomed image: move up
action-core-view-pan_down = Zoomed image: move down
action-core-view-back = Close / go back
action-core-app-help = Show or hide this help
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
view-hint-raw = b: camera JPEG / RAW   z: 1:1   i: histogram   o: zones   Esc: grid
view-hint-flat = z: 1:1   i: histogram   o: zones   Esc: grid
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
histogram-linear-scale = Linear scale
histogram-ev-scale = EV scale
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
rename-failed = cannot rename: { $error }
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

## What is shown

shown-thumbnail = Thumbnail (preview being computed)
shown-embedded = Camera JPEG
shown-raw = RAW render
shown-raw-full = RAW render · 100 %
shown-embedded-full = Camera JPEG · 1:1
shown-image-full = Image · 100 %
shown-exposure = Exposure analysis
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
keys-shadowed = "{ $keys }" ({ $action }) never fires: it is the start of "{ $longer }"
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
stats-raw-luminance = RAW data: each photosite records a single colour, so there is no luminance; green is the best approximation. Values in EV below clipping.
stats-all = All channels
histogram-channel-tip = { $channel } ({ $keys }: next channel)
analysis-over = clipped
analysis-under = crushed
