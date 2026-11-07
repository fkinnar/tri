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
- "Read the ratings back from the images" (`NoteBook::import`) settles in favour of the
  image, and copies a rating merged from it into the sidecars.

## Command line

`tri <folder> --conflicts`: merges made, and each image in conflict with its sources and
their ratings.

## Interface

- A merge is told in the status bar ("Ratings merged from other programs for N images").
- An image in conflict carries an orange triangle at the top left of its thumbnail
  (`marks::conflict_sign`); a click, or a rating set on it, opens the "Ratings disagree"
  dialog: a table, one row per source (XMP, RapidRAW, RawTherapee, ART, "In the image")
  and the previous ratings, one column per field. Only the fields in disagreement (in
  bold) are chosen, by clicking the value wanted; the others are already merged
  (`Conflict::merged`, feedback of 2026-11-07: a colour added in the image must not
  depend on the choice of stars). The "Result" row shows the ratings that will be kept;
  "Apply", or "Keep" (every field in disagreement of a row). "Same choice for the N other
  images in conflict in the folder": the same kind of source, field by field. One history
  step, undoable.

## To come

An "in conflict" filter, then automation (write everywhere, merge
on the fly on changes seen by watching). Renaming or moving an image does not carry its
entry yet: it is read again as the first time.
