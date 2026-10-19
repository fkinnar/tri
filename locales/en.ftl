# Engine and command-line catalogue — English.
# Every identifier must also exist in fr.ftl (checked by the tests).

images-count = { $n ->
    [one] { $n } image
   *[other] { $n } images
}

# General messages
warning-prefix = warning: { $message }

## Metadata

metadata-unreadable = unreadable file: { $error }
metadata-camera = Camera
metadata-lens = Lens
metadata-exposure = Exposure
metadata-bias = Compensation
metadata-bias-value = { $value } EV
metadata-mode = Mode
metadata-focal = Focal length
metadata-focal-equivalent = { $focal } mm (35 mm equiv.: { $equivalent } mm)
metadata-focal-value = { $focal } mm
metadata-flash = Flash
metadata-flash-fired = fired
metadata-flash-not-fired = did not fire
metadata-date = Date
metadata-date-value = { $year }-{ $month }-{ $day } { $time }
metadata-dimensions = Dimensions
metadata-dimensions-value = { $width } × { $height } ({ $mpx } MP)
metadata-file = File
metadata-size-kb = { $size } kB
metadata-size-mb = { $size } MB
program-manual = Manual
program-normal = Program
program-aperture = Aperture priority
program-shutter = Shutter priority
program-creative = Creative (depth of field)
program-action = Action (fast shutter)
program-portrait = Portrait
program-landscape = Landscape

## Errors

raw-init-failed = libraw_init failed
raw-invalid-path = path cannot be passed to LibRaw: { $path }
raw-libraw-error = LibRaw error { $code }: { $message }
raw-cancelled = LibRaw processing cancelled
raw-empty-image = LibRaw returned an empty image without an error code
raw-no-mosaic = RAW without a colour filter mosaic (linear DNG, Foveon)
raw-unsupported-image = unsupported image (type { $kind }, { $colors } channels, { $bits } bits)
flat-read-failed = cannot read: { $error }
flat-decode-failed = cannot decode: { $error }
flat-heif-not-compiled = HEIF not supported (built without "heif")
flat-heif-failed = cannot decode HEIF: { $error }
flat-heif-layout = libheif did not return 8-bit RGB
files-invalid-name = invalid name: "{ $name }"
files-exists = { $path } already exists
files-trash-failed = cannot move to trash: { $error }
files-launch-failed = cannot launch "{ $program }": { $error }
files-not-a-directory = { $path } is not a folder
files-same-folder = the images are already in { $path }
files-mismatch = copy of { $path } differs from the original (nothing was kept)
files-not-in-trash = { $path } is no longer in the trash
history-notes-changed = { $name }: ratings changed elsewhere since (nothing was changed)
history-missing = { $name } cannot be found (moved or deleted elsewhere)
history-modified = { $name } was modified elsewhere since (nothing was changed)
history-nothing = nothing to undo
history-trash-one = { $name } moved to the trash
history-create-dir = folder { $name } created
history-version-created = version “{ $version }” created: { $name }
history-version-changed = version “{ $version }” changed: { $name }
history-version-deleted = version “{ $version }” deleted: { $name }
history-stars = { $n ->
    [one] 1 star
   *[other] { $n } stars
}
history-accepted = accepted
history-rejected = rejected
history-no-flag = no flag
history-color-added = { $color }
history-color-removed = no { $color }
history-red = red
history-yellow = yellow
history-green = green
history-blue = blue
history-purple = purple
history-notes-one = { $what }: { $name }
history-notes-many = { $what }: { $n } images
history-rename = { $from } renamed to { $to }
history-copy = { $n ->
    [one] 1 image copied to { $dest }
   *[other] { $n } images copied to { $dest }
}
history-move = { $n ->
    [one] 1 image moved to { $dest }
   *[other] { $n } images moved to { $dest }
}
history-trash = { $n ->
    [one] 1 file moved to the trash
   *[other] { $n } files moved to the trash
}
config-invalid-toml = invalid TOML value: { $value }
config-not-a-section = “{ $section }” is not a section
preview-io = cannot access the file: { $error }
preview-no-embedded = no usable embedded preview
preview-cancelled = computation cancelled
preview-not-an-image = this level is not an image
exposure-unreadable = unreadable exposure data
cache-no-user-dir = user cache folder not found
xmp-io = XMP sidecar: { $error }
xmp-malformed = malformed XMP sidecar (left unchanged): { $error }
xmp-no-description = XMP sidecar without rdf:Description (left unchanged)
settings-auto-or-integer = "auto" or a positive integer
offload-no-data-dir = data folder not found (no HOME)
offload-unnamed-file = file without a name
offload-verify-failed = the copy read back differs from the original (faulty medium?)
offload-copy-missing = copy missing or different: kept
places-home = Home folder
places-root = Root (/)
settings-defaults-used = { $warning }; default settings used
settings-file-error = configuration { $path }: { $error }

