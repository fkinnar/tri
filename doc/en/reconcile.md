# Module `reconcile`: ratings coming from several sources

`src/reconcile.rs` (issue #368, 2026-11-07).

A photo can carry its ratings in several places: `.xmp` (darktable, Lightroom…),
`.rrdata` (RapidRAW), `.pp3` / `.arp` (RawTherapee, ART), the image itself
([embedded.md](embedded.md)). Each program changes only one. Before, `tri` read the most
recent sidecar: a change made elsewhere could silently vanish at the next rating.

## What each source changed

For each image and **each source**, `tri` keeps what it read (or wrote) there last time,
with the file's date and size ([`Stamp`]), and the ratings it settled on. On reading
(`reconcile`):

- a source whose date and size have not moved changed nothing;
- otherwise, its fields that differ from what was read there are **its** changes: a JPEG
  retouched elsewhere without touching its ratings changes nothing, even if its ratings
  are behind;
- **field by field merge** (stars, flag, each colour): changed by one source, or by
  several the same way → adopted (`Outcome::Merged`, with the sources); differently →
  **conflict** (`Outcome::Conflict`);
- a merge of colours no source carries (two colours added one on each side) → conflict:
  none is invented in single-colour mode;
- an **unknown source** (first reading, sidecar created by another program) that
  disagrees → conflict: we ask, without guessing from dates (FAT cards accurate to 2 s,
  NAS clocks). Meanwhile, the most recent ratings are shown;
- a **silent** image (it could carry ratings but has none) is kept without counting in
  agreements: a rating that appears in it later (Windows Explorer, XnView…) is a known
  change, merged.

After a write by `tri` (rating, undo, resolving), what it just wrote is authoritative
(`notes::remember`): a source left behind (the image, not rewritten) is no disagreement.

## The memory: `.tri/notes.json`

Next to the photos, one file per folder, indexed by file name (travels with the folder,
easy to delete when testing). Created only when an image has ratings, removed when it
becomes empty again; `.tri` folder hidden on Windows (attribute), ignored by the image
scan, subfolders and watching (dot name). Read-only folder: fallback in `tri`'s data
folder (`notes/<fingerprint>.json`, `reconcile::set_data_dir` at start-up).

Not in the XMP: darktable rewrites its XMP from its database and may erase our fields.

## In `notes.rs`

- Each reading (`read_notes`) takes every source: the sidecars, and the image
  (`embedded`) if it can carry ratings.
- An image in conflict shows its previous ratings; `NoteBook::edit` is **refused** there
  (rating would overwrite a source unknowingly); `NoteBook::resolve` settles it (ratings
  written to every sidecar, the pair follows, and kept).
- `NoteBook::conflict`, `conflicts`, `take_merged` (merges made since the last call, for
  a discreet message).
- **A conflict is never settled silently** (bug of 2026-11-07: "Read the ratings back from
  the images" settled it in favour of the image): rating (`edit`), undo (`write_only`),
  reading back from the images (`import`), writing into the images (`embed`),
  regenerating sidecars (`generate`) refuse an image in conflict ("resolve that first");
  the interface then opens the dialog. Only `resolve` settles.
- "Read the ratings back from the images" (`NoteBook::import`) copies a rating merged from
  the image into the sidecars.

## Command line

`tri <folder> --conflicts`: merges made, and each image in conflict with its sources and
their ratings.

## Interface

- A merge is told in the status bar ("Ratings merged from other programs for N images").
- An image in conflict carries an orange triangle at the top left of its thumbnail
  (`marks::conflict_sign`); a click, or a rating set on it, opens the "Ratings disagree"
  dialog: a table, one row per source (XMP, RapidRAW, RawTherapee, ART, "In the image")
  and the previous ratings, each source's modification date (`Conflict::modified`, local
  time: a hint), one column per field. Only the fields in disagreement (in
  bold) are chosen, by clicking the value wanted; a field where rows differ without a
  conflict (only one source changed it) is preselected on the merged value, and can be
  changed too (feedback of 2026-11-07: a different colour that could not be chosen); the
  others are already merged
  (`Conflict::merged`, feedback of 2026-11-07: a colour added in the image must not
  depend on the choice of stars). Alternating rows, fixed-width columns, cells drawn by
  hand (values centred). A row's thick arrow (`marks::take_row_arrow`) chooses every field
  in disagreement there, without applying; the "Result" row, in the accent colour (it is
  not clickable), shows the ratings that will be kept; "Apply" at the bottom right. "Same choice for the N other
  images in conflict in the folder": the same kind of source, field by field. One history
  step, undoable.

- "In conflict" filter (`f c`, `core.filter.conflicts`) in the Filters section of the
  right panel, below the others (a secondary filter: not in the top bar), only when
  there are conflicts (`notes::ConflictFilter`).
- The thumbnail's triangle and clipping icons are on one row, centred on the height of
  the marks opposite.

## Renamed, moved, deleted files

`files::rename`, `files::transfer` (copy or move) and `files::delete` carry the memory
(`reconcile::carry`, `reconcile::forget`): the image's entry changes name and folder, its
sources renamed like them. Without it, an image with a source left behind (the image,
not rewritten) would be in conflict at its new place. A copied or moved folder takes its
`.tri` with it. Undo goes through the same functions.

## Automation (2026-11-07)

- **Writing by itself** (`[xmp] embed`): `never` (default), `internal` (internal and
  fixed disks; not a removable disk or a network share, `places::outside_roots`),
  `everywhere`. On each rating (and undo, settling a conflict), the ratings also go into
  the image (`NoteBook::embeds`). Never an image in conflict.
- **Folder setting**, which wins ("this removable disk, yes"): as in the settings,
  always or never; kept in `.tri/notes.json` (`embed` field, `Memory::embed`), so it
  travels with the folder; the folder alone, not its subfolders (exceptions set
  knowingly). Right-click on the folder, "Ratings in the images"; a mark in the folder
  panel (accent: always; grey crossed out: never). `NoteBook::folder_embed`,
  `set_folder_embed`.
- **Merge on the fly**: watching reads the folder again when an image or a sidecar
  moves; the merge happens on reading, and a conflict shows the triangle.
- **Propagation**: a merge coming from one source is written at once into the others
  (existing sidecars; the image if it receives ratings by itself), where `tri` writes by
  itself (`NoteBook::writes_automatically`, `propagate`).
- After its own writes into images, `tri` resets watching (`own_writes_done`): no
  reading the folder again as for a change made elsewhere.
- Dropped for now: a "ratings in the files, no sidecar" folder.
