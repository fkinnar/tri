# Module `tri::scheduler`

Background work queue that fills the cache for a folder.

- **Workers**: a single *heavy* worker for LibRaw renders (standard and 1:1 previews of
  RAWs), plus `Config::light_workers` *light* workers (half the cores by default) for
  thumbnails and flat images.
- **Background levels per file type**: thumbnail, **camera preview** (embedded JPEG, RAW
  only) and standard preview; the 1:1 is only computed on demand. The camera preview is
  light (light worker), the standard preview of a RAW heavy (LibRaw).
- **Priorities**, recomputed each time a task is taken (`priority`):
  1. displayed image: what is shown fitted (camera preview, RAW render ~0.5 s), then its
     exposure analysis (histogram, ~0.4 s), then its 1:1 (100 % RAW ~4.5 s);
  2. neighbours n+1 then n-1: fitted, analysis, then 1:1 (preloading);
  3. thumbnails, in folder order;
  4. other camera previews, then other **exposure analyses** (before the background RAW
     renders: they feed the grid badges), then other standard previews, by distance
     (forward first).

  **Colour palettes** (`Level::Palette(source)`, #492, 2026-11-21): for the whole folder
  only once a source is chosen (`Config::palette`, `Scheduler::set_palette`; none by
  default), after everything else (class `PALETTE_BACKGROUND`: they are asked for to
  search, not to cull). The camera JPEG's goes through the light workers (the
  thumbnail); a RAW's raw-data palette and a finished render's through the **heavy
  worker**: they first make the RAW render or the software's render if it is missing.
  Changing the source drops the other source's palettes still waiting. See
  [palette.md](palette.md).

  **Sharpness score** (`Level::Sharpness`, 2026-10-17): computed in the background for the
  whole folder (`[analysis] sharpness`), by the **light workers** (downscaled camera
  JPEG, ~120 ms), same priority class as the exposure analysis; "sharpness" progress. See
  [sharpness.md](sharpness.md).

  **Exposure analysis** (`Level::Exposure`, 2026-10-17): computed in the background for the
  whole folder (`[analysis] exposure`, default `"background"`), RAWs as well as flat images; a RAW's
  goes through the **heavy worker** (LibRaw decompression, already parallelised
  internally, see [exposure.md](exposure.md)), a flat image's through the light workers.
  Like the background renders, it waits for the thumbnails and camera previews to finish.
  Measured on 24 RAFs: histograms done ~10 s after the camera previews (~0.45 s per
  image). Lead not pursued: the RAW render also decompresses the file, it could produce the
  analysis along the way.

  **Analyses in the background, on demand or removed** (`[analysis]`, #564): exposure,
  sharpness, stack fingerprints and palettes, each `"background"` (the whole folder in the
  background), `"on-demand"` or `"off"` (`scheduler::Analysis`). On demand, the folder's
  tasks are still queued, but **only read from the cache**: the sweeper delivers those it
  finds (`Event::Ready`) and reports the others (`Event::Missing`, counted in the
  progress); no worker takes them. A request (`Scheduler::request`, the shown image) is
  always computed. Removed: no task. The old keys (`[scheduler] exposure`, `[sharpness]
  background`, `[similarity] background`) set to `false` mean `"on-demand"`. Changed in
  the settings window: `Scheduler::set_analyses`, then the folder is read again.

  The fitted render goes before the 1:1 so that the RAW look appears quickly; the 1:1 of
  the displayed image goes before the neighbours' preloading so that the sharpness of the
  image being looked at arrives as early as possible. A priority only applies to *pending*
  tasks: a started task is not interrupted (X-Trans demosaicing even less so), hence the
  importance of the order in which requests are sent (see [gui.md](gui.md)).
- **API**: `set_folder(files) -> generation`, `add_files(files)` (after the folder's,
  nothing interrupted: offload, issue #40), `set_focus(index)`, `set_shown(images)`,
  `request(index, level)`, `set_paused(bool)`, `progress(level)`, `wait_idle(timeout)`.
  `Drop` cancels and stops the threads.
- **Displayed images** (2026-10-17): a group, the main image first. `set_focus(n)` = the
  image and its neighbours (n, n+1, n-1: full screen); `set_shown` gives any group
  (side-by-side comparison: all the compared images, the active one first). The "displayed
  image" priorities and the requests apply to the whole group, in its order; whatever
  leaves the group is abandoned (`Dropped`).
- **Requests outside background work** (`request`): 1:1 (`Full`, `EmbeddedFull`) and RAW
  renders (`Standard`) of the displayed images: the image **and its neighbours**
  (preloading when navigating zoomed in), or the compared images. They do not count in the
  progress (`on_demand`). Priority: 1:1 first (displayed image, then next, then previous).
  Changing image abandons those that are no longer neighbours, cancels a running 1:1 for a
  distant image, and **signals it with `Event::Dropped`**: the interface thus knows it must
  request again if the user comes back. (Without this signal, an abandoned 1:1 was never
  requested again: bug fixed on 2026-10-16.) A request already in the cache simply yields a
  `Ready`.
- **`Config::raw_previews`**: `Background` (default: RAW renders of the whole folder in the
  background, after thumbnails and camera previews) or `OnDemand` (no background RAW render;
  used by the interface). Previews of flat images are always computed in the background.
- **Events**: `Ready`, `Failed`, `Progress`, all with the folder generation. Callback
  guarantees:
  - called **outside the state lock**: it can call back into the scheduler;
  - **never twice at the same time**, and **in the order** in which the events occurred.
    Workers drop their events into a shared queue (`outbox`), under the lock; a single
    thread at a time drains it (`deliver`). Without this, two workers delivering in
    parallel could make "6/6" arrive before "5/6": the progress went backwards (bug found
    by the tests, see below);
  - `set_folder` purges the undelivered events of the previous folder. Only an event being
    delivered can still arrive, and the interface ignores those of another generation. A
    background task running for an image that stays in the new list (refiltered folder)
    goes on at its new place, in the new generation; the others are cancelled (issue #73:
    each refilter dropped the analysis in progress).
- **`wait_idle`** only returns once **all events are delivered**: nothing pending, nothing
  running, event queue empty, no callback in progress. A panicking callback does not block
  `wait_idle` (`Drop` guard).
- **Cancellation**: a new folder cancels everything; changing image cancels a running 1:1
  for another image (effective only between two LibRaw steps, see `raw.md`).
- **`Producer`**: the computation trait; `CachedPreviews { cache, settings }` is the real
  implementation, and a fake producer makes it possible to test scheduling without LibRaw.

## "Fastest first" policy

`Config::thumbnails_first` (default `true`): *background* camera previews wait for the
thumbnails to finish, and *background* standard previews (RAW renders) wait for the
thumbnails **and** the camera previews to finish. The displayed image and its neighbours
n+1/n-1 always go through. This way the grid, then the whole folder in full screen, are
available as soon as possible, before starting the LibRaw renders that take all the cores.
Each finished task wakes the workers up, so that they pick up the previews as soon as the
last thumbnail is done.

## Background on hold (#555)

`set_background_paused`: background work (classes from background camera previews on)
waits; the shown image, its neighbours and the thumbnails still go through. A computation
requested on a folder ([compute.md](compute.md)) thus goes first.

## What is already cached goes through at once (issue #428)

One more thread, the **sweeper** (`tri-cache`), computes nothing: it takes the pending
tasks it has not checked yet, in priority order but **without** the "thumbnails first"
blocking, and looks whether the cache already has their entry (`Producer::is_done`, a
metadata read). The task **stays in the queue** while it is checked: a worker choosing at
that moment sees the priority order intact, and may take it (the sweeper then leaves it).
A first version took it out while checking, and upset the order (27 failures out of 150
runs of the ordering tests). If the entry is cached, the task is delivered at once
(`Ready`, progress); otherwise it is marked as checked (`State::checked`, cleared when the
folder changes), and the workers compute it in turn.

Why: after a change of a cache key (colour management, #423, put `cs=1` in the key of the
image levels), the first opening of a known folder recomputed all its thumbnails and camera
previews. Its grouping signatures, sharpness and exposures, still cached (their keys had
not changed), waited behind: the stacks formed step by step (TEST folder, 570 RAW).
Measured on 2026-11-12 on 27 RAW, thumbnails and camera previews removed from the cache:
signatures, sharpness and exposures delivered between **0.16 and 0.4 s** instead of 2.7 s,
thumbnails unchanged (~0.9 to 1.2 s). Bonus: a 1:1 already cached is delivered while the heavy worker renders
something else.

## What is already known in the cache, from one list to the next (#621)

Each refilter gives the engine a new list (`set_folder`), which checked the cache again
for every photo and level (video of 2026-11-27: ticking then unticking a filter in All
photos restarted the check of 6,737 photos of a NAS, thumbnails came back one by one, the
PC struggled). The engine now remembers, per photo and level, what it found ready
(`State::known`), with the file's stamp (size, date) at that time; on the next list, those
jobs are said ready at once (`Ready` and progress), without the disk. Safeguards:

- the stamp must be the one `cache::stamp` still remembers (`cache::known_stamp`, no disk):
  a changed file (watching, ratings written into the image, folder opened) is forgotten,
  so checked again;
- any removal from the cache since (clearing, recomputing, damaged entry, eviction, move:
  `cache::removals`) forgets everything;
- only the levels that depend on the file alone (`rememberable`): not developments nor
  their palettes (sidecar), not the 1:1 (evicted by size).

## Measurements

Real folder: 472 images (236 X-T4 RAFs + 236 camera JPEGs), 8-core Apple Silicon Mac,
release build, empty cache. Command:
`tri <dossier> --previews --cache <dossier vide> [--workers N] [--mixed]`.

| Policy | Light workers | Thumbnails (full grid) | Total |
|---|---|---|---|
| mixed, before optimising `fit` | 4 | 91 s | 210 s |
| mixed | 2 | 59 s | 206 s |
| mixed | 4 | 39 s | **173 s** |
| mixed | 8 | 36 s | 237 s |
| thumbnails first | 4 | 29 s | 227 s |
| thumbnails first | 8 | **19 s** | 252 s |

After switching to libjpeg-turbo (`turbojpeg` feature), several measurements per
configuration to estimate the variance (machine more or less warm):

| Policy | Light workers | Thumbnails | Total |
|---|---|---|---|
| thumbnails first (default) | 4 | **14–19 s** | **179–183 s** |
| thumbnails first | 8 | 12 s | 190 s |
| mixed | 4 | 27–38 s | 187–209 s |

With faster decoding, "thumbnails first" wins on both counts: it is the default, the other
policy remains available (`Config::thumbnails_first`, `--mixed`). The total is dominated by
the 236 RAW previews of the heavy worker (~0.5 s each).

Second pass (everything cached): ~10 ms.

### Why the camera preview (2026-10-16)

RAW renders cannot go any faster: decompressing an X-T4 RAF already takes all the cores
(LibRaw parallelises it), and nothing can be gained elsewhere:

| Attempt | Result |
|---|---|
| previews at 1280 px instead of 2560 | 139 s instead of 176 s (−21 %), cache 4× lighter |
| 1 / 2 / 4 / 8 `half_size` renders in parallel | 600 / 573 / 1,046 / 1,440 ms per RAF |

Hence the camera preview: the embedded JPEG (4416×2944 on X-T4) downscaled to the standard
size. Measured on the test folder: **thumbnails at 14 s, the whole folder viewable in full
screen at 35 s**, RAW renders done at 180 s (total unchanged). Cost: one more cache level
(197 MB for 236 RAFs at 2560 px). Lead: ~0.25 s per camera preview, mostly the Lanczos3
downscaling from 4416 to 2560 px; a simpler filter would be worth measuring. Cache size:
13 MB of thumbnails (28 KiB on average), 397 MB of standard previews at 2560 px (861 KiB
on average).

Interpretation:

- **competition with LibRaw dominates**. A thumbnail costs 140 to 250 ms on its own, but
  more than 300 ms per worker while a LibRaw render takes all the cores. Hence the gain of
  the "thumbnails first" policy on the grid;
- on the other hand it lengthens the total: the heavy worker does nothing during the
  thumbnail phase, and the overlap between the two phases is lost;
- more light workers only help if LibRaw is not running at the same time.

## Tests

Unit tests: priorities, heavy/light routing, progress fraction.

Integration tests (`tests/scheduler.rs`), with a **fake producer** that records the tasks,
simulates durations, failures, entries already cached, and measures concurrency per worker
type:

- each file processed once per level, complete progress, no 1:1 without a request;
- events in order, strictly increasing progress (35 files, 4 workers);
- cached entries reported without recomputation; failures reported and counted;
- exact order of the heavy worker around the displayed image (5, 6, 4, 7, 3, 8, 2, 9, 1, 0);
- RAW renders never simultaneous, light tasks in parallel;
- on-demand 1:1, cancelled when the displayed image moves away (< 2 s), signalled by
  `Dropped`, then completed for the new one; neighbours' 1:1 preloaded and kept from one
  step to the next; every abandoned request signalled;
- new folder: the old one is abandoned, none of its results under the new generation;
- "thumbnails first": before the last thumbnail, only the displayed image and its
  neighbours have a preview; everything ends up computed;
- camera previews for RAWs only (progress = number of RAWs); background RAW renders never
  before the camera previews are done; the camera preview of the displayed image goes
  before all thumbnails;
- pause / resume; out-of-range displayed image ignored; quick stop on destruction;
- sweeper (#428): cached signatures and sharpness delivered before most of the thumbnails
  to recompute, without recomputing anything; a cached 1:1 delivered without waiting for
  the render in progress;
- with `TRI_TEST_RAW`: real producer on a RAW and a JPEG, sizes and orientation of the
  entries, second pass served by the cache (0.66 s then ~40 µs).

These tests revealed two races, now fixed: `wait_idle` could return before the last events
were delivered, and progress events could arrive out of order. To make sure neither comes
back, the test file was run 300 to 500 times in a row, without a failure.
