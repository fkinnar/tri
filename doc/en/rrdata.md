# Module `tri::rrdata` — notes in RapidRAW sidecars

RapidRAW keeps an image's settings and notes in a JSON file next to it: `IMAGE.RAF.rrdata`
for the main development, `IMAGE.RAF.<id>.rrdata` for each virtual copy (internally,
RapidRAW calls it `IMAGE.RAF?vc=<id>`). Checked in its source code on 2026-10-30.

Each development keeps its notes where its software reads them (decided on 2026-10-30):
a virtual copy has its own in its `.rrdata`; the main development has the photo's,
written to `IMAGE.RAF.rrdata` as well (issue #234, 2026-10-31; see `developments.md`).

## Fields

| Field | Content | In `tri` |
|---|---|---|
| `rating` | 0 to 5 | stars |
| `flag` | `"pick"`, `"reject"`, absent otherwise | accepted, rejected, no flag |
| `tags` | list; the colour is the tag `"color:red"` (only one: `red`, `yellow`, `green`, `blue`, `purple`) | colour (the first one, if several) |
| `adjustments` | the settings | render of a virtual copy (`rapidraw export --adjustments`) |

## Never lose anything

As for XMP files: only `rating`, `flag` and the colour tag change; the rest (settings,
keywords, unknown fields) is written back as is, in the same order (the `preserve_order`
feature of `serde_json`). The file is read again just before writing, and is not written
if the notes did not change.

## Functions

- `parse(json)` / `read(path)`: the notes.
- `apply(json, notes)` / `write(path, notes)`: the notes written, the rest kept.
- `adjustments(json)`: the settings, to render a virtual copy.

## Files

Virtual copies follow the copied, moved or renamed image, like the main `.rrdata`
(`xmp::SidecarIndex::rapidraw_copies`, see `files.md`).
