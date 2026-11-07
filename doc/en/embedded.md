# Module `embedded`: ratings written into the images

`src/embedded.rs` (issue #276, decided for the beta on 2026-11-07).

`tri`'s ratings (stars, colours, accepted / rejected) live in sidecars (see
[xmp.md](xmp.md), [sidecars.md](sidecars.md)). This module **also writes them into the
images themselves**, to find them outside `tri`: Windows Explorer, Spotlight, Lightroom,
darktable, digiKam… And it reads those another program put there.

## Formats

| Format | Where | Notes |
|---|---|---|
| JPEG | APP1 segment `http://ns.adobe.com/xap/1.0/` | replaced in place, or added after JFIF and EXIF |
| PNG | `iTXt` chunk `XML:com.adobe.xmp` | uncompressed; an existing compressed XMP: refused |
| WebP | `XMP ` chunk | a simple WebP becomes extended (`VP8X`), through `img-parts` |
| TIFF | tag 700 (`XMLPacket`) | in place if it fits (padding spaces), otherwise at the end of the file with a copy of IFD0 |
| HEIC / HEIF | `mime` item `application/rdf+xml`, `cdsc` reference to the primary image | in place if it fits; otherwise its data in an `mdat` added at the end of the file, `meta` rebuilt (`iinf`, `iref`, `iloc`) and the offsets of the data after it shifted accordingly; not sequences (`moov`) |

Never a RAW: its ratings stay in the sidecars. Checked with ExifTool (`-validate`) and macOS:
tile grids like an iPhone's, decoded pixels identical.

## What is written

- The image's **XMP packet**, completed by `xmp::apply` (our fields only, as in a sidecar:
  `xmp:Rating`, `xmp:Label`, accepted / rejected…), never replaced: another program's
  keywords or history stay.
- An **EXIF** `Rating` (0x4746) or `RatingPercent` (0x4749) already there is updated in
  place (two bytes), otherwise Windows Explorer could show the old one. Missing, it is not
  added: rewriting the EXIF risks damaging the camera's data (MakerNotes), and the XMP is
  enough. Rejected: 0 (EXIF has no rejection).
- **Without re-encoding**: the rest of the file is kept byte for byte.
- An image with no rating and nothing written in it yet is not touched.

## Safe writing

The new content is first copied next to it (`.<name>.tri.tmp`), then written **into the
same file** (extended attributes, permissions, Finder tags kept), then read back: if the
ratings read back differ, error. If writing fails half-way, the full copy stays next to
it.

The image changes (size, date): its thumbnails and previews are computed again.

## Reading

`embedded::read` streams the headers only (segments up to the JPEG data, skipped PNG and
WebP chunks, the TIFF's IFD0): fast on a whole folder. The XMP first; otherwise the EXIF
`Rating`.

In [notes.md](notes.md), an image **without a sidecar** takes the ratings written in it
(stars set in Windows Explorer…). Rating it creates its sidecar, with those ratings.
After that the sidecar wins: a rating changed later in the image is read with "Read the
ratings back from the images".

## When

On demand only (decided on 2026-11-07: automatic writing will come after some thought),
two symmetric actions, on the selection or, from the folder menu, the whole folder:

- **"Write the ratings into the images"** (`core.file.embed_notes`, in the background):
  `tri`'s ratings go into the images. `tri <folder> --embed-notes [--dry-run]`,
  `NoteBook::embed`.
- **"Read the ratings back from the images"** (`core.file.import_notes`): the ratings
  written in the images (changed in another program: XnView MP, Windows Explorer…)
  replace those of the sidecars, like a rating set in `tri` (the RAW+JPEG pair follows;
  one history step, undoable). An image with nothing written inside is not touched.
  `tri <folder> --import-notes [--dry-run]`, `NoteBook::import`.

## With other programs

- **XnView MP** (checked on Linux Mint on 2026-11-07): it keeps stars and colours in its
  catalogue, read from the files the first time a folder is opened; after that, ratings
  written by `tri` only show after "Update catalog from files". The other way round, a
  rating changed in XnView only goes into the image with "Metadata › Update file from
  catalog"; `tri` then sees it at once (folder watching) if the image has no sidecar,
  otherwise with "Read the ratings back from the images".

## To check

- Windows Explorer: it reads `xmp:Rating` and the EXIF; a packet that also carries
  `MicrosoftPhoto:Rating` (written by Windows) is not updated yet.
- HEIC files from a real iPhone (tested on files produced by macOS).
