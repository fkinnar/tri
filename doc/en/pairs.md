# Grouped RAW+JPEG pairs (`tri::pairs`)

Issue #392, decided on 2026-11-09. A card set to RAW+JPEG gives two files per photo
(`DSCF0001.RAF` and `DSCF0001.JPG`). The JPEG is only a development made in the camera:
**grouped**, the pair is **one thumbnail** in the grid, and the JPEG is filed with the
RAW's developments (darktable, RapidRAW…), with a "Camera" chip. Unfold the development
stack (Shift+V) to see it, like a darktable clone.

Nothing changes on disk: the JPEG stays a separate file. Its ratings are already shared
with the RAW; copy, move, rename and delete already carry the pair (`[files] pairs`).

## Decision per folder

- **Global setting** `[files] group_pairs`: `ask` (default), `separate` or `grouped`.
- **The folder's decision**, kept with its culling data ([store.md](store.md)): it
  travels with the folder when "keep the culling data here" is on. It always wins over
  the global setting.
- With `ask`, opening a folder that has pairs and no decision yet opens the "Group RAW
  and JPEG?" dialog (the grid stays separate meanwhile): "Group", "Keep separate", and a
  "Don't ask again" checkbox, which writes the answer as the global setting. Closed
  without an answer: asked again next time.
- To change one's mind: the open folder's menu (right-click, Files group), or the
  palette, action `core.dev.pairs` (no default key).
- Subfolders shown with the folder: each image follows its own folder's decision.

## Engine

- `Grouping`: the setting's three values.
- `decided(data, folder)` / `decide(data, folder, Some(bool) | None)`: the folder's
  decision, kept in its values (`tuning.json`, key `pairs_grouped`).
- `grouped(setting, data, folder)`: the decision, else the setting.
- `camera_jpegs(files)`: the flat images that have a RAW of the same name (ignoring
  case) in the same folder, with that RAW. Also used by the folder summary
  ([folder.md](folder.md)).

## Interface

Development stacks gather the images that have the same photo: for the JPEG of a
grouped pair, that photo is its RAW (`TriApp::stack_photo`). The stack's cover stays the
RAW.

Coming next (same issue and #391): RAW / RAW+JPEG / JPEG filters in a grouped folder,
the folder summary's "RAW+JPEG pairs" line as a filter, a checkbox when offloading, a
mark in the folder panel, the JPEG as a cover on demand, and the full-resolution JPEG as
the camera render.
