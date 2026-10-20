# Module `tri::sharpness` — sharpness map (focus peaking)

To check focus at 1:1 (requested on 2026-10-17): the sharp edges of the
image are highlighted in a bright colour, like the *focus peaking* of cameras.

## Decisions (2026-10-17)

- **At 1:1 only**, in full screen: in the grid, it would be costly and unreadable at
  that size.
- **2-3 configurable colours, cycled with a shortcut**: depending on the image, one colour
  stands out better than another (a colour close to the subject gets lost in it).

## Computation

`peaking(&Bitmap, &SharpnessSettings) -> Peaking`: one byte per pixel (255 edge, 0
nothing), at the image's dimensions.

1. Approximate luminance `(R + 2G + B) / 4` (green carries most of the detail).
2. **Sobel** gradient, `(|gx| + |gy|) / 4` (0 to 510; a hard black / white edge
   gives 255), compared to a **fixed threshold** (`threshold`, 48; 32 until 2026-10-17,
   judged too permissive: a soft photo stayed marked at 4 %, against 1 % at 48; a
   sharp one goes from 16 % to 8 %, still clearly marked): not relative to the image,
   so that two photos of a burst can be compared — the sharper one shows more edges.
3. **Isolated points removed** (`min_neighbours`, 2): an edge is a line, high-ISO noise
   makes scattered points (measured on an X-T4 RAF at ISO 2000: skin was
   covered in points).
4. **Thickening** (`spread`, 1 pixel on each side): a one-pixel edge is
   barely visible at 1:1 on a high-density screen. `marked` counts before thickening.

The map is computed on the **displayed image** (camera JPEG at its size, or full RAW
render), so it overlays pixel for pixel, orientation included. The camera JPEG is
sharpened by the camera, which inflates the map a little; the RAW render is not. Idea
for later (per-image score, "sharp only" filter, analysis cache): compute
on the RAW mosaic (green channel), without sharpening or demosaicing.

Measurements (Apple Silicon Mac, `--release`, X-T4 camera JPEG 2944 × 4416): 35 ms for the
gradient alone, 111 ms with isolated-point removal and thickening. No
parallelisation: pointless at this cost, and the map is only computed on demand.

Tuning tool: `cargo run --release --example decode -- peaking <image> <sortie.png>
[--threshold N]` (edges in red, time, share of marked pixels). Thresholds compared on
the face: 48 marks only hard edges; 32 also marks the lines of the hand and
the lips.

## Sharpness score: spotting blurry photos (2026-10-17)

Goal: weed out **objectively** blurry photos (missed focus, camera shake), without AI
to begin with, and set up the pipeline (background computation, analysis cache, filter) that a
finer method (sharp eyes on a portrait…) will reuse later. To be refined with
deliberately failed photos (the user is preparing some).

`score(&Bitmap, &SharpnessSettings) -> Score`:

1. **Fixed size**: the image is reduced at decoding to `score_side` (1536 px long side),
   so that cameras can be compared and noise weighs less. RAW: the camera JPEG
   (`preview::compute_sharpness`, light workers, ~120 ms); flat image: itself.
2. **Luminance stretched** between its 1 % and 99 % percentiles (`score_stretch`): without this, an
   underexposed but sharp photo passed for blurry (measured on a dark portrait: 0.21
   × the median before, 0.42 after). Luminance range under `score_min_range`: no
   detail (all-white image), zero score.
3. **By tiles** (`score_tiles` = 16 along the long side): mean gradient (Sobel, root of
   the energy) of each tile.
4. **The sharpest tiles** make the score (`score_top_share` = 5 %): the question is
   "is anything sharp somewhere?". A portrait with a deliberately blurred background
   keeps a good score; camera shake or missed focus makes everything drop.

**Relative judgement** (`judge`): the score means nothing in absolute terms (ISO, lens,
camera sharpening). A photo is **blurry** if it is below `blur_ratio` (0.5) × the
**folder median**, **or** below `neighbour_ratio` (0.8) × the median of its
**neighbours** (`neighbours` = 3 photos before and 3 after, in folder order: same
scene, same burst most of the time). The two rules complement each other: the first catches
long blurry series (their neighbours are blurry too), the second an isolated miss
in a sharp, highly textured series, which stays at folder level.

