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
- `folder_strictness` / `remember_strictness`: the chosen strictness, remembered for each
  folder (`group-strictness.tsv` in the data folder); in the middle, it is forgotten. The
  former per-folder thresholds (`group-thresholds.tsv`) are no longer read.
- `tri <folder> --groups` groups at the computed centre (`--threshold X` to force a
  threshold).

## Chosen covers

`chosen_covers` / `choose_cover`: the favourite of each stack, chosen by the user (one per
stack), separate from the ratings; remembered in `stack-covers.txt` in the data folder.
