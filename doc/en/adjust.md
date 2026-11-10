# Viewing adjustments (`tri::adjust`)

Issue #137, decisions of 2026-11-10. A few basic adjustments **to compare and cull a
series**, not to develop: exposure, white balance, black and white with a channel mixer.
The key: **apply one image's adjustments to the others**, matching what must be matched
(a photo at -1 EV and a well-exposed one end up alike).

Done: the engine, the command line, the interface ("Adjustments" section), the report of
the values. Next: writing them for other software (RawTherapee / ART first), real LibRaw
thumbnails as an option, exporting adjusted images. The
adjustments apply **before** any development by another program: an image developed in
darktable is not affected.

## Layers

An image's adjustments (`Layers`) are layers that can be redone separately:

| Layer | Role | Goes to other images as (default mode) |
|---|---|---|
| `correction` (exposure, balance) | bring the image to a common base | **harmonised**: computed for each image |
| `creative` (exposure, balance), optional | the intent, the same for the series | **copied** (the same offset) |
| `mono` (mixer) | black and white | **copied** |

Modes (`Mode`) can be forced layer by layer (`Modes`):

- **absolute**: the value as is;
- **offset**: the same difference added to what the image already has;
- **harmonise**: the difference computed so that the image matches the reference;
- and **keep**: the image keeps its own.

## Values

- **Exposure** in EV.
- **White balance** stored as red and blue gains (`Gains`), relative to the image's
  as-shot balance, green being the reference. Shown and typed in **kelvins and tint**
  for the correction (see below); the creative offset keeps **warmth** (log₂ red / blue,
  positive = warmer) and **tint** (positive = magenta), relative.
- **Mixer**: share of red, green, blue (1.0 = 100 %). **Normalised** by default: the
  shares are brought back to a sum of 100 % when applied, so moving a slider doesn't
  change the brightness (`normalise`, written in the sidecar only when switched off).
  Colour-filter presets: `neutral` (Rec. 709 luminance), `red`, `orange`, `yellow`,
  `green`, `blue`.

## Kelvins (`adjust::kelvin`)

Feedback of 2026-11-10: relative warmth didn't speak; a temperature is typed as in a
development program. Kelvins are only a **presentation** of the stored gains:

- an illuminant (K, tint) is a point of the CIE 1960 (u, v) plane: the Planckian locus at
  K (Krystek's approximation), moved perpendicularly by `tint / 3000` (Duv; positive:
  towards green, so the image turns magenta, as in Lightroom);
- its colour in linear sRGB is that of a white lit by it, in a D65-balanced render;
- the image shown is balanced for the as-shot illuminant, of colour `shot`: for a RAW,
  the camera neutral (`1 / cam_mul`) brought to LibRaw's daylight balance (`pre_mul`,
  D65) then through `rgb_cam` (`RawFile::as_shot_white`, read on opening, no unpacking);
  D65 assumed for a flat image. Balancing for another illuminant `W` multiplies each
  channel by `shot / W` (von Kries in the output space), normalised to green.

Accepted approximation: every program computes its kelvins with its own camera model;
tri's are close, not identical (on an X-T4 RAF in auto WB, about 5,970 K with a green
tint, consistent with the camera's own temperature table).

Presets (`[adjust]`, zero tint): daylight 5,500 K, cloudy 6,500, shade 7,500, tungsten
2,850, fluorescent 3,800, flash 5,500; plus "as shot" (no gains) and "auto" (grey world).

## Automatic exposure

`auto_exposure`: the correction that brings the image's median luminance (its balance
corrected, without the other adjustments) to `[adjust] auto_exposure_target` (0.18,
the mid grey of light meters), **with highlight priority**: when brightening, it stops
when the `auto_exposure_highlights` percentile (0.99) reaches
`auto_exposure_highlight_target` (0.6); when darkening, when it comes down to it.
Feedback of 2026-11-10: on a low-key portrait (dark sweater and background, spot metering
on the face), the median alone gave +3.3 EV and a burnt face; highlight priority gives
+0.3 EV. A snow scene isn't brought down to grey either.

## Where they apply

