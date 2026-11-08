# Module `tri::xmp`

A photo's culling notes — **stars** (0–5), **colors**, **accepted / rejected** — read
from and written to its XMP sidecars, while staying readable by darktable, Lightroom,
digiKam and Capture One. Format decisions: `CLAUDE.md`, "Notation XMP".

## Principle: only touch our fields

A sidecar contains much more than notes: darktable development history, keywords,
author, rights… `tri` must not lose any of it. The XMP is therefore read **event by
event** (`quick-xml`) and rewritten **identically**, except for our fields:

- the `rdf:Description` tag carrying our attributes is modified **in its text**: value
  changed in place, attribute removed along with the whitespace before it, new attribute
  added at the end of the tag with the spacing of the last one (one attribute per line
  with exiv2);
- a field written as an element (`<xmp:Rating>3</xmp:Rating>`) is removed and replaced
  by an attribute;
- fields are recognized by their **namespace**, not by their prefix: Bridge writes
  `xap:Rating`, which `tri` understands and updates under the same prefix;
- if nothing changes, the file is not rewritten at all; otherwise, atomic write
  (temporary file then rename);
- a **malformed** sidecar (invalid XML, or without `rdf:Description`) is never
  rewritten: `XmpError::Malformed` / `NoDescription`;
- only a **top-level** `rdf:Description` carries our fields: another one can be nested
  in a property (Lightroom's `Iptc4xmpCore:CreatorContactInfo`, the photographer's
  contact details) and is left as is (issue #27, 2026-10-23: taken for the main one, it
  lost the fields placed after it, and a "saved" version was never written); a list
  (`write_list`) that does not read back is an error, never a silent success.

## The fields

| Note | Field | Values | Read by |
|---|---|---|---|
| Stars | `xmp:Rating` | 0–5 (`-1` = rejected, ISO 16684-1 standard) | all |
| Accepted / rejected | `xmpDM:pick` | 1 / 0 / -1 | Lightroom Classic ≥ 13.2 |
| | `xmpDM:good` | `True` / absent / `False` | Lightroom Classic ≥ 13.2 |
| | `digiKam:PickLabel` | 3 / 0 / 1 (2 = pending, read as "none") | digiKam |
| Colors | `xmp:Label` | `Red`, `Yellow`, `Green`, `Blue`, `Purple` | Lightroom, Capture One, darktable |
| | `darktable:colorlabels` | sequence from 0 (red) to 4 (purple) | darktable |
| | `digiKam:ColorLabel` | 1 red, 3 yellow, 4 green, 5 blue, 6 magenta | digiKam |
| Stars of a rejected photo | `tri:RatingBeforeReject` (`urn:tri:xmp:1.0`) | 1–5 | tri |

Writing rules:

- **rejected**: `xmp:Rating = -1` (what other software sees) and the stars remembered in
  `tri:RatingBeforeReject`: undoing the rejection restores them;
- **no noise**: a field that would be 0 is only added if it already existed (darktable's
  `xmp:Rating="0"` is updated, `xmpDM:pick="0"` is not added everywhere);
- **fields per program** (`[xmp.fields]`, all ticked by default): `label`, `darktable`,
  `digikam`, `lightroom`, read and written; an unticked field is ignored, neither read,
  written nor removed (it stays as it is), and it leaves the `color_sources` order.
  `xmp:Rating` is always handled. Several colors need darktable (its list is the only
  field that holds several): without it, only one.

### Colors: one or several (`[xmp] multiple_colors`)

`xmp:Label` and `digiKam:ColorLabel` hold a single color; `darktable:colorlabels` holds
several. darktable rewrites its list from its database at every change but never touches
`xmp:Label` ("managed by an external application", checked in its source code), and reads
it *before* its list when importing a file. A `Label` set earlier by other software can
therefore lag behind (report of 08/11/2026: `Label="Green"`, list at `0`, red, in
darktable).

- **One color** (default, like Lightroom): written into `xmp:Label`,
  `darktable:colorlabels` (darktable finds its list) and `digiKam:ColorLabel`. Read from
  the order of `[xmp] color_sources` (default: `["darktable", "label", "digikam"]`): the
  first field that holds a color wins (darktable only if it holds a single one, number
  translated). darktable first: its color is the one it shows. A field left out is read
  last; in the settings window, the ticked fields in a column, dragged into place (greyed
  out when only one is left). `[xmp.fields]` is a table there: program, XMP fields, what
  they hold, a "handled" box. Several colors set in
  darktable are all read: the next write does not erase them.
- **Several** (like darktable): only `darktable:colorlabels` is read and written;
  `xmp:Label` and `digiKam:ColorLabel` stay as they are. Without a list (Lightroom,
  XnView… file), `xmp:Label` then digiKam are read. In an image (JPEG…), software that only
  reads `Label` (XnView, Windows Explorer) then does not see the colors.

**Where the list goes**: `darktable:colorlabels` is written just before
`darktable:history`, as darktable itself does. Placed after the history, darktable did not
read the colors back ("keep the XMP" at startup had no effect; seen by a user, although
element order has no meaning in XMP). Without a history, the list goes at the end of the
description.

