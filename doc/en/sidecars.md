# Module `tri::sidecars`: the formats of the notes sidecars

Requested by the user on 2026-10-31: when an image is rated, the rating must go to
**every sidecar known** for it, each software reading its own. Rating in `tri` is then
enough: darktable (XMP) and RapidRAW (`.rrdata`) follow, even on an image never opened in
either. A format is an implementation of the `Format` trait, and the `formats()` list (a
"vector of trait objects"): the XMP ([xmp.md](xmp.md), read by darktable, Lightroom,
Capture One, digiKam…) and RapidRAW's `.rrdata` ([rrdata.md](rrdata.md)). The next
software with a sidecar of its own (RawTherapee, `.pp3`) is added here, with its settings
section.

## The `Format` trait

- `id()`: `"xmp"`, `"rrdata"` (command line); `engine()`: the software reading it, when
  there is only one (the XMP is shared); `owns(sidecar)`: from the name.
- `existing(image, index)`: the photo's sidecar of this format, from `xmp::SidecarIndex`
  (one listing of the folder, no disk access per image: issue #15); `new_path(image,
  convention)`: where to create it when missing.
- `read`, `write` (the rest of the file kept, see each format), `create` (notes only: for
  the `.rrdata`, null adjustments, like RapidRAW for an image rated without being
  developed).
- `of(sidecar)`: a sidecar's format from its name; `by_id(id)`: by format id (`rrdata`) or software id (`rapidraw`).

## When to create a missing sidecar: `[sidecars.<software>]`

An **existing** sidecar always receives the notes. A **missing** one is created according
to `create` in the software's section (`[sidecars.rapidraw]`, `SidecarSettings`):

| `create` | Created when rating |
|---|---|
| `"auto"` (default) | if the software is installed on this machine, or if the folder already has some of its sidecars |
| `"always"` | always |
| `"never"` | never: on demand only (below) |

The XMP is always created (convention `[xmp] sidecar`). `SidecarSettings::creates(installed,
in_folder)` gives the ids to create; the note book takes them through
`NoteBook::set_creates` (interface: software found by `develop::Developers`,
`NoteBook::has_format` for the folder). A user without RapidRAW therefore never sees a
`.rrdata` appear.

## On demand: `NoteBook::generate`, `tri --sidecars`

For a software used only occasionally (`create = "never"`), one generates what it needs
when needed: `NoteBook::generate(images, formats, dry_run)` creates, for the photos that
have notes, the sidecars of the requested formats they do not have yet, with their notes;
developments, images without notes and those already equipped are skipped (`Generated`:
created, unchanged, skipped, errors). Command line: `tri <folder> --sidecars
rapidraw|xmp|all [--dry-run]`. In the interface (2026-10-31): action `core.file.sidecars` (no
default key), a "sidecar" icon in a thumbnail's right-click dialog (the selection) and
"Generate the sidecars…" in the open folder's right-click menu; the dialog
(`SidecarsDialog`) offers "every software" or one format ("RapidRAW (.rrdata)", "XMP"),
announces how many will be created, skipped or are already provided (dry run), then
creates; the summary goes to the status bar, and the "auto" policy is applied again (the
folder now holds that format).

## A `.rrdata` without adjustments is not a development

RapidRAW writes an `IMG.RAF.rrdata` as soon as an image is rated, with null adjustments
or only its `baselineExposure` marker; `tri` creates the same. That file is only a notes
sidecar: `rrdata::is_development` (adjustments other than empty) decides, and
`developments::find` makes an `IMG@rr.RAF` development only of a `.rrdata` that develops
(a virtual copy always is one). No "1 dev." stack for a merely rated photo.

## In the note book (`notes.md`)

A photo has **several sidecars** (`Entry::sidecars`, the most recent first: read; all
written); a development has only its own. `edit` writes the existing ones then creates
the missing ones of the active formats; undo (`write_only`) writes only the existing ones.

## Tests

`sidecars.rs`: format from the name, creation policy from the setting, the installed
software and the folder. `notes.rs`: rating a photo writes its XMP **and** its `.rrdata`;
a missing `.rrdata` is created when the policy says so, not otherwise; `generate` creates
the missing ones of rated photos only, dry run included. `developments.rs` / `media.rs`: a
`.rrdata` without adjustments makes no development.
