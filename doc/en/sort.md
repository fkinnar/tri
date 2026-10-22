# Module `tri::sort`: order of the images

Added on 2026-10-22. The order of a folder's images, to choose from:

| Key (`SortKey`) | Order | Use |
|---|---|---|
| `Name` (default) | file name, that of `media::scan` | the camera's order |
| `Captured` | shooting date (EXIF) | several cameras in a session, numbering restarted at 0001, renamed files |
| `Modified` | file modification date | exports, retouched images |

`Order { key, descending }`; `sort(files, order, captured)` sorts in place.

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

In the interface: list and direction arrow on the right of the filter bar, `f s` (next
key) and `f r` (direction), choice remembered from one launch to the next; first-launch
values `[gui] sort` and `sort_descending`.

Not yet (beta): sorting by rating or colour. The stacks and the sharpness judgement rely
on the shooting order (neighbours, bursts): to be designed with them.
