# Module `tri::cache`

Persistent on-disk cache of computed images: thumbnails, standard previews, 1:1 renders.
The principle and the choices (three levels, half_size, central location) are described in
`CLAUDE.md`, section "Aperçus et cache". This module only **stores and retrieves**; it
computes nothing. The work queue that fills it in the background sits on top.

## On-disk layout

```
<racine>/
└── v4/                      format version (FORMAT_VERSION)
    ├── thumbnail/
    │   ├── 8c41…07/         one folder of photos (start of the hash of its path)
    │   │   ├── .source      that folder of photos, in plain text
    │   │   └── 3f9a…e1.jpg  one entry = one quality-90 JPEG
    │   └── …
    ├── embedded/            previews taken from the camera JPEG (RAW only)
    ├── standard/
    ├── embedded-full/       embedded JPEG at its original size (1:1 of the camera JPEG)
    ├── full/
    └── exposure/            exposure analyses (.bin, ~14 KB: histograms, mask)
```

- **One file per entry**, with no database or index: the state of the cache *is* the
  content of the folder. Nothing to rebuild, nothing to corrupt beyond a single file.
- **One subfolder per folder of photos** (2026-10-23, format `v2`): its name is the
  start (16 characters) of the hash of the photos' folder path. Clearing the cache of one
  folder (`clear_folder`, `tri <folder> --clear-cache`) removes that subfolder in each
  level, without having to recompute the keys: they depend on the settings, and the
  standard preview size on the screen, which the command line does not know. Before, the
  subfolders were the first 2 characters of the entry's hash.
- **`v4/`**: changing the format (or the key algorithm) is done by incrementing
  `FORMAT_VERSION`. Old entries become invisible; the previous formats listed in
  `OLD_FORMATS` (`v1/` to `v3/`) are deleted in the background when the cache is opened.
  `v3` (2026-10-24): key without the folder, `.source` file (see below). `v4`
  (2026-10-25): modification date to the quarter hour (see the key).

## Cache on the photos' drive (2026-10-24, issue #55)

Three possible places for a photo's cache:

- **the user's** (above): photos that do not move;
- **at the root of a removable drive**: a transfer drive, plugged into several
  computers;
- **in a folder**: a folder that gets moved.

The last two are one mechanism: a hidden `.tri-cache` folder (`DISK_DIR`) keeps the cache
of all the photos below it, subfolders included, with the same layout
(`.tri-cache/v4/<level>/<folder>/`), but the folder is identified by its path
**relative** to the root (written with `/` on every system). Mounted elsewhere
(`/Volumes/T7` on a Mac, `/media/u/T7` on Linux), the drive finds its cache again.
`Cache::disk_of(folder)` looks for the nearest `.tri-cache` going up (remembered per
folder, forgotten at every change).

- `use_disk(root)`: creates `.tri-cache` and **moves** the entries already computed into
  it (central cache, or a `.tri-cache` higher up): nothing is computed again;
