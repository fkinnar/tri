# Module `tri::exposure`

Exposure of an image: histograms on the **real data**, clipped and crushed pixels, mask
of the affected zones. *Derived* data (analysis cache, see `CLAUDE.md`), never written
to the XMP.

## Why the sensor data

The camera JPEG (shown by default) reflects the camera settings: curve, white balance,
highlight compression. It can look blown out when the RAW is not, and vice versa. The
histogram of a RAW is therefore **always** computed on the sensor mosaic (user's
decision, 2026-10-17), like FastRawViewer:

- after `libraw_unpack` only: no demosaicing, no white balance, no curve
  ([`raw::Sensor`](raw.md));
- at the real bit depth (12, 14 or 16 bits, deduced from the sensor's white level);
- **black subtracted pixel by pixel**: on Fuji, the black level is a 6 × 6 pattern
  (LibRaw's `cblack[6..]`, ~1022 in 14 bits), not a single value — read as `black = 0`
  on the first try, hence distorted shadows;
- filter channels: red, green (the two Bayer greens together), blue, Bayer or X-Trans
  pattern read by `libraw_COLOR`.

A flat image is analyzed **at its recorded bit depth** (`flat::decode_deep`, since
2026-10-17): 16 bits for a 16-bit TIFF or PNG (`Exposure::from_rgb16`,
`Source::Flat { bits: 16 }`), 8 bits for the rest (JPEG, WebP, HEIF: a 10-bit HEIC is
still brought down to 8 bits by libheif). Values are **converted to linear light** (sRGB
decoding) before any EV computation and the clipped / crushed thresholds: 128 out of 255
is not half the light but ~22 % (-2.2 EV). Before 2026-10-17, a mid gray showed up at
-1 EV. The linear histogram, for its part, stays on the file's values (0 to 255, or 0 to
65,535), like usual histograms. A test shows it: a near-black at -9.96 EV in 16 bits is
not crushed, but is once brought down to 3 out of 255 (-10.1 EV).

**Useful range** (`Exposure::stops`): log₂ of the range of a RAW (13.9 EV in 14 bits,
11.8 in 12 bits), ~11.7 EV for an 8-bit sRGB image (smallest non-zero value, converted
to linear light), ~19.7 EV for 16 bits (display limited to `ev_range`). The interface
shows this range by default (user feedback, 2026-10-17: a JPEG was shown over 12 EV like
a RAW).

## What is computed (`Exposure`)

A single pass over the pixels builds the **full** histogram (one bin per value and per
channel) and the mask; everything else is derived from it:

- `linear`: linear histogram from 0 (black) to `range` (white), `[exposure]
  linear_bins` bins;
- `ev`: histogram **in EV below saturation** (the interface's default scale), from
  `-ev_range` to 0, `ev_bins_per_stop` bins per EV; black and everything darker go into
  the first bin;
- `total`, `over`, `under` per channel: pixels, clipped (≥ white × 2^`over_ev`), crushed
  (≤ white × 2^`under_ev`). `over_ev` is **-0.02 EV** by default, not 0: on an X-T4 RAF,
  a clipped red or blue is recorded from 16383 at the center down to 16323 towards the
  edges, in steps (the camera's vignetting correction on the raw data). At the exact
  threshold, only a central rectangle counted as clipped in all three channels: a flaw
  revealed by the per-channel zone display (2026-10-17). No normal image of the session
  is affected by this margin (not even by -0.05 EV); `over_fraction` /
  `under_fraction`: most affected channel;
- `high`: exact value at the 99.9th percentile per channel; `headroom_ev()`: headroom
  before saturation of the most exposed channel, in EV ("expose to the right"; 0 = at
  white). Computed on the full histogram: a first version on the reduced histogram gave
  -0.004 EV for a white image;
- `mask`: `mask_size` cells on the long side, bits `OVER_R/G/B` (at least one clipped
  pixel in the cell: a blown highlight must show) and `UNDER` (at least
  `mask_under_share`, half, of the cell's pixels below the shadow threshold); **already
  rotated** according to the camera orientation, to be overlaid as is on the displayed
  image. First version: `UNDER` only if *all* pixels were crushed; sensor noise near
  black almost always lifting one, the black sweater had no zone at all (seen in a
  screenshot on 2026-10-17). `VERSION` (2) enters the cache key: the computation changed
  without the settings changing.

## Cost (X-T4 RAF, 26 Mpx, Apple Silicon Mac, 2026-10-17)

- `unpack` 0.35–0.55 s, then ~40 ms for all the rest: **0.38 s on average** per RAW over
  129 files from a real session (`tri <dossier> --exposure --raw`); 0.15 s for a fully
  blown-out image (compression);
- `unpack` is already parallelized by LibRaw (OpenMP): 2 threads = 1.7 s per file, 8
  threads = 9 s. RAW analyses therefore go through the single LibRaw worker, like the
  renders.

Checked on the session: DSCF0129 (fully white photo) → 99.8 % clipped; DSCF0214–0220
(black sweater, -3.5 EV of headroom) → 4 to 6 % crushed; portraits on a white background
→ -0.45 EV of headroom, nothing clipped (exposure pushed well to the right).

## Cache

`to_bytes` / `from_bytes`: little-endian binary format, versioned header (`TRIEXP01`),
run-length compressed mask (almost always empty): a few KB per image.

## Command line

```
tri <dossier> --exposure [--raw|--flat] [--depth N]
```

One line per image: source (bit depth, black, white), headroom before saturation,
clipped, crushed, duration.

## Per-channel statistics (`Exposure::channel_stats`, 2026-10-18)

Taken from the EV histogram (1/8 EV per bin by default), without rereading the image:
median and percentiles (interpolated within the bin where the cumulative count crosses
the threshold), mean and standard deviation (bin centers), clipped and crushed shares of
the channel. In EV below saturation: the mean is that of the tones, not of the light.
Shown in the interface's large histogram. No luminance for a RAW (each photosite only
measures one color); for a flat image, a real luminance would require one more histogram
in the analysis (not done).

## Crushed per channel (2026-10-18)

The mask also marks, for each channel, the cells where at least `mask_under_share` of
that channel's values are crushed (`mask::UNDER_R`, `UNDER_G`, `UNDER_B`, functions
`mask::over(c)` / `mask::under(c)`), in addition to the all-channel crushed flag
(`mask::UNDER`, unchanged: full-screen zones). Used for the analysis image of the large
histogram when a single channel is chosen. Cache format unchanged (one byte per cell),
but `VERSION` goes to 5: analyses already in the cache are recomputed.

## Strictness of the "well exposed" filter (2026-10-24)

`clip_threshold(base, strictness, span)`: threshold of one side (`Clip::Over` blown,
`Clip::Under` crushed) at a slider's strictness (-1 to 1): `base × span ^ -strictness`,
at most 1; at -1, 1 (that side no longer excludes anything: black backgrounds, issue
#48). `folder_strictness` / `remember_strictness`: strictness remembered per folder and
per side (`clip-over-strictness.tsv`, `clip-under-strictness.tsv` in the data folder).
