# Clean-up — `tri::purge`

Issue #604 (2026-11-26, the owner's idea: some photographers have a lot of images).
Three things grow without limit; the clean-up keeps them to a reasonable size, never
losing anything that could still be useful.

| What | `[purge]` setting | Default | What goes |
|---|---|---|---|
| Offload history (`offload-history.tsv`) | `offload_history` | `"1y"` | the entries of older photos (shooting date) |
| Cache | `cache_unused` | `"never"` | the previews of folders not opened for that long |
| Cache | `cache_gone` | `true` | the previews of folders that no longer exist |
| Sorting data (`folders/<hash>/`) | `folder_data_gone` | `true` | that of folders that no longer exist |

Durations to choose from: `"1w"`, `"1m"`, `"3m"`, `"1y"`, `"3y"`, `"never"` (a dropdown in
the settings window, "Clean-up" section).

## Offload history

A card coming back with photos older than a few weeks is very unlikely.
`History::purge(before, dry_run)` removes the entries of photos taken before the limit
and appends them to the **archive** next to it, `offload-history-archive.tsv` (same
format, never read again: the record of where each file was copied). Unreadable lines
stay. "Forget this card" (#602) archives nothing: the point is to really forget a test
card.

## Cache

`Cache::purge(unused_since, gone, dry_run)` walks the central cache, one entry folder at a
time: each has its `.source` marker (the photos' folder).

- **Gone folder**: see [`gone`](#a-folder-that-cannot-be-found-is-not-necessarily-gone).
- **Not opened since**: the marker's date, updated each time the folder is opened
  (`Cache::note_use`, in the background: one small write per level, not per image read;
  reading an entry writes nothing, issue #74). A folder never reopened since this version
  counts from the creation of its cache.
- Caches kept **on a disk** (`.tri/cache`) are never touched: they travel with it.

Off by default for age: computing again takes time, choose according to space.

## Sorting data

`store::purge_gone(data, gone, dry_run)` deletes the fallbacks (`folders/<hash>/`, with
their `folder` file) of gone folders. A `.tri/` in a folder or on a disk is never touched.

## A folder that cannot be found is not necessarily gone

Unplugged disk, removed card, offline NAS, share not mounted: `gone(folder)` only says it
is gone if it no longer exists **and** its nearest existing parent is none of:

- a mount container (`/Volumes`, `/media`, `/media/<user>`, `/run/media/<user>`, `/run`, `/mnt`,
  `/net`, GVFS, `Library/CloudStorage`);
- an **empty** folder (mount point of an NFS NAS whose volume is absent);
- the root of a disk (too uncertain);
- or nothing (absent Windows drive).

A relative folder is never said to be gone.

## When

- **Automatic**: in the background, 20 s after the interface starts (the folder opens
  first), at most once every `interval_hours` (24; 0: never), remembered by the
  `last-purge` file in the data directory. The report goes to the log.
- **By hand**: "Clean up…" in the Cache section of the settings window: what would go
  first (dry run), then "Clean up" for real.
- **Command line**: `tri --purge [--dry-run]`.

## API

- `PurgeSettings`, `Keep` (`duration`, `cutoff`);
- `gone(folder)`, `due(data, settings, now)`, `mark_done(data)`;
- `run(settings, cache, data, history, now, dry_run) -> Report` (archived entries, freed
  cache, folders, errors);
- `offload::History::purge`, `offload::archive_path`, `offload::history_path`;
  `Cache::purge`, `Cache::note_use`; `store::purge_gone`.
