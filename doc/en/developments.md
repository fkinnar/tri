# Module `tri::developments`: a photo's developments

Decided on 2026-10-30: everything that **renders** a photo is a *development*, all at the
same level: **darktable**'s main development (its XMP, when darktable wrote it) and its
**clones** (`IMG_01.RAF.xmp` without an `IMG_01.RAF` image), **RapidRAW**'s main one
(`IMG.RAF.rrdata`) and its **virtual copies** (`IMG.RAF.<id>.rrdata`). The neutral LibRaw
render is the photo itself; the camera JPEG is its initial display, not a development.
"Versions" keep their meaning in `tri`: the crops.

## A development is an image of the grid

A development has no image file: it has a **path of its own**, never on disk, in the
photo's folder and with its extension: `IMG@dt.RAF`, `IMG@dt-01.RAF` (clone 1),
`IMG@rr.RAF`, `IMG@rr-c33b94.RAF`. The rest of `tri` (cache, notes, interface, exports)
treats it as an image:

- the **pixels** are the photo's (`media::source(path)` gives the file to read);
- the **notes** are its own, where its software reads them: the clone's XMP, the copy's
  `.rrdata`; the photo and darktable's main development share `IMG.RAF.xmp` (`NoteBook`,
  `shared_with`). `tri` writes only its own fields and never creates an `IMG_NN.RAF.xmp`
  itself;
- the **render** is its sidecar's (`Engine::sidecar` resolves it; a RapidRAW copy goes
  through `--adjustments`).

A photo that has developments of a software **no longer has a "developed" render of its
own** for that software (`Engine::sidecar(photo)` returns `None`): its sidecar is the
development's, which has its thumbnail; the photo remains the camera JPEG and the LibRaw
render. A photo without development keeps the previous behaviour (`Shift+B` develops it
on demand, `b` cycles through the renders).

## Functions

- `find(photo, names, images, is_darktable)`: a photo's developments from the names of
  its folder's files, in order: darktable main (if its XMP is darktable's: `is_darktable`
  reads its head, `darktable:xmp_version` or a history), clones by number, RapidRAW main,
  copies by identifier.
- **Registry** (global, filled by `media::scan`): `set_for(photo, developments)` replaces
  a photo's known developments (a deleted clone is no longer known), `register` adds one;
  `get(path)` gives the development of a path of its own, `of(photo)` a photo's in order,
  `has(photo, engine)` whether it has one of that software, `order(path)` a development's
  rank (for sorting).
- `Development::path()`, `own_file()` (the sidecar one can delete without touching the
  photo: a clone or copy, never the main one), `notes_in_rrdata()`.
- `Development::rank(siblings)`: its place for the label, `Rank::Main`, `Rank::Clone(n)`
  (the file's number) or `Rank::Copy(n)` (the virtual copy's rank among the photo's, the
  first = 1: the identifier says nothing).
- `latest(developments)`: the most recent by its sidecar's date.
- `preferred(data)`, `prefer(data, development, on)`: a photo's preferred development
  (cover of its stack), one per photo, in `preferred-developments.txt` of the data folder.

## Order in the list

`media::compare` puts a development **with its photo**, right after it and before the
other file of the pair: `IMG.RAF`, `IMG@dt.RAF`, `IMG@rr.RAF`, `IMG.JPG`, in the registry's
order (not the order of the paths of their own: `IMG@dt-01` would come before `IMG@dt`,
`IMG@dt` after `IMG2`). Sorting by date (`sort`) gives a development its photo's key.

## In the interface

See `gui.md`, "Development stacks": a photo is a stack of its developments, folded by
default (cover and "N dev."), unfoldable (`Shift+V`); `Shift+W` / `Shift+Q` step from one
to the next; in full screen a development shows its render, `b` shows the camera JPEG
instead (before / after).

## Next

Double delete (the shown development, the photo, the stack; `[files] allow_actions`),
export of RapidRAW developments, comparing a photo's developments.
