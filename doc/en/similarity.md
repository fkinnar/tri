# Module `tri::similarity`: grouping similar images

Decided on 2026-10-18 with the user, on the model of Capture One: automatic **stacks**
(bursts, series of the same scene), a **threshold slider** that regroups live. This
module is the engine; the stack display is in the interface.

## Signature (`Signature`)

Computed once per image, on a small version (`[similarity] side`, 64 px: the camera JPEG
of a RAW, the shrunk flat image), and kept in the analysis cache (`Level::Similarity`,
~70 bytes):

- **composition**: 9 × 8 luminance cells, each compared with its right-hand neighbour,
  64 bits ("dHash"); insensitive to small shifts, to exposure and to details;
- **colours**: average colour of 4 × 4 cells; separates two scenes with the same
  composition (sky and sea, indoors and outdoors);
- **capture time** (EXIF / LibRaw), in seconds: only the differences are used.

Distance from 0 (identical) to 1: `shape_weight` × (differing bits / 64) + the rest ×
mean colour difference (×4, capped: a 10 % difference is already very visible).

## Grouping

Images are taken **in grid order** (that of the shoot): two neighbours belong to the same
group if their distance is below the threshold (`threshold`, 0.25 by default) and their
time gap below `max_gap_s` (60 s; 0: no limit). `distances` computes the links between
neighbours once; `group` is then just a walk through the list: instantaneous, hence the
live slider. An unknown link (signature not yet computed) breaks the group.

Calibration (2026-10-18, 94 RAF of a portrait outdoors then indoors): distances from 0.01
to 0.05 within a burst, 0.1 to 0.3 between poses of the same series, 0.4 to 0.55 between
scenes; at threshold 0.25: 20 groups, the indoor session (2 h 35 later) separated.
`tri <dossier> --groups [--threshold X]` shows the links and the groups, for tuning.
Computation: ~50 ms per RAF (LibRaw opening and shrunk embedded JPEG), light workers.

Remaining (decided: later): manual corrections (merge, split), non-contiguous groups
(same subject at two different times).

## Centre computed from the folder, strictness slider (2026-10-21)

The user's idea: same logic as the blur slider. The stack slider goes from -1 to 1; **in
the middle**, the threshold computed from the folder; **to the right stricter** (lower
threshold, smaller stacks: bursts only), **to the left more lenient** (larger stacks);
double-click: back to the middle.

- `suggested_threshold`: the **Otsu** cut over the distances between neighbours (the one
  that best separates small distances, same series, from large ones, scene changes).
  Links already broken by the time gap do not count. Falls back to `threshold` if
  `auto = false`, with fewer than `auto_min_links` links, or if the separation (share of
  the variance explained by the cut) stays below `auto_separation`; clamped between
  `threshold / auto_range` and `threshold × auto_range`. Measured on three real shoots
  (2026-10-17): 0.21, 0.16 and 0.15 (separation 0.66 to 0.77), against a fixed 0.25.
- `with_strictness(centre, s, strictness_span)`: `centre × span^(-s)` (×3 to ÷3 by
  default).
- `threshold_by_rank(sorted distances, centre, s, span, min)` (2026-10-25, issue #61: the
  strict side was pixel-sensitive, and stacks remained fully to the right): the slider
  follows the **rank** of the distances (`distances_by_rank`), not their value. In the
  middle, `centre`; towards the right, fewer and fewer links joined, in equal shares,
  down to none; towards the left, up to `centre × span`. Simulated on six real shoots:
  fully to the right, 9 to 40 stacks remained with the geometric threshold, none now.
  Fewer than `min` links: `with_strictness`.
- `folder_strictness` / `remember_strictness`: the chosen strictness, remembered for each
  folder (`group-strictness.tsv` in the data folder); in the middle, it is forgotten. The
  former per-folder thresholds (`group-thresholds.tsv`) are no longer read.
- `tri <folder> --groups` groups at the computed centre (`--threshold X` to force a
  threshold).

## Pauses and reach to the left (2026-11-06, issue #306)

Measured on FULLTEST (570 RAF from a studio self-portrait shoot, same framing): the
fingerprint mostly sees the background and the framing. A hand near the face barely
changes it. "Hand on the head" and "arms down" are 0.34 apart, as far as two images of
the same pose where one moves within the frame (up to 0.41). Finer fingerprints (grey
grids from 12 × 16 to 48 × 64, shift-tolerant, medians over windows of 2 to 4 images)
did not separate the poses better. On the other hand, pose changes fell on **pauses**:
9 to 302 s, against 1 to 3 s between two images of the same pose.

- `with_pauses(links, weight)`: a pause longer than the folder's **pace** (median gap,
  at least 1 s) multiplies the distance by 1 + weight × log2(pause / pace). Multiplying
  rather than adding: two identical images stay together, the pause decides between the
  others. The slider's **centre** is still computed from the distances without pauses:
  a pause can only cut more. Measured at 0.1: fully to the left, the 234-photo "bust"
  stack splits into 12 + 102 + 120 (arm across the face, hand on the head or the chin,
  arms down); at the centre, on three other shoots, 0 to 4 more stacks.
- A pause is a **hint**, not a rule (the user's feedback): in the studio, the content
  comes first; in street photography, a pause often means another place. Hence the
  **"Weight of pauses"** slider of the `Ctrl+G` window, remembered per folder
  (`folder_pauses` / `remember_pauses`, `group-pauses.tsv`):
  `pause_weight(weight, t, range)`, in the middle `[similarity] pause_weight`, fully to
  the left 0 (pauses ignored), fully to the right × `pause_range`.
- **"Reach to the left"**: `strictness_span` can be set per folder (`folder_span` /
  `remember_span`, `group-span.tsv`), from 1 to `strictness_span_max`. At ×3, the
  strictness slider stopped at 0.375 on FULLTEST, below the 0.41 that would bring the
  "arms crossed" stacks together.
- `tri <folder> --groups` also shows the distance with the pause, and groups with it.

## Chosen covers

`chosen_covers` / `choose_cover`: the favourite of each stack, chosen by the user (one per
stack), separate from the ratings; remembered in `stack-covers.txt` in the data folder.