**Calibration of 2026-10-17** on 94 X-T4 RAFs deliberately failed and labelled by the user
(green: sharp, red: blurry, yellow: borderline; folder `2026-10-17_MISS`):

| | Folder only (< 0.5) | Folder + neighbours (< 0.8) |
|---|---|---|
| Reds caught | 12 / 15 | **14 / 15** |
| Greens wrongly flagged | 0 / 56 | **0 / 56** (the lowest at 0.95 × its neighbours) |
| Yellows flagged | 1 / 15 | 1 / 15 |

Lessons:
- **Focus on the background** (DSCF0361: blurry subject, sharp post):
  undetectable without knowing where the subject is. The focus point (`FocusPixel`, recorded by
  Fuji in the frame of the embedded JPEG) is not enough: it was on the post, sharp — the
  camera did what it was asked. Subject detection (person,
  face) will be needed: the "AI" step.
- **Sharpness at the focus point** (prototype): no better than the global score, because it
  depends on the content of the area (a smooth cheek looks blurry); comparing areas of
  different scenes makes no sense.
- The **yellows** (borderline) mostly stay at the level of the greens: differences too
  subtle for a score without AI at this analysis size.
- An unlabelled photo that was flagged (DSCF0347) was indeed blurry at 100 %.

**Pipeline**: analysis cache level `Level::Sharpness` (binary entry, `Score::to_bytes`,
~1 KB, version `SCORE_VERSION` in the key), computed in the background by the scheduler for the whole
folder (`[sharpness] background`, light workers, after thumbnails and camera previews),
read by `preview::sharpness`. Filter `BlurFilter` (trait `Filter`, like the others): a
photo with no score computed yet is kept.

Interface: "blurry" label on the thumbnail (`[gui.grid] blur_badge`), a
"Sharpness" line in the metadata panel (ratios to the folder and to the neighbours), filters
"without blurry" (`f b`) and "blurry only" (`f B`, to check flagged photos
before rejecting them: `BlurRule`), "sharpness" progress bar.

**Focus point** (2026-10-17): the focus point marker (read from the RAW,
`Metadata::focus_point`, see `metadata.md`) is drawn in full screen and in comparison
(`m`, `[gui.view] focus_point`, shape `focus_marker`: square, cross, circle or heart).
It shows at a glance a focus on the background, which the score does not see. The filter
applies once all scores are known (the median moves while they arrive), and
only if the judgement has changed.

Command line: `tri <dossier> --sharpness` (score, ratios to the folder and to the
neighbours, judgement, duration): the threshold tuning tool.

Measured on 2026-10-17 on the test folder (26 X-T4 RAFs): 24 photos between 0.80 and 1.08 ×
the median; flagged: a white photo (zero score) and an underexposed portrait (0.42),
indeed soft at 100 % (eyelashes and eye contours not crisp) — to be confirmed on
deliberately failed photos.

Known limits: the relative judgement assumes comparable subjects in the folder (a
folder mixing landscapes and portraits is less reliable); the camera JPEG is
sharpened; noise at very high ISO looks like detail.

## Settings

| Section | Key | Default | Role |
|---|---|---|---|
| `[sharpness]` | `threshold` | 48 | gradient threshold |
| | `spread` | 1 | thickening (pixels) |
| | `min_neighbours` | 2 | marked neighbours required (0: keep everything) |
| | `score_side` | 1536 | score: long side of the analysed image |
| | `score_tiles` | 16 | score: tiles along the long side |
| | `score_top_share` | 0.05 | score: share of the sharpest tiles |
| | `score_stretch` | 0.01 | score: luminance stretch percentiles |
| | `score_min_range` | 8 | score: minimum luminance range (otherwise zero score) |
| | `blur_ratio` | 0.5 | blurry below this fraction of the folder median |
| | `neighbours` | 3 | neighbours compared on each side (0: none) |
| | `neighbour_ratio` | 0.8 | blurry below this fraction of the neighbours' median |
| | `background` | `true` | scores of the whole folder in the background |
| `[gui.sharpness]` | `visible` | `false` | map shown at startup |
| | `colors` | red, green, blue | colours of the cycle |
| | `opacity` | 1.0 | edge opacity |