Reading rules: rejected if `xmp:Rating = -1`, `xmpDM:pick = -1` or
`digiKam:PickLabel = 1`; accepted if `xmpDM:pick = 1` or `digiKam:PickLabel = 3`. Stars
written "3.0" (some software) are accepted. The rules (mode, priority, fields) apply to
the whole process: `xmp::set_rules`, set at start-up and live from the settings window
(the folder is read again).

## Which sidecar

| Convention | Name | Used by |
|---|---|---|
| `Darktable` (default) | `DSCF0092.RAF.xmp` | darktable, digiKam: one sidecar per file |
| `Adobe` | `DSCF0092.xmp` | Lightroom, Capture One, Bridge: shared by the RAF and the JPG of a pair |
| `Both` (`"both"`) | both | moving from darktable to Lightroom or Capture One |

- if sidecars exist, **all are read and all are written**: two XMP sidecars of the same
  photo (`IMG.RAF.xmp` and `IMG.xmp`) are two sources, merged like the others
  ([`reconcile.md`](reconcile.md); never seen together and disagreeing: a conflict to
  settle). Before 08/11/2026, only the most recent was read;
- otherwise, sidecars are created following the `[xmp] sidecar` convention: a Capture One
  user sets `"adobe"` once and for all; `"both"` creates both;
- when rating, an existing sidecar is reused whatever its name (set to Adobe, an existing
  `IMG.RAF.xmp` is enough); **"Generate sidecars"** (`tri --sidecars`, folder menu)
  however creates the ones the convention expects that are missing (set to Adobe, a folder
  of darktable sidecars gets its `IMG.xmp`);
- the search lists the folder and compares names **case-insensitively** (`IMG.XMP` is
  found), with the actual names on disk: on macOS, `IMG.XMP` and `IMG.xmp` are the same
  file, on Linux two distinct files. `SidecarIndex` lists a folder only once for all its
  images.

A new sidecar is a minimal XMP packet (`x:xmptk="tri"`). darktable, on import, reads our
notes and then rewrites the file its own way.

## Converting from one convention to the other

```
tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
```

`xmp::convert(images, target, dry_run)` copies the notes of each image, read from its
most recent sidecar, into the sidecar of the target convention (created if missing; if
it exists, only our fields change):

- **no sidecar is deleted**: darktable's keeps the development history;
- the target sidecar becomes the most recent one, hence the one `tri` reads and writes
  afterwards (if it was already up to date, only its date changes);
- no noise: an image without notes does not get a sidecar;
- towards Adobe, `IMG.RAF` and `IMG.JPG` share `IMG.xmp`: rated differently, that is a
  **conflict**, the first image (the RAW, in `media::scan` order) wins and the other is
  reported; towards darktable, `IMG.xmp` is copied into `IMG.RAF.xmp` and
  `IMG.JPG.xmp`;
- `--dry-run` lists what would be written; running a conversion again changes nothing.

Checked (2026-10-17) on copies of 131 real darktable sidecars: 18 Adobe sidecars created
(the other 113 photos have no notes), notes read back identically by ExifTool, valid
files, darktable sidecars unchanged byte for byte, second conversion without effect.

## API

- `read(image) -> Notes`, `write(image, &notes, convention) -> sidecar path`;
  `read_sidecar(path)`, `write_sidecar(path, &notes)`: a specific sidecar;
- `convert(images, convention, dry_run) -> Conversion` (summary: written, unchanged,
  conflicts, errors);
- `parse(xml) -> Notes`, `apply(xml, &notes) -> xml`: the same work on a text;
- `strip_darktable(xml) -> xml` (2026-10-30): the text without the darktable development
  (attributes and elements of its namespace, except `colorlabels`, a note; the rest byte
  for byte), to "delete the main development" (`developments.md`);
- `SidecarIndex::new(folder)`, `.sidecars(image)`, `.sidecar_path(image, convention)`;
  `existing_sidecars`, `sidecar_path` (a single image);
- `Notes { stars, flag, colors }`, `Flag { None, Accepted, Rejected }`,
  `Color { Red, Yellow, Green, Blue, Purple }`, `Convention { Darktable, Adobe }`.

## Check on real sidecars (2026-10-17)

On **copies** of the 131 darktable sidecars from a real session (including two virtual
copies with ~750 lines of development history):

- rewrite without change: **131 files identical byte for byte**;
- various notes written (stars, accepted, rejected with stars, one and two colors), then
  read back by **ExifTool**, an independent reference: all notes read back identically,
  **all other fields unchanged**, and ExifTool validates the files (`-validate`: OK).

## To check in real conditions

- darktable: **checked by the user on 2026-10-17**, notes set by `tri` make it into
  darktable (with a delay before it takes them into account). It only rereads an XMP
  modified elsewhere if the "look for updated XMP files on startup" option is enabled;
  if it runs during culling, it may rewrite the sidecar from its database and erase our
  notes;
- Lightroom, Capture One, digiKam: to be checked by testers;
- Capture One: behavior with `xmp:Rating = -1` and `xmp:Label` undocumented;
- Lightroom: known bug (13.2) if `xmpDM:pick` is missing from the file.
