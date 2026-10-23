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
- **a single color**: `xmp:Label`, `digiKam:ColorLabel` and `darktable:colorlabels`;
  **several**: only `darktable:colorlabels`. darktable reads `xmp:Label` *before* its own
  colors (checked in its source code): writing it with several colors would make it lose
  some.

Reading rules: rejected if `xmp:Rating = -1`, `xmpDM:pick = -1` or
`digiKam:PickLabel = 1`; accepted if `xmpDM:pick = 1` or `digiKam:PickLabel = 3`.
Colors: `xmp:Label` first (like darktable), otherwise `darktable:colorlabels`, otherwise
`digiKam:ColorLabel`. Stars written "3.0" (some software) are accepted.

## Which sidecar

| Convention | Name | Used by |
|---|---|---|
| `Darktable` (default) | `DSCF0092.RAF.xmp` | darktable, digiKam: one sidecar per file |
| `Adobe` | `DSCF0092.xmp` | Lightroom, Capture One, Bridge: shared by the RAF and the JPG of a pair |

- if sidecars exist, the **most recent one is read and written** (modification date);
- otherwise, one is created following the `[xmp] sidecar` convention: a Capture One user
  sets `"adobe"` once and for all;
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