## In the interface

`p` shows or hides the map (`core.view.peaking`); `Ctrl+P` switches to the next
colour (`core.view.peaking_color`) and shows it if it was hidden. Outside 1:1, the
info line reads "sharpness: at 1:1 (z)". The map is computed by the loading
thread (`gui/src/loader.rs`, after the requested images), sent as a white
texture whose opacity marks the edges, and **tinted when drawn**: changing colour
recomputes nothing. Only one map is kept (that of the displayed image); leaving 1:1
withdraws the pending request.

### Severity slider (2026-10-20)

Requested by the user after a portrait at a very wide aperture (sharp eyes, everything
else blurry, judged "blurry" although usable): a slider in the filter
bar, next to the drops. **In the centre** the thresholds from the settings, **to the right
stricter** (more blurry photos), **to the left more lenient**. It multiplies the three thresholds
of the judgement (`blur_ratio`, `neighbour_ratio`, `group_ratio`) by `strictness_span ^
severity` (×0.5 to ×2 by default, never above 1): `SharpnessSettings::with_strictness`.
Live effect (badges, "blurry" filter), instantaneous: the scores are cached, only
the judgement is redone. Remembered per folder on release (`sharpness::folder_strictness`,
file `blur-strictness.tsv` in the data folder); double-click: back to the centre
(marker). The metadata panel gives the score relative to the folder and to the neighbours
(or to the stack): enough to understand a judgement. Same principle to come for the stack
slider (centre computed from the folder). The real remedy for this borderline case remains
subject detection (eyes), later.

## Tests

Score (`src/sharpness.rs`): a blur divides the score by more than two, a sharp subject on a
plain background keeps the score of an image sharp everywhere; an image four times darker has the
same score, a white image a zero score; binary round trip, truncated data
rejected; median, relative judgement, filter (images without a score stay). Scheduler:
light background job, after the camera preview. Interface: on sharp checkerboards and a
blurry checkerboard, the blurry one is judged as such, `f b` hides it then shows it again.

Map (`src/sharpness.rs`): a hard edge is marked on both sides, a uniform
area and the image borders never; the threshold is absolute; isolated points removed,
lines kept; thickening; small images without panicking. Interface: `p` in fit view
requests nothing, at 1:1 the map arrives at the image's size (pixel for pixel), leaving
1:1 withdraws the request, `Ctrl+P` cycles through the colours and shows the map again.

## Judgement within stacks (2026-10-18)

The false positives of a folder with varied subjects (free photos of the manual: close-ups on
blurred backgrounds among landscapes) came mostly from the **neighbour rule**: the
"neighbours" there are other scenes. With stacks of similar images (`similarity`),
`judge_in_groups` no longer compares a photo to its neighbours, but to the other images of its
stack (at least `group_min` = 3, blurry below `group_ratio` = 0.8 × their median); the
**folder rule remains** (it catches a burst blurry from end to end, which the stack does not
see: all its images look alike). Judgement stacks wider than display
stacks (`group_threshold` = 0.45, against 0.25): the "same scene" of a sharpness
comparison goes beyond the burst.

Measurements, with the user's labels (MISS folder, 94 RAFs: 15 reds, 56
greens) and the 19 free photos:

| Rules | MISS: reds found | greens flagged | free photos flagged |
|---|---|---|---|
| folder + neighbours (before) | 14 / 15 | 0 | 3 |
| stack only | 9 / 15 | 0 | — |
| folder + stack, threshold 0.25 | 12 / 15 | 0 | 1 |
| **folder + stack, threshold 0.45** | **14 / 15** | **0** | **1** |

Without fingerprints (`[similarity] background = false`), the previous judgement (folder +
neighbours). The remaining free photo is below 0.5 × the folder median:
only an analysis of the content (subject, intended background blur) would tell it apart.