| | RAW rendered by LibRaw | Camera JPEG, flat images |
|---|---|---|
| Exposure | on the sensor data, before demosaicing (`RenderOptions::exposure`, LibRaw's `exp_shift`; bounds -2 to +3 EV, the rest on the image): highlights recoverable | on the image, in linear light: clipped highlights don't come back |
| Balance | on the rendered image, in linear light | same |
| Mixer | exact | exact |

A RAW's balance is done **on the rendered image, not on the sensor multipliers** (a
deliberate departure from the initial decision): harmonisation measures the cast on the
image, in the output space; moving it to the sensor would need a conversion through the
camera's colour matrix, exact for a single colour only. To revisit if the difference
shows in use.

Each 8-bit image is brought back to linear light with **its own** curve (`Transfer`):
BT.709 for a LibRaw render (the previews' curve, `OutputSpace::PREVIEW`), sRGB for a JPEG.
Decoding a LibRaw render as sRGB skewed the measure in the shadows (try of 2026-11-10 on a
dark photo: +1.18 EV instead of +1).

## Measures and harmonisation

`measure` gives, on the image as shown (adjustments included):

- the **median luminance** (Rec. 709, linear): little sensitive to a clipped sky;
- the **cast**: linear red / green and blue / green means, without blocked or nearly
  clipped pixels (a clipped channel skews the ratio); on a zone (`Zone`) for the
  **eyedropper**, on the whole image for "grey world".

`apply_to(reference, reference base, target adjustments, target base, modes)`:

1. creative offset and black and white first (they count in the measure);
2. harmonised balance: the target takes the cast of the **corrected** reference;
3. harmonised exposure: the target's median luminance, with all its new adjustments,
   equals the reference's with its own. Exposure is a plain factor in linear light: one
   computation is enough, clipped pixels aside.

A small image is enough for the measures (800 px on the command line). On a black and
white image, harmonisation works on the black and white (decided: the difference is
measured after the adjustments already set).

The initial decision planned to measure on the RAW histograms of the analysis cache;
measuring on the image shown works the same for every source (RAW, camera JPEG, flat
images) and accounts for black and white.

## Report

`report`: total exposure to copy (correction + creative, also separate), balance in
kelvins (and the as-shot one), warmth, tint, gains, applied mix in percent. First stage
before writing into development software formats: these values are copied by hand
("Values" tab).

## In the interface

**"Adjustments"** section of the right panel (at the bottom, after EXIF; grid and full
screen), for the current image (`gui/src/app/adjust.rs`), reorganised after the first
try (feedback of 2026-11-10):

- **"Sync N images"**, at the top: greyed with a single image, checked by default as
  soon as there are several. See below.
- **Four tabs**, kept for the session, a dot "•" on those that change the image (a
  hidden adjustment shows):
  - **Base**: exposure and **"Auto"**; temperature (K), with the white balance list (as
    shot, auto on the whole image, presets with their kelvins) and the **"Eyedropper"**
    (in full screen, a click on a zone that should be grey, `[gui]
    adjust_eyedropper_size`; or a **frame drawn** for an auto balance on that part; from
    the grid, the button opens the full screen); tint.
  - **Style**: exposure, warmth, tint. No checkbox (the tab is enough): the creative
    offset exists as soon as a value isn't zero.
  - **B&W**: the **filter** (list: "None (colour)", then red, orange, yellow, green,
    blue; no neutral, luminance alone bringing nothing next to the filters; "Custom" as soon as the mix is touched), red / green / blue in %,
    **"Normalised"** (checked by default) and the total. No checkbox: choosing a filter
    or touching a channel switches to black and white, "None" goes back to colour.
  - **Values**: the table to copy into a development program (total exposure, base,
    style; balance and as shot in kelvins; style balance; mix), right-click a row to copy
    it, "Copy all".
- Each setting: its **title** (with Auto, the list or the eyedropper on the right), then
  the **slider across the whole width** and its value to type (exposure to the hundredth
  of an EV, kelvins to the unit). The interface's common slider: drag by large steps,
  Shift + drag for fine adjustment; double-click: default value (temperature and tint:
  the as-shot balance).
- **Normalised**: the total stays at 100 %. Moving a channel moves the other two the
  opposite way, in proportion to their shares (half each if they are zero or of opposite
  signs); at the bounds (-100 % to 200 %), the dragged channel stops where the others
  can't follow. Checking the box on a mix not at 100 % rescales it (`Mixer::set_locked`,
  `Mixer::rescaled`).
