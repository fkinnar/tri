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
computes palettes. The folder opened at launch, read before the writer thread exists, is
recorded when the engine starts.

**Camera cards are never indexed** (feedback of 2026-11-22: raw files, before they are
copied elsewhere): nothing under a `DCIM` folder (`locate::is_camera_card`).

## Finding a moved folder (`tri::locate`)

In "All photos", the completion from disk (`folder::complete_from_disk`) also returns the
**unreachable** folders; their photos stay, **greyed**, with the reason on hover: **disk
unplugged** (the first existing parent is a mount folder, `/Volumes`,
`/media/<user>`…: `locate::unplugged`, plugging it back in is enough) or **not found**
(moved or renamed on a disk that is there).

**Find its folder** (`core.file.locate`, `g l`, right-click, folder-and-magnifier icon)
searches in the background (`locate::find`) under the disks, the home folder and the
favourites:
1. the old path placed on each root, from its end towards its start
   (`…/2026-11-18_Sarah`, `…/2026/2026-11-18_Sarah`…): a folder moved from one disk to
   another often keeps its structure, and these tries cost nothing;
2. then the folders with the same name, down to the grid's subfolder depth
   (`[gui.folders] subfolders_depth`, feedback of 2026-11-22: the same setting), hidden
   folders and cards left out.

Each candidate is **checked** as soon as it comes up (`locate::verify`): three sample
photos of the old folder (first, middle, last by name, `Index::photos_in`) must be there,
at the same size, and with the same content when the index has its fingerprint. Not
found: a box asks where it is (Tab completion, "Browse…"), checked the same way ("This
folder does not hold the same photos"). Once found, **everything follows**
(`locate::relocate`): the index (`Index::relocate`, subfolders included, fingerprints and
palettes kept), the cache (`Cache::relocate`) and the sorting data kept on the machine
(`store::relocate`); "All photos" is read again. A folder moved or renamed **inside
`tri`** takes its index along too.

**Photos moved, renamed or copied by `tri`** (`files::rename`, `files::transfer`, like the
notes, `reconcile::carry`): the index follows them (`index::carry`, `Index::carry`), even
into a folder never opened. Moved, a photo keeps everything; copied, the copy takes the
original's content fingerprint (its palettes follow). A photo moved **outside `tri`** on
its own leaves "All photos": its folder is there, read again, and it is no longer in it;
the watched roots will find it at its new place. A folder **that no longer exists** is
never recorded as "seen empty" (reopened at launch after a rename, it erased everything
the index knew about it): its photos stay, greyed.

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
