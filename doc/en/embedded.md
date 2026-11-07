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

Never a RAW: its ratings stay in the sidecars. HEIC / HEIF: to come.

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

## When

- **On demand** (default): action "Write the ratings into the images"
  (`core.file.embed_notes`, on the selection, or the folder menu for the whole folder; in
  the background), `tri <folder> --embed-notes [--dry-run]`, `NoteBook::embed`.
- **On each rating**, according to `[xmp] embed` (`embedded::Auto`): `"never"`
  (default), `"internal"` (internal and fixed disks; not a removable disk or a network
  share, `places::outside_roots`), `"everywhere"`. The ratings always go to the sidecar
  too. Undo follows (`NoteBook::write_only`).

## To check

- Windows Explorer: it reads `xmp:Rating` and the EXIF; a packet that also carries
  `MicrosoftPhoto:Rating` (written by Windows) is not updated yet.
- HEIC / HEIF (ISOBMFF container: a `mime` item to add, offsets to shift).
