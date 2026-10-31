# Module `tri::developments`: a photo's developments

Decided on 2026-10-30: everything that **renders** a photo is a *development*, all at the
same level: **darktable**'s main development (its XMP, when darktable wrote it) and its
**clones** (`IMG_01.RAF.xmp` without an `IMG_01.RAF` image), **RapidRAW**'s main one
(`IMG.RAF.rrdata`) and its **virtual copies** (`IMG.RAF.<id>.rrdata`), **RawTherapee**
(`IMG.RAF.pp3`) and **ART** (`IMG.RAF.arp`; 2026-10-31: one main development each, no
clones; paths `IMG@rt.RAF`, `IMG@art.RAF`). The neutral LibRaw
render is the photo itself; the camera JPEG is its initial display, not a development.
"Versions" keep their meaning in `tri`: the crops.

## A development is an image of the grid

A development has no image file: it has a **path of its own**, never on disk, in the
photo's folder and with its extension: `IMG@dt.RAF`, `IMG@dt-01.RAF` (clone 1),
`IMG@rr.RAF`, `IMG@rr-c33b94.RAF`. The rest of `tri` (cache, notes, interface, exports)
treats it as an image:

- the **pixels** are the photo's (`media::source(path)` gives the file to read);
- the **notes**: a clone and a virtual copy have their own, where their software reads
  them (the clone's XMP, the copy's `.rrdata`); a **main development has the photo's**
  (it is the photo in its software), written to each sidecar: the `IMG.RAF.xmp` shared
  with darktable and RapidRAW's `IMG.RAF.rrdata` (`NoteBook::shared`, issue #234 of
  2026-10-31: with the `.rrdata` apart, RapidRAW did not see the notes set on the photo).
  `tri` writes only its own fields and never creates an `IMG_NN.RAF.xmp` itself;
- the **render** is its sidecar's (`Engine::sidecar` resolves it; a RapidRAW copy goes
  through `--adjustments`).

A photo that has developments of a software **no longer has a "developed" render of its
own** for that software (`Engine::sidecar(photo)` returns `None`): its sidecar is the
development's, which has its thumbnail; the photo remains the camera JPEG and the LibRaw
render. A photo without development keeps the previous behaviour (`Shift+B` develops it
on demand, `b` cycles through the renders).

## Functions

- `find(photo, names, images, is_darktable, is_rapidraw)`: a photo's developments from
  the names of its folder's files, in order: darktable main (if its XMP is darktable's:
  `is_darktable` reads its head: a non-empty history, `darktable:history_end` > 0 as an
  attribute or an element; **the only rule**, the same as the renderer and the filter
  since 2026-10-31: an XMP merely imported into darktable or written by `tri` for the
  ratings is not one), clones by number
  (darktable's too: an orphan `IMG_01.RAF.xmp` written by another software, digiKam for
  instance, is not a clone), RapidRAW main (if its `.rrdata` has adjustments:
  `is_rapidraw`, `rrdata::file_is_development`; without, it is a notes sidecar,
  2026-10-31, see `sidecars.md`), copies by identifier. A Lightroom, Capture One or
  digiKam XMP, or the `.rrdata` of a merely rated image, therefore triggers nothing.
- **Registry** (global, filled by `media::scan`): `set_for(photo, developments)` replaces
  a photo's known developments (a deleted clone is no longer known), `register` adds one;
  `get(path)` gives the development of a path of its own, `of(photo)` a photo's in order,
  `has(photo, engine)` whether it has one of that software, `order(path)` a development's
  rank (for sorting).
- `Development::path()`, `own_file()` (the file one can delete without touching the
  photo: a clone, a virtual copy, or the `.rrdata` of the main RapidRAW development; never
  the main darktable XMP, shared with the photo), `notes_in_rrdata()`.
- **Deleting a development** (decided 2026-10-30): `removal()` says what it touches,
  `Removal::File` (the own file, trash) or `Removal::Strip` (main darktable: its XMP
  rewritten **without the history**, `xmp::strip_darktable`, everything darktable writes
  there except `colorlabels`, a note; the notes stay and the photo has no development any
  more). `Rewrite::strip(development)` prepares the rewrite (content before and after,
  nothing written), `apply()` / `revert()` write or undo it, re-reading the file just
  before: changed in the meantime, nothing is written (`false`). On darktable's side, its
  library keeps the history until it re-reads the XMP ("look for updated XMP files on
  startup" option). The whole plan is in `files.md` (`plan_delete`, `delete_plan`), the
  undo in `history.md` (`Step::Developments`).
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
default (cover according to the folder's thumbnail render, 2026-10-31: the photo in
camera JPEG, the preferred or latest development in "Cover", a software's main
development; and "N dev."), unfoldable (`Shift+V`); `Shift+W` / `Shift+Q` step from one
to the next, `Ctrl+Shift+V` folds every stack, a badge on each unfolded thumbnail folds
with the mouse (Alt+click: keeping this development as the cover, 2026-10-31); in full
screen a development shows its render, `b` shows the camera JPEG
instead (before / after); `c` on the photo (2026-10-31) compares it side by side with its
developments (see "Side-by-side comparison").

## Export

A development is exported by **its own software** (`Ctrl+E`, 2026-10-31: RapidRAW copies
through RapidRAW, clones through darktable, whatever is being looked at), under the name
that software gives to its exports (`IMG_01`, `IMG_VC01`); see `export.md`, "We export
what we are looking at" and "Through RapidRAW".

## In the status bar and in the background

The status bar counts the photos and, apart, their developments ("12 images (10 RAW,
3 dev.)", 2026-10-31); development thumbnails are computed in the background whatever
`[gui.grid] develop` says (see `gui.md`).