## Command line

cli-usage = 
    usage:
      tri <folder> [--raw|--flat] [--depth N]
      tri <folder> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N] [--workers N] [--mixed] [--cache FOLDER]
      tri --clear-cache [thumbnail|embedded|standard|full|exposure] [--cache FOLDER]
      tri --cache-usage [--cache FOLDER]
      tri <folder> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
      tri <folder> --exposure [--raw|--flat] [--depth N]
      tri <folder> --metadata [--raw|--flat] [--depth N]
      tri <folder> --groups [--threshold X]
      tri <folder> --sharpness [--raw|--flat] [--depth N]
      tri --offload <card> [--to <root>] [--folder <name>] [--raw-only] [--dry-run] [--previews] [--delete-copied [--delete-left]] [--eject]
      tri --rename <image> <name> [--dry-run]
      tri --delete <image>... [--permanent] [--dry-run]
      tri --copy|--move <image>... --to <folder> [--dry-run]
      tri --reveal <image>
      tri --edit <image>...
      tri --export <profile> <image>... [--to <folder>] [--libraw|--embedded] [--passthrough] [--dry-run]
    common option: --config FILE (settings; default: the system location)
cli-unknown-convention = unknown convention: { $value } (darktable or adobe)
cli-unknown-option = unknown option: { $option }
cli-extra-argument = extra argument: { $argument }
cli-cache = cache: { $path }
cli-cache-level = { $entries } entries  { $mib } MiB  (average { $average } KiB)
cli-previews-start = { $n } images ({ $raws } RAW), thumbnails { $thumbnail } px, previews { $standard } px, { $workers } light workers + 1 heavy, { $order ->
    [thumbnails] thumbnails first
   *[mixed] mixed
}
cli-previews-progress = thumbnails { $thumbnails }   camera JPEG { $embedded }   histograms { $histograms }   previews { $previews } { $percent } %   { $seconds } s
cli-thumbnails-done = thumbnails done in { $time }
cli-embedded-done = camera previews done in { $time } (the whole folder can be browsed)
cli-all-done = done in { $time } ({ $ms } ms per image on average)
cli-failure = failure: { $error }
cli-trimmed = 1:1 limit: { $n } old entries removed ({ $mb } MB)
cli-offload-plan = { $card }: { $n } images ({ $known } already offloaded), { $ignored } other files ignored, { $mb } MB to copy
cli-nothing-to-copy = nothing to copy
cli-destination = destination: { $path }
cli-already-there = { $n } images already in the destination (skipped if identical)
cli-to-copy = to copy
cli-offload-progress = { $done }/{ $total } files   { $mb }/{ $total_mb } MB
cli-error = error
cli-offload-done = { $verified ->
    [yes] { $copied } copied and verified
   *[no] { $copied } copied
}, { $skipped } skipped, { $left } left on the card, { $errors } errors, in { $time }
cli-deleted = { $n } files deleted from the card
cli-not-deleted = not deleted
cli-ejected = card ejected
cli-eject-failed = cannot eject: { $error }
cli-some-not-copied = some files were not copied
cli-exposure-header = file	source	headroom (EV)	clipped	crushed	time
cli-exposure-raw-source = RAW { $bits } bits, black { $black }, white { $white }
cli-error-detail = error: { $error }
cli-xmp-written = { $mode ->
    [dry-run] to write
   *[write] written
}
cli-xmp-conflict = conflict	{ $image }: ratings differ from the image sharing the same sidecar, skipped
cli-xmp-summary = { $n } images: { $written } sidecars { $mode ->
    [dry-run] to write
   *[write] written
}, { $unchanged } unchanged, { $conflicts } conflicts, { $errors } errors
cli-xmp-some-failed = some sidecars could not be converted
cli-rename-shared-copy = (copy: shared)
cli-delete-trash = trash:
cli-delete-permanent = deleted:
cli-delete-kept = kept (shared):
cli-dry-run = (--dry-run: nothing was changed)
cli-export-unknown-profile = unknown export profile: { $profile } (profiles: { $known })
cli-export-plan = export “{ $profile }” ({ $summary }) to { $dest }:
cli-export-libraw = darktable missing (or --libraw): basic LibRaw rendering
cli-export-as-is-raw = the “{ $profile }” profile only applies to already developed images (applies_to): a RAW has no color space, its rendering needs one (another profile, or --embedded for its camera JPEG)
cli-export-raw-only = the “{ $profile }” profile only applies to rendering a RAW (applies_to)
cli-export-passthrough = of which { $n } without recompression
cli-export-done = { $n ->
    [one] 1 file written
   *[other] { $n } files written
}
cli-export-failed = { $failed } images without a written file (unreadable by darktable?)
cli-groups-header = image	next	distance	gap (s)
cli-groups-summary = { $n } images, { $groups } groups at threshold { $threshold } ({ $ms } ms per signature)
cli-cache-cleared = cache cleared ({ $level })
cli-cache-all = all
cli-group = GROUP { $index } ({ $n } images)

