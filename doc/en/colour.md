# Colour management (`tri::colour`)

*[Version française](../fr/colour.md)*

Issue #422, designed with the user on 2026-11-11, after a darktable render came out far
too saturated on a calibrated wide-gamut screen: `tri` did no colour management at all.
Every image was assumed to be sRGB, and on macOS the window was not even declared sRGB,
so the system sent the values to the screen unconverted.

## Principle

**Never assume, read the profile; convert once; let the screen do the rest.**

```
source (its own profile) ──convert once, when cached──▶ working space ──▶ screen
```

### The working space

Everything `tri` shows (thumbnails, previews, 1:1) is in one **working space**:
**Rec.2020 primaries, sRGB curve, D65 white** (`colour::working`, its ICC
`colour::working_icc`).

- **Wide.** It contains Display P3, Adobe RGB, and the wide-gamut screens that go beyond
  both. The user's Eizo, wider than Adobe RGB but not P3, ruled out P3 as a working space.
- **Independent of the screen.** The cache serves every screen, and a window moved to
  another screen recomputes nothing.
- **Same greys as sRGB.** It shares sRGB's curve and white, so egui's interface, drawn in
  sRGB, keeps its brightness.
- **Safe in 8 bits.** The cache is 8-bit JPEG. A smooth sRGB gradient sent through the
  working space and back to sRGB (two roundings, the Linux / Windows case) moves by at
  most 2 levels, ±1 almost everywhere (measured 2026-11-11, test
  `eight_bit_round_trip_stays_within_two_levels`).

### The sources