- `release_disk(root)`: the reverse, the entries go where their photos now expect them
  (a `.tri-cache` higher up, otherwise the user's);
- `relocate(from, to)`: after a folder is moved or renamed, its entries follow it (a
  `.tri-cache` in the folder itself has left with it);
- `apply(location, folder, removable)`: the `[cache] location` setting (`user`, `disk`,
  `folder`) applied to an opened folder whose cache is still the user's; a choice already
  made is respected (`wanted_root`, `places::removable_roots`).

The `.source` file of each folder of entries (absolute path in the central cache,
relative on a drive) makes these moves possible without knowing the settings. To another
drive: copy then delete; an entry already present at the destination is kept. The 1:1
levels of a `.tri-cache` are not limited by `zoom_max_mb`, and `--cache-usage` counts the
central cache only. FAT32 drives: the modification date is in local time there, shifted
by a clock change or a time zone; the key no longer depends on it since `v4` (see the key).

### Default location: `default_dir()`

| System | Folder |
|---|---|
| Linux / Unix | `$XDG_CACHE_HOME/tri` if the variable is an absolute path, otherwise `~/.cache/tri` |
| macOS | `~/Library/Caches/tri` |
| Windows | `%LOCALAPPDATA%\tri` |

The cache is **never written next to the photos**: SD cards, read-only disks, NAS.
These folders are the ones the systems provide for recomputable data: excluded from Time
Machine backups on macOS, cleanable by the user.

## Keys: `Key::new(source, level, params)`

A key is a **BLAKE3** hash (256 bits, in hexadecimal) of:

1. the format version;
2. the **name** of the source file (since `v3`; the folder chooses the layout instead:
   the cache can thus change place without computing anything again);
3. its **size** and **modification date** (nanoseconds), reduced to the remainder of its
   division by 15 minutes (since `v4`, issue #74): a FAT32 drive stores its dates in local
   time, and the switch to winter time shifted them all by one hour, so the whole cache of
   a USB stick was computed again. Clock changes and time zones always shift by a multiple
   of 15 minutes; a real change alters the size, or the date by something else;
4. the **level** (`Thumbnail`, `Embedded`, `Standard`, `EmbeddedFull`, `Full`);
5. the computation **parameters**, a free-form string supplied by the caller (e.g.
   `"half=1;side=2560"`).

Consequences:

- **file modified** (edited, rewritten) ⇒ new key ⇒ automatic recomputation; size and
  date are read **once** per file then kept (`cache::stamp`, 2026-10-29: on a NAS, each
  read waited behind the RAWs read by the engine, and the interface did thousands of
  them, frozen up to 37 s); the folder watch forgets the stamps of changed files
  (`forget_stamps`), opening a folder those of its files (`forget_stamps_under`). The same
  goes for the sidecars of development software (`develop::stamp`, `darktable_sidecar`).
  While the grid is drawn, missing stamps are **deferred** (`defer_stamps`,
  `lookup_stamp`): read by a separate thread, the interface redrawn when they arrive
  (`on_stamps_read`); the thumbnail arrives one frame later instead of freezing the page
  (still 0.4 to 3 s per page on a NAS). Actions keep exact stamps;
- **settings changed** (preview size, white balance…) ⇒ same, provided the caller
  includes them in `params`;
- **folder moved or renamed** ⇒ the cache is no longer found, unless it follows
  (`relocate`: renaming and moving from `tri`, "Bring the cache along" checkbox) or it is
  in the folder (`.tri-cache`); checking the content would mean re-reading every ~30 MB
  RAW;
- the old entry is **not deleted**: it remains until clearing or eviction.

Each field is prefixed with its length before being hashed: (`ab`, `c`) and (`a`, `bc`)
cannot produce the same key.

Why BLAKE3 rather than the standard library's `DefaultHasher`: the latter does not
guarantee an identical result from one Rust version to the next, which would invalidate
the cache with every compiler update. A unit test (`key_hash_is_stable`) pins a reference
value to detect any accidental change.

## `Cache` API

| Method | Role |
|---|---|
| `Cache::open(root)` | Opens a cache in `root`, creating the folders if needed |
| `Cache::open_default()` | Same in `default_dir()` |
| `with_quality(q)` | Same cache, JPEG quality `q` (1 to 100, default `DEFAULT_JPEG_QUALITY` = 90) for entries written from now on; not part of the key, see [settings.md](settings.md#settings-and-cache) |
| `contains(&key)` | Does the entry exist? (without decoding it) |
| `get(&key) -> Option<Bitmap>` | Reads and decodes the entry |
| `get_bytes(&key)` / `put_bytes(&key, &bytes)` | Binary entry of an analysis (`Level::Exposure`, `Level::Sharpness`, `.bin` file), same atomic write; `Level::is_image` tells the two kinds apart |
| `put(&key, &bitmap)` | Writes the entry (JPEG at the cache's quality, via [`jpeg`](jpeg.md): 4:4:4 for 1:1, 4:2:0 elsewhere), overwriting the old one |
| `remove(&key)` | Deletes an entry (no error if absent) |
| `clear(Some(level))` / `clear(None)` | Clears one level / the whole cache |
| `clear_folder(folder, level)` | Clears the entries of the images of `folder` (not its subfolders), one level or all, whatever the settings they were computed with; returns what was freed |
| `usage(level) -> Usage` | Bytes and number of entries of a level |
| `evict(level, max_bytes) -> Usage` (wired up on 2026-10-17: `EngineSettings::trim_cache` limits each 1:1 level to `[cache] zoom_max_mb`, at interface startup in a separate thread and at the end of `tri --previews`; until then, 1:1 renders piled up without limit) | Deletes the least recently used entries down to `max_bytes`, returns what was freed |

### Robustness

- **Atomic write**: `put` writes to a temporary file in the same folder
  (`<hash>.jpg.<pid>-<n>.tmp`), syncs it to disk, then renames it. A reader sees the old
  entry or the new one, never half of one, even if the program is killed mid-write.
  Orphaned temporaries are ignored by `usage` and `evict`.
- **Self-repair**: an unreadable entry (truncated JPEG, damaged disk) is deleted by
  `get`, which returns `None`: the caller recomputes it like a missing entry.
- **Sharing**: `Cache` keeps no state in memory (just the path). It is `Clone`, usable
  from several threads, and even several processes, on the same folder.
- **Last use**: for a 1:1 (the only evicted levels), `get` updates the file's modification
  date. `evict` uses it as the last-use date, which avoids having to maintain an index.
  Not elsewhere (issue #74): one write per read slowed everything down on a USB stick.

## Sizes and performance

JPEG quality 90: a few tens of KB for a 400 px thumbnail, around 0.5 to 1 MB for a
2560 px preview, 10 to 15 MB for a 26 Mpx 1:1. These figures are indicative and will be
measured once the work queue fills the cache.
