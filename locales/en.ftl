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
      tri <folder> --sharpness [--raw|--flat] [--depth N]
      tri --offload <card> [--to <root>] [--folder <name>] [--raw-only] [--dry-run] [--previews] [--delete-copied [--delete-left]] [--eject]
      tri --rename <image> <name> [--dry-run]
      tri --delete <image>... [--permanent] [--dry-run]
      tri --copy|--move <image>... --to <folder> [--dry-run]
      tri --reveal <image>
      tri --edit <image>...
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
cli-cache-cleared = cache cleared ({ $level })
cli-cache-all = all
cli-group = GROUP { $index } ({ $n } images)

## Sharpness (command line)

cli-sharpness-header = file	score	× folder	× neighbours	verdict	time
cli-sharpness-blurry = BLURRY
cli-sharpness-summary = { $n } images, median { $median }: { $blurry } blurry (below { $ratio } × the folder median, or { $neighbour_ratio } × that of the { $neighbours } neighbours on each side)