| Source | Its space | Converted |
|---|---|---|
| RAW rendered by LibRaw | asked directly in the working space (`OutputSpace::WORKING`: LibRaw's Rec.2020 output, sRGB curve) | nothing to do |
| Camera JPEG (embedded preview) | its ICC, else its EXIF (Adobe RGB per DCF: `InteroperabilityIndex` R03, or `ColorSpace` 2), else sRGB | when cached |
| Flat image (JPEG, PNG, TIFF, WebP) | its ICC, else sRGB (EXIF for a JPEG) | when cached, **at its depth**: a 16-bit TIFF / PNG is converted in 16 bits, then rounded to 8 bits (no banding in a ProPhoto gradient) |
| HEIF / HEIC | its ICC, else NCLX (Display P3 for an iPhone), else sRGB | when cached |
| darktable, RawTherapee, ART, RapidRAW render | the ICC the tool embeds (the output profile of the history or `.pp3`, **never forced by `tri`**), else sRGB | **when read** from the cache (`Cache::get`): the tool's JPEG stays untouched in the cache, so no development is redone |

The source's profile is read by `colour::Origin` (also used by the export). sRGB and
Adobe RGB are recognised by their description, except linear profiles; any other ICC is
parsed by `moxcms`.

`tri` writes a JPEG among developed renders (the thumbnail-size copies of the grid, or the
reduction of a RawTherapee / ART render). Each one carries its profile: the working ICC for
a copy already converted, the tool's ICC for a reduction. This way nothing is converted
twice.

### The screen

- **macOS.** The window is declared in the working space (`display::declare_working`, the
  Metal layer's colour space from the working ICC). ColorSync converts to the screen
  profile, calibration included, and follows the window from one screen to another.
  wgpu resets the layer's colour space on every surface reconfiguration (a resize, for
  instance), so it is declared again at every frame. That costs two Objective-C messages
  when it is already set.
- **Linux, Windows.** The system converts nothing for applications. Each image is converted
  to the screen profile before it becomes a texture (`display::to_screen`, in the loading
  thread for large images). The profile comes from three places, in this order:
  1. `[gui] display_profile`, a `.icc` / `.icm` file (`~/…` accepted). It is a fallback
     when detection fails, and useful for tests.
  2. The system (`display::Screen::follow`):
     - **Linux**: the X11 `_ICC_PROFILE` property of the root window, published by colord
       (or `dispwin`) for the **main screen**. It is read with `x11rb` in pure Rust.
       Under Wayland it works only when the compositor publishes it to XWayland.
     - **Windows**: `GetICMProfileW` on the screen **the window is on**, the profile
       associated in "Colour Management". It is followed when the window moves to another
       screen (`MonitorFromWindow` at every frame, cheap).
  3. Otherwise sRGB.

  When the profile changes, every image texture is redone in place, like after an
  adjustment, without falling back to the thumbnail. A profile that is unreadable or
  unusable by `moxcms` is logged, and sRGB is used. The profile is read at start-up,
  when the setting changes, and when the window changes screen (Windows): a profile
  changed in colord while `tri` runs is seen at the next launch.

The interface (egui's colours, in sRGB) is not converted. On macOS, its greys are
unchanged, but its vivid colours (selection, colour labels) come out more saturated. The
user accepted this on 2026-11-11, to be adapted if it bothers.

### What stays on the real data

- **Exposure analyses**: histograms, clipping alerts, zones.
- **Sharpness scores and grouping fingerprints**: their cache keys are unchanged, so
  nothing is recomputed.
- **The camera JPEG's histogram** (`preview::camera_exposure`): it reads the embedded
  JPEG in its own space. In the converted preview, the camera's clipped values would no
  longer sit at 255.

### Viewing adjustments

They stay defined on **sRGB channels**, as before colour management: balance, mixer,
eyedropper, luminance. Each pixel goes from the working space to linear light with sRGB
primaries (`working_to_srgb_linear`, exact 3×3 matrix; values below 0 or above 1 outside
the sRGB gamut), is adjusted, then goes back. See [adjust.md](adjust.md).

## What changed for the user

- **Colours** follow the source's profile and the screen's.
- **RAW renders are a little brighter, mostly in the shadows.**
  - LibRaw used to output the BT.709 curve (video), and `tri` showed it as sRGB, darker
    than the data: an 18 % grey came out at 104 instead of 118.
  - On a test RAF, the mean displayed luminance went from 29 to 44 out of 255.
  - The data are the same (0.5 level apart on average, measured 2026-11-11). The user
    chose the correct reading.
- **The cache is redone once.** The colour version enters the key of the image levels
  (`cs=1` in `preview::params`): thumbnails, camera previews, RAW renders, grid copies.
  Developed renders are not redone (converted when read), nor are analyses.
- **`tri --adjust`** writes its JPEGs with the working ICC embedded
  (`jpeg::with_icc`, without recompressing).

## Cost

Measured with `moxcms` 0.8 on one core, 2560 px image (4.4 Mpx):

| Conversion | Time |
|---|---|
| 8 bits | 6–7.5 ms |
| 16 bits | 18 ms |
| thumbnail | a fraction of a ms |
| decoding the same preview, for comparison | about 30 ms |

Each conversion is built once per pair of spaces and kept (`colour::to_working`,
`to_screen`).

## Tests

- `src/colour.rs`:
  - unchanged greys;
  - the same colour from sRGB, Adobe RGB and P3;
  - working ICC recognised;
  - linear profiles not taken for sRGB;
  - inverse matrices and Rec.2020 luminance;
  - 8-bit round trip;
  - 16-bit precision.
- `gui/src/display.rs`: a profile file read and described, a bad one refused; a wide
  screen gets less saturated values than an sRGB one.
- `tests/colour.rs` (synthetic charts written with their ICC):
  - the same colour from JPEGs in sRGB (with and without profile), Adobe RGB, P3 and
    ProPhoto;
  - a 16-bit ProPhoto PNG;
  - a ProPhoto developed render converted when read, and its grid copy not converted
    twice;
  - `with_icc`.

## Checking by hand: colour charts

For testers: [colour-check.md](colour-check.md) (step-by-step sheet, no build needed);
the JPEG and PNG charts are in [`doc/charts`](../charts).

```
cargo run --release --example colour_charts -- <output folder> [--side N] [--check-only]
```

It writes the **same** chart in sRGB, Adobe RGB, Display P3, ProPhoto, Rec.2020 and the
**linear** ProPhoto / Rec.2020 of darktable, each with its ICC embedded: 8-bit JPEG,
16-bit TIFF, 8- and 16-bit PNG, plus an sRGB JPEG without a profile.

The chart, from top to bottom:
1. sRGB primaries and secondaries;
2. common colours (skin, sky, foliage, orange, purple) and an 18 % grey;
3. a grey ramp;
4. black-to-red, black-to-green and black-to-blue ramps;
5. a full-saturation hue sweep;
6. colours **outside sRGB**: Display P3 primaries and secondaries, next to the sRGB red.

All files must look the same in `tri`, and the same as in Preview or darktable. The last
row is the exception: it is clipped in the sRGB files (normal) and partly in Adobe RGB
(the P3 red). It is vivid on a wide-gamut screen and nearly flat on an sRGB one.

The tool then reads each file back as `tri` does (`flat::decode_working`) and prints the
largest gap per row, in levels out of 255.

Checked on 2026-11-11 against macOS's ColorSync (`sips -m` to sRGB): every file agrees,
JPEG edges aside. Two things showed up along the way:
- **`moxcms` 0.8's Display P3 profile carries a wrong CICP tag**: "SMPTE RP 431", cinema
  P3 with the DCI white, instead of "EG 432". ColorSync follows that tag. The charts, and
  the P3 profile `tri` builds for an iPhone HEIC, leave the tag out; `tri` itself reads the
  primaries of the matrix and was not affected.
- **ImageIO rejects these Display P3 and Rec.2020 profiles inside a PNG** (`sips -g
  profile` says "nil") and reads the file as sRGB, while it accepts the same profile in a
  TIFF or a JPEG. No PNG is written for these two spaces, so the comparison with Preview
  cannot mislead.

## Still to do (#422)

- Check with exotic output profiles, and on Linux and Windows (Windows: compile-checked
  only from macOS, untested by the developer).
- Linux: per-screen profiles (`_ICC_PROFILE_n`), Wayland's colour-management protocol
  once egui / wgpu expose it.
