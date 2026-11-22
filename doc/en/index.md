# Palette search index (`tri::index`)

Step 1 of issue #561: searching palettes across **all folders**, not only the open one. The
next steps (results grid, "Find…" for a moved folder, watched roots) will build on this
module.

## Principle: a search cache, not a library

The user does not want a "library" visible from outside: a Lightroom-style catalogue
breaks as soon as a folder is moved outside the application (in darktable, from an
internal disk to a storage disk…). Hence:

- **Only recomputable data**: the palettes. Ratings, versions, etc. stay in the XMP
  sidecars. The index can be deleted at any time: searches are partial until the next
  computations. Nothing is written next to the photos.
- **Identity by content, not by path.** Each photo has a **fingerprint** (`fingerprint`):
  its size and a BLAKE3 hash (truncated to 16 bytes) of its first and last
  `fingerprint_kb` KB (32 by default). A palette depends only on the content and on its
  parameters (`preview::stamp`: palette settings and, for a software's render, the state of
  the development in the sidecar). A moved photo therefore keeps its palette; its new path
  is simply recorded when `tri` sees it again.
- **Nothing is tracked** (moves, copies, deletions), in `tri` or elsewhere:
  - moved: the old path leads nowhere, which will show when a result is displayed;
  - copied: two paths, one fingerprint, so two results;
  - modified: another fingerprint; the old entry becomes orphaned.

## The database

SQLite (`rusqlite`, SQLite compiled with the project), file `index.sqlite` in `tri`'s
**data folder** (`[data] dir`), not in the cache, which gets cleared. WAL journal: reads
while writing. Two tables:

| Table | Key | Content |
|---|---|---|
| `paths` | path (system bytes; UTF-16 on Windows) | size, modification date (ns, as in the cache keys), fingerprint (empty until a palette is computed) |
| `palettes` | (fingerprint, source) | parameters stamp, palette (cache format, `Palette::to_bytes`) |

The size and date kept with each path will let a result's thumbnail be found in the cache
**without reading the file again** (fast on a NAS or an unplugged drive). A database of
another schema (`user_version`) or an unreadable one is rebuilt.

## Every photo seen (#561, schema 2)

For "All photos" (a virtual root above every location, where the filters act as searches),
the index records **every photo seen**, not only those with a palette:
`index::seen(folder, levels, images)` when a folder is opened (interface) and when a
folder is computed ("Compute"). Path, size and date only: the fingerprint is read only
when a palette is computed. Photos of those levels that are no longer there are forgotten
(deleted, or moved: seen elsewhere, they will be recorded there); a modified photo (other
size or date) loses its fingerprint. Developments (`IMG@dt-01.RAF`) are not files: not
recorded, they are found again by reading the folder. `Index::photos(under)` lists them
without walking any folder. Moving to schema 2 rebuilds the database once: palettes
already cached are recorded again by themselves.

## Similarity fingerprints (schema 3)

The stacks' fingerprints (`similarity::Signature`, ~70 bytes) are recorded with each path
(`paths.signature`, and `signature_stamp`: their parameters) as soon as they are ready
(`index::note_signature`, scheduler and "Compute"). They follow the file: a modified file
loses them, a repaired folder keeps them. "All photos" loads them all at once when it
opens (`Index::signatures`): "similar" everywhere without reading the cache photo by
photo (100,000 photos: ~7 MB). Moving to schema 3 rebuilds the database once.

## Filling

Every **ready** palette, computed or found in the cache, is reported through
`index::note`:
- by the scheduler (`scheduler::finish`), so when a folder is opened, whether its palettes
  are computed or already cached;
- by chosen computations (`compute::run`, right-click › Compute, `tri --compute`).

A **separate thread** (`index::start`, `tri-index`) writes in batches (at most `batch`, one
transaction): the computation never waits for it. A path already known at the same size
and date, with the palette at the same parameters, only costs a database read; otherwise
the fingerprint is computed (at most 2 × 32 KB read) and the palette read back from the
cache. `index::flush` waits until everything is written (command line, before exiting).
The interface starts the index with the engine (never in tests); `--compute` too, when it
computes palettes.

## `[index]` settings

| Key | Default | Role |
|---|---|---|
| `enabled` | `true` | record palettes in the index |
| `fingerprint_kb` | 32 | KB read at the start and end of a file for its fingerprint |
| `batch` | 256 | palettes recorded per write, at most |

## Measurements (2026-11-22)

100,000 synthetic palettes (6 colours, coverage of 96 × 96 noisy points), Apple Silicon Mac,
release (`cargo test --release --test index -- --ignored --nocapture`):

| | |
|---|---|
| palette | 2.4 KB on average (real ones: ~1.2 KB, measured on 131 photos, #562) |
| database | 429 MB |
| writing, batches of 256 | 48 s (~0.5 ms per palette, in the background) |
| reading everything | 0.34 s |
| "similar palettes" over everything | 96 ms |
| "contains this colour" over everything | 68 ms |

Searching in memory is enough (under 0.1 s, plus loading, once). The database size is the
point to watch: compressed or coarser coverage in the index, to decide on real photos.

## Tests

- `src/index.rs`: moved photo (one more path, same palette), palette recomputed with other
  settings (replaced), fingerprint (only the ends of the file), database of another schema
  rebuilt, non-ASCII paths.
- `tests/index.rs`: end to end, real palette computation, index filled by the writer
  thread, nothing new on a second computation, moved photo found again; the measurement
  above (`#[ignore]`).
