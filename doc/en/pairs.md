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

## Filters

In a grouped folder, the RAW and JPEG checkboxes of the right panel become **three**:
RAW (RAW files without a JPEG), RAW+JPEG (grouped pairs), JPEG (JPEGs without a RAW).
Combinable, at least one checked; the file-type key (`core.filter.cycle`) steps from all
to each one alone. A development follows its photo; kept with the folder's filters.

## Engine

- `Grouping`: the setting's three values.
- `decided(data, folder)` / `decide(data, folder, Some(bool) | None)`: the folder's
  decision, kept in its values (`tuning.json`, key `pairs_grouped`).
- `grouped(setting, data, folder)`: the decision, else the setting.
- `PairKinds`, `PairFilter`: the three checkboxes and their filter (`Filter` trait).
- `camera_jpegs(files)`: the flat images that have a RAW of the same name (ignoring
  case) in the same folder, with that RAW. Also used by the folder summary
  ([folder.md](folder.md)).

## Interface

Development stacks gather the images that have the same photo: for the JPEG of a
grouped pair, that photo is its RAW (`TriApp::stack_photo`). The stack's thumbnail follows
the shown render: with **camera JPEG**, it is the **JPEG file itself**, at its true
resolution (full screen and 1:1 included, with its own histogram and metadata), rather
than the RAW's embedded preview; with LibRaw or a software, the RAW or its development;
with "Cover", the chosen cover (bookmark of the "Shown render" list), else `[gui.grid]
cover_default` (#391): `latest` (the latest development, else LibRaw; default), `camera`
(the camera JPEG) or `libraw`.

- **Folder summary**: in a grouped folder, the "RAW+JPEG pairs" line is a filter (the
  RAW+JPEG checkbox alone); separate, its tooltip says how to group them.
- **Offloading**: a "Group RAW and JPEG (n pairs)" checkbox, shown only when what will be
  copied has pairs (so not with "RAW only"), checked by default with `group_pairs =
  "grouped"`; its answer is kept for the session folder once copied (`Plan::pairs`). With
  "ask", an untouched checkbox leaves the question for the opening.
- **Folder panel**: a mark (two offset sheets) after the name of a grouped folder, read
  once per folder.

Closed with this: #391 and #392.
