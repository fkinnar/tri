# Chosen computations on a folder (`tri::compute`)

Issue #555 (user's idea, 2026-11-21): prepare a folder **without opening it or waiting**
(before a palette search, before culling on a laptop, before unplugging a slow drive),
choosing **what** to compute.

## Computations (`Kind`)

| Id | What is computed | Cache levels |
|---|---|---|
| `previews` | thumbnails and camera previews (flat image: its preview) | `Thumbnail`, `Embedded` / `Standard` |
| `raw` | RAW renders (RAWs only) | `Standard` |
| `exposure` | histograms | `Exposure` |
| `sharpness` | sharpness | `Sharpness` |
| `signatures` | stack signatures | `Similarity` |
| `palette-camera`, `palette-raw`, `palette-darktable`… | palettes of a source | `Palette(source)` |

## `run`

Each image goes through the engine's producer (`Producer`, the same as the interface):
same cache keys, what is already there is skipped. Summary (`Summary`): computed, already
cached, failed (and the first reason), cancelled. `clear` first erases what the
computation fills ("recreate").

Several computations run at once (one task each), but **heavy** work (LibRaw, development
programs: `scheduler::is_heavy`) takes **turns** (`heavy_turn`, a shared lock): they
already use every core, and several renders at once are much slower than in series
(measured on 2026-10-16).

## Priority

A requested computation goes **before** the open folder's background work (user's
decision): the interface holds that background (`Scheduler::set_background_paused`) while
a computation runs; the shown image, its neighbours and the thumbnails are still served.

## Interface and command line

Right-click a folder, "Compute": one entry per computation, "Compute everything" and
"Compute everything again"; each is an action (`core.folders.compute.*`,
`plugin.<program>.compute_palettes`), so also in `:`, `?` and `[keys]`. Each computation
has its line in the status bar, with its stop button (see [gui.md](gui.md)).

On a **selection** too (feedback of 2026-11-21): actions `core.file.compute.*` and
`plugin.<program>.compute_image_palettes` (the selection, else the current image), and
the "Compute" list of the right-click box on an image; "recreate" then erases only those
images' entries (`clear_files`).

`tri <folder> --compute previews,exposure,palette-camera [--recreate]`.

## Tests

Fake producer: cache skipped, RAW renders for RAWs only, failures counted, cancelling;
ids round trip. Interface: a real task (computed, then already cached, then recreated),
the actions, the menu.