## Sharpness (command line)

cli-sharpness-header = file	score	× folder	× neighbours	× stack	verdict	time
cli-sharpness-blurry = BLURRY
cli-sharpness-summary = { $n } images, median { $median }: { $blurry } blurry (below { $ratio } × the folder median, or { $neighbour_ratio } × that of the { $neighbours } neighbours on each side)

## Overlays
overlay-thirds = Thirds
overlay-golden = Golden ratio
overlay-spiral = Golden spiral
overlay-diagonals = Diagonals
overlay-bad-color = colour “{ $color }”: expected "#rrggbb"
overlay-bad-ratio = format “{ $ratio }”: expected width:height, for example "4:5"
overlay-bad-image = logo { $path } unreadable: { $error }

## Sending (plugins)
send-nothing = nothing to send
send-main-thread = cannot send outside the interface's main thread
send-unavailable = { $name } is not available on this machine
send-refused = { $name } refuses these files

## Rendering by a development program
develop-none = no darktable development for this image
develop-unavailable = darktable not found ([develop] darktable setting)
develop-failed = darktable: { $error }
export-bad-bits = export profile “{ $profile }”: { $bits } bits not possible in { $format }
export-bad-quality = export profile “{ $profile }”: quality { $quality } outside 1 to 100
export-bad-color = export profile “{ $profile }”: unknown color space “{ $value }”
export-bad-intent = export profile “{ $profile }”: unknown rendering intent “{ $value }”
export-needs-16-bits = export profile “{ $profile }”: “{ $color }” needs 16 bits or more (gradients break up in 8 bits), so 16-bit TIFF or PNG
export-libraw-color = export profile “{ $profile }”: the “{ $color }” color space needs darktable (without darktable: sRGB, Adobe RGB, ProPhoto, Rec709, Rec2020)
export-libraw-bits = export profile “{ $profile }”: 32-bit TIFF needs darktable
export-bad-icc = unreadable image color profile: no conversion possible (“JPEG as is” keeps it as it is)
export-version-empty = darktable rendered nothing for this version
