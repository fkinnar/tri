# Module `tri::store`: a folder's data

What `tri` remembers about a photo folder, apart from the ratings (always in the XMP
sidecars), and where it keeps it (2026-11-11, issue #378). Use case: offload a card to a
USB key or an external SSD and carry that drive from one computer to another, or sync a
folder (Syncthing, a cloud): everything `tri` needs must travel with the photos.

## One small file per kind

| File | Content | Written by |
|---|---|---|
| `notes.json` | Ratings memory: what was last read from each source, the folder's "ratings in the images" setting | `reconcile::Memory` ([reconcile.md](reconcile.md)) |
| `view.json` | Filters, sort, thumbnail render | the interface (`gui/src/state.rs`, `FolderView`) |
| `tuning.json` | Slider values: stack strictness, pauses, curve (`group-*`), blur severity (`blur-strictness`), exposure strictness (`clip-over-strictness`, `clip-under-strictness`) | `store::value` / `store::set_value` |
| `choices.json` | Stack covers (`covers`) and preferred developments (`developments`), by **file name** | `store::chosen` / `store::choose` |

One file per kind: when two computers change the same synced folder, a sync conflict
only touches the file concerned. File names only, never an absolute path: a drive mounted
at `/Volumes/T7`, `/media/u/T7` or `E:\` keeps everything. A file that goes back to its
default value is deleted.

No lock (it protects nothing between two computers working offline): every write
re-reads the file just before (`update_json`), changes only its own key, and replaces the
file at once (temporary file, then rename: `write_atomic`).

## One switch: "keep tri's data in this folder / on this drive"

All or nothing (owner's decision): **the cache and the data together**.

- **On**: the file `.tri/keep` in a parent folder, or the folder itself (`holder(folder)`,
  remembered per folder, forgotten at every change: `changed`). Every folder below keeps
  its files in its own `.tri/` (a moved folder takes its data along), and the cache of
  everything below goes into `<root>/.tri/cache` ([cache.md](cache.md)). The marker is a
  file, not the presence of `.tri/cache`: someone who excludes the cache from a sync
  (one line, `.tri/cache` in `.stignore`) keeps the switch on the other computer.
- **Off** (default): nothing is written in the photo folder; the files go to `tri`'s data
  folder, `folders/<fingerprint>/` (fingerprint of the folder's path; a `folder` file
  gives the path back, to find a drive's folders). Same fallback for a **read-only**
  folder (locked card, NAS) even when the switch is on.
- Without a data folder (tests, tools): the folder's `.tri/`, rather than nowhere.

`keep(data, root)` sets the switch and moves the data of every folder below that were
kept on the machine; `release(data, root)` removes it and brings them back (walk of the
drive). Both are called by `Cache::use_disk` / `Cache::release_disk`, which move the
cache: from the interface (right click on a folder or a drive, offload dialog), the
command line (`--cache-here` / `--cache-user`) and the `[cache] location` setting.

## Tidying on opening: `settle(data, folder)`

Called when a folder is opened (`Folder::open`, `Folder::scan_counting` for the folder
itself, `Loading` for the subfolders of its images):

- a cache kept with the photos the old way (`.tri-cache`, in the folder or a parent) was
  an explicit choice: renamed to `.tri/cache`, marker `.tri/keep` set;
- files on the wrong side (switch changed here or by a sync, `.tri/notes.json` written by
  default before 2026-11-11) move to the other side; present on both sides, the more
  recent wins;
- what the data folder kept for this folder under its absolute path before 2026-11-11
  (`folder-views`, `*-strictness.tsv`, `group-*.tsv`, `stack-covers.txt`,
  `preferred-developments.txt`, `notes/<fingerprint>.json`) joins its files and leaves the
  old files (deleted once empty).

At best: what cannot move (read-only) stays where it is. `relocate(data, from, to)`: after
a folder is renamed or moved from `tri`, its data kept on the machine follow (those of its
`.tri/` have left with it).

## What stays on the machine

Favourites, settings, keys, network shares, last folder, "Subfolders" checkbox, recent
destinations, export log, palette history: they belong to the computer, not to a folder.

Showing where a folder's data live ("on the drive" / "on this computer") is planned in
the folder summary (#388).
