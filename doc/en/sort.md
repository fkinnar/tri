# Module `tri::sort`: order of the images

Added on 2026-10-22. The order of a folder's images, to choose from:

| Key (`SortKey`) | Order | Use |
|---|---|---|
| `Name` (default) | file name, that of `media::scan` | the camera's order |
| `Captured` | shooting date (EXIF) | several cameras in a session, numbering restarted at 0001, renamed files |
| `Modified` | file modification date | exports, retouched images |
| `Stars` (2026-10-31) | stars, a rejected one below 0 (like `xmp:Rating = -1`) | the best first (descending) |
| `Color` | colours in the order of `[sort] color_order`; without a colour, or none of the list, last | one colour per sorting pass |
| `Flag` | accepted, no flag, rejected | the accepted first |

`Order { key, descending }`; `sort(files, order, captured, notes, settings)` sorts in
place. `SortKey::by_notes()` says whether the key depends on the ratings;
`SortKey::of_notes` gives an image's value.

- **RAW+JPEG pairs kept together**, the RAW first, in both directions: a pair is ranked by
  its first file.
- **Unknown key last**, in both directions, in name order (shooting date not read yet, or
  missing).
- **Shooting dates**: provided by the caller (`captured(path)`). The interface takes
  those of the similarity fingerprints (`similarity::Signature::time`), already read in
  the background and kept in the analysis cache for the stacks: no extra reading on the
  disk. Until they arrive, the order stays that of the names; the grid is sorted again
  once they are all there (and only if a *new* date arrived: refiltering makes the cache
  announce its own again, a trap already met with the "well exposed" filter, issue #10).
- **Modification dates**: read on the disk when sorting (`modified`).
- **Ratings**: provided by the caller (`notes(path)`, the folder's book); a pair by its
  RAW, a development by its photo (same key as the dates). `[sort] color_order`
  (`SortSettings`) may name only a few colours (user's request, 2026-10-31: red / yellow
  / green ordered for one thing, blue and purple for another); an image is ranked by its
  best-placed colour (several-colours mode), the others go last with the uncoloured, in
  both directions, in name order.

In the interface: list and direction arrow on the right of the filter bar, `f s` (next
key) and `f r` (direction), choice remembered from one launch to the next; first-launch
values `[gui] sort` and `sort_descending`.

Under a sort by rating, the interface sorts again as soon as a rating changes (set,
undone, changed elsewhere: `TriApp::notes_changed`): the image takes its place, the
selection follows it. The stacks form on the displayed order (a burst rated alike stays
contiguous, ties broken by name order); the sharpness judgement stays the engine's, on the
shooting order.
