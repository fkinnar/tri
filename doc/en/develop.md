# Module `tri::develop`: rendering by a development software

The user's idea (2026-10-19): see your photos in `tri` **as they were developed**
elsewhere, darktable first. The render is **on request**: `Shift+B` or the button
(aperture) in the strip below the image. After that, `b` goes from the camera JPEG to the
RAW render, then to the **developed** one, and back to the camera JPEG; as long as the
image has not been developed, `b` only offers camera JPEG and RAW and never launches
darktable (user feedback, 2026-10-19: when culling, the basic render is almost always
enough, no point waiting 10 s for nothing).

## A compiled plugin driving an external program

Like the other plugins (overlays, sending), the code is in Rust and compiled with `tri`;
it calls `darktable-cli` as a **separate process**: darktable cannot bring `tri` down,
and its license remains its own. RawTherapee (`rawtherapee-cli`, `.pp3` profiles) and ART
would follow the same model.

- `find_darktable(réglage)`: `[develop] darktable` = `"auto"` (the `PATH`, then the usual
  locations: `/Applications/darktable.app/…` on macOS, `/usr/bin` on Linux,
  `Program Files` on Windows), a path, or `""` (never). On Linux, if it is not found: a
  darktable **AppImage** (`*darktable*.appimage` in `~/AppImages`, `~/Applications`,
  `~/.local/bin`, `~/bin`, the downloads folder, `/opt`; the most recent one), launched
  with `darktable-cli` as first argument: the AppImage then runs that binary instead of
  the interface (documented by darktable, "invocation per OS"). That is the user's
  installation on Mint (`~/AppImages/darktable.appimage`, 2026-10-19): **checked by him**
  the same day, rendering and export work. An AppImage path is also accepted in the
  setting. Otherwise, the **Flatpak** `org.darktable.Darktable` (system or user
  installation, `flatpak info`), launched with `flatpak run --command=darktable-cli
  org.darktable.Darktable` (`Cli`: program and arguments placed before `tri`'s);
  `"flatpak"` selects it outright. Observed on 2026-10-19 on Linux Mint: the Software
  Manager installs darktable as a Flatpak, with no `darktable-cli` in the `PATH` (and its
  exported launcher opens the interface, not `darktable-cli`). The Flatpak must be able to
  read `tri`'s images and cache (home folder): to be checked for images outside the home
  folder.
- `darktable_sidecar(image)`: `IMAGE.RAF.xmp`, **only if it carries an actual
  development** (`darktable:history_end` > 0): a sidecar written by `tri` alone (notes)
  has none.
- `Darktable::render`: `darktable-cli image sidecar sortie --width --height --hq
  --core --configdir <cache>/darktable --library :memory: --conf
  write_sidecar_files=never`, cancellable (the program is stopped).

## Nothing is written in the user's files

Configuration specific to `tri` (in its cache), library in memory: neither darktable's
configuration nor its library is touched, and darktable can stay open (no lock).
`write_sidecar_files=never`: the sidecar is never rewritten, whatever the configuration;
a test checks this on a real render (on a copy).

## Cache and computation

Level `Level::Developed`: darktable's JPEG, at the standard preview size, stored **as is**
in the cache. Its key includes the sidecar's fingerprint (size, date): changing the
development in darktable redoes the render. In the interface, a developed image already
displayed whose sidecar has changed (folder watching, or at the time of `b` / `Shift+B`)
is forgotten: `b` goes back to the basic render and `Shift+B` redoes it (feedback from
2026-10-19: a modified crop was not seen). Computed **on explicit request only**
(`Shift+B`, displayed image; an image already in the cache stays in the `b` rotation,
even from one session to the next), by the heavy worker (darktable occupies the whole
machine), never the whole folder in the background. Measured on 2026-10-19 (X-T4 RAF,
Apple Silicon Mac): 8 to 10 s per image, of which ~6 s is darktable's startup.

## Size of the developed image (2026-10-21)

`developed_size(xmp, size)`: the size of the image darktable would produce at full
resolution, for the resolution shown in full screen. Read from the sidecar's history
(`darktable:history`, entries before `darktable:history_end`, last state of each module,
main instance):

- **`crop`**: its parameters are four little-endian 32-bit floats in hexadecimal, left,
  top, right and bottom edges as fractions of the image (checked on real sidecars of the
  user, darktable 5, crop v3);
- **`flip`**: `-1` (`ffffffff`) follows the EXIF orientation; another value is a
  darktable orientation, width and height swapped if bit 4 is set;
- **`ashift`**, **`clipping`**, **`rotatepixels`**, **`scalepixels`** active: unknown
  size (`None`), rather than a wrong one; likewise for compressed parameters (`gz…`).

## In the interface

While waiting for the render, the RAW render (or the camera JPEG) with "darktable
development in progress…"; when the image has no development, the reason instead. No
developed 1:1: you zoom into the render. The focus point and the clipped / blocked areas,
placed on the whole sensor, are hidden on a developed image (cropped). Without darktable,
the source is not offered and `Shift+B` says so.

Coming: `darktable-cli` can process a folder in a single launch (amortized startup) — for
sending developed JPEGs in the "after-development" stage.