- Buttons **as icons** (name and explanation in the tooltip): auto exposure (circled
  "A"), eyedropper, and at the bottom **"Copy adjustments"** / **"Paste adjustments"**
  (see below), **"Harmonise the selection"** (exposure and balance of the selected images matched to
  the current image, style and black and white untouched; one step), **"Before"** (every
  image without its adjustments, toggle, sidecars untouched), **"Reset"**.

### Syncing a selection

Several images selected and the box checked: what is done on the current image goes to
the whole selection, in **one** history step. Rule: **a slider shifts, a button
computes** (`tri::adjust::propagate`).

| Setting | On the other images |
|---|---|
| Exposure (slider) | same difference in EV (A 0.3 → 0.5: B 1.0 → 1.2) |
| Temperature (slider) | same ratio in kelvins (rule of three, each in its own kelvins) |
| Tint, style (sliders) | same difference |
| Auto, presets, as shot | computed for each image (an image at -1 EV comes back to 0, the one at 0 doesn't move) |
| Eyedropper | the current image neutralised on the zone, the others matched to it |
| Black and white | copied |

Exposure is additive, not proportional: EV is already a relative scale (+0.2 EV = light
× 1.15 on every image); a rule of three on EV would brighten the images already corrected
more, and wouldn't work from 0. Kelvins, always positive, do follow a rule of three;
tint, around 0, a difference. Only the **layer touched** goes: changing the black and
white doesn't re-match the exposure. While dragging, **the whole selection follows live**,
and all thumbnails change together: each is redrawn in place from its unadjusted
thumbnail, kept in memory for the gesture (no disk, no decoding at every frame); images
and their sidecars are written on release.

### Copy / paste

**"Copy adjustments"** keeps the current image's **and its thumbnail**; **"Paste
adjustments"** puts them on the current image (and the synced selection), even in
another folder: exposure and balance **matched** to the copied image, style and black
and white copied (`apply_to` default modes). No default shortcut: Ctrl+Shift+C and
Ctrl+Shift+V are taken (copy a stack, previous version).

Sliders show the change at once (thumbnail and large image); the sidecar is written on
release, one history step per gesture (`u` / Ctrl+Z undoes it). Adjustments are read in
the background from the sidecars (never waiting on a slow disk) and applied to the pixels
just before the texture (`gui/src/adjust_view.rs`, shared with the loading thread). **On
screen, everything is done on the image** (approximation on JPEGs, exact mixer); exposure
on the sensor data stays for the command line and the export to come. A development
(darktable…) is never adjusted.

Actions without a default shortcut, in the palette: `core.adjust.before_after`,
`core.adjust.apply` (harmonise the selection), `core.adjust.copy`, `core.adjust.paste`.

## Command line

```
tri --adjust <reference> <image>... --to <folder> [--ev EV|auto] [--neutral auto|x,y,w,h]
    [--kelvin K[,tint]|<preset>] [--creative-ev EV] [--warmth EV] [--tint EV]
    [--mono <preset>|r,g,b] [--embedded]
```

The first image is the reference, set by the options (`--ev`, `--neutral` and
`--kelvin`: its correction, `--ev auto` the automatic exposure; `--creative-ev`, `--warmth`, `--tint`: the creative offset; `--mono`: a
preset or three percentages). The following ones get its adjustments in the default
mode. Each image is written as `<name>-adjusted.jpg` in `--to` (never next to the
photos), at the preview size (`--side`), and its report is printed. A RAW is rendered by
LibRaw; `--embedded` takes its camera JPEG (approximation).

Example: match a series to its first image, neutralise the balance on a grey zone and
switch to orange-filter black and white:

```
tri --adjust DSCF0101.RAF DSCF0102.RAF DSCF0103.RAF --to /tmp/try \
    --neutral 0.40,0.55,0.10,0.10 --mono orange
```

## Tests

`src/adjust/kelvin.rs`: D65 ≈ 6,500 K, kelvins and colours inverse, more kelvins warm the
image and a positive tint adds magenta, gains ↔ kelvins. `src/adjust/mod.rs`: one EV
doubles the light (sRGB and BT.709), reversible curves, mixer,
warmth / tint, eyedropper on a grey zone, harmonising an image at -1 EV, balance aligned
on the corrected reference, black and white series measured after the mix, forced modes,
report, normalised mix, automatic exposure, creative offset kept when checked without
values. `tests/cli.rs`: a copy at -1 EV gets about +1 EV.
