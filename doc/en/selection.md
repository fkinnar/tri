# Module `tri::selection` — multiple selection

The **marked** images of a list, in addition to the current image (the cursor). Requested
on 2026-10-17, with the keyboard **and** the mouse, with one requirement: everything must
be doable with the keyboard. The module does not depend on the display (graphical
interface, future TUI); the interface only translates keys and clicks into calls.

## Model

- A set of marked paths (`Selection`), independent of indices: the selection survives a
  filter change and a reread of the folder (`retain_existing` forgets deleted files,
  `renamed` follows a rename).
- A vim-like **visual mode** (`start_visual` / `update_visual` / `end_visual`): the
  range goes from the anchor to the current image and follows every move; confirming
  keeps the range, cancelling restores the previous marks.
- `targets(list, cursor)`: the images to act on, in list order — the marked ones **still
  visible** (we do not act on what a filter hides), otherwise the current image. All
  actions go through it: no special "single image" case.

| Function | Role |
|---|---|
| `toggle(path)` | mark / unmark |
| `add_range(list, from, to)` | mark a range (Shift+click) |
| `select_all(list)` / `clear()` | all / none |
| `start_visual`, `update_visual`, `end_visual(keep)` | visual mode |
| `targets(list, cursor)`, `visible(list)` | action targets, visible count |

## In the interface

| Key / gesture | Action |
|---|---|
| `Space` | mark / unmark the current image (`core.select.toggle`) |
| `v` | visual mode; `v` confirms, Esc cancels (`core.select.visual`) |
| `Ctrl+A` / `Ctrl+Shift+A` | all / none (`core.select.all`, `core.select.clear`) |
| Esc | cancels visual mode, otherwise deselects, otherwise returns to the grid |
| Shift+click | range from the current image |
| Cmd+click (macOS), Ctrl+click | add / remove an image |
| simple click | selection cleared, current image chosen |

**Ratings** (keyboard, rating bar) apply to the targets, without auto-advance. A RAW+JPEG
pair is rated only once (rating one image rates the other: a colour would toggle twice);
a **colour** has the same effect on all of them, decided from the first one (added if it
does not have it, removed otherwise), instead of an image-by-image toggle that would mix
things up. **Trash** and **external editor** take the targets; rename and show in the
Finder, the current image.

Display: orange frame (`[gui.colors] marked`, `[gui.grid] marked_frame`) around marked
thumbnails, distinct from the current image (solid background) and from hover (grey);
"N selected" (and "visual mode") in the status bar; "selected" in the full-screen info
line; the rating bar says how many images its ratings apply to.

## Tests

Unit (`src/selection.rs`): target = cursor without marks; marks in list order, hidden
image excluded; visual mode in both directions, cancelled then confirmed; bounded range,
all, rename. Interface (`gui/src/app.rs`): Space on two images, rating and colour applied
to both and not to the cursor, colour removed from all, Esc deselects; visual mode
cancelled then confirmed, trashing both images ("2 files"); Ctrl+A / Ctrl+Shift+A;
Shift+click, Cmd+click, simple click.
