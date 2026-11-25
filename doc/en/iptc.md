# Creation metadata (`tri::iptc`, #277)

Author, copyright, usage terms, caption, title, location: the metadata a photographer puts
into their images. `tri` writes them as **XMP** only (decided on 2026-11-25): that is what
Lightroom, darktable, Capture One, Photos, Bridge and recent Windows Explorer read. No
IPTC-IIM (the old binary format, the APP13 segment of JPEGs), only useful to very old
software.

## Sets

One or more **sets** in the configuration, `[iptc.sets.<id>]`, one offered by default
(`[iptc] default`). The `mine` set ("My metadata") comes empty, to be filled in; it is the
model of those created from the settings window ("New…").

| Key | XMP property | Shape |
|---|---|---|
| `creator` | `dc:creator` | list (`rdf:Seq`); several authors separated by ";" |
| `copyright` | `dc:rights` | text per language (`rdf:Alt`, `x-default`) |
| `usage_terms` | `xmpRights:UsageTerms` | text per language |
| `web_statement` | `xmpRights:WebStatement` | text (an address) |
| `credit` | `photoshop:Credit` | text |
| `title` | `dc:title` | text per language |
| `caption` | `dc:description` | text per language |
| `city`, `state`, `country` | `photoshop:City`, `State`, `Country` | text |

With a copyright, `xmpRights:Marked = True` ("copyrighted" in Lightroom and Bridge) is set
if absent. Variables in the values: `{year}` (the year of the file's date: the shooting for
a camera photo, the export for an exported file), `{name}` (the image's name, without
extension).

## Never overwrite by surprise

A field is only set if it is **empty** in the image (absent, or without text), except those
the set names in `replace` (`replace = ["copyright"]`: the copyright is always its own). A
caption written elsewhere is never replaced unless asked for. Everything else in the packet
is copied as it is (checked on a real darktable sidecar: development history identical for
ExifTool).

## Where, when

- **Where**: an image that carries embedded XMP (JPEG, TIFF, PNG, WebP, HEIF) gets it in
  the file, **without recompression**: only the XMP packet changes (`embedded::rewrite`,
  the mechanism of ratings written into images, generalised by `embedded::Change`). A RAW
  gets it in its **sidecar** (the newest, otherwise the one of `[xmp] sidecar`), which
  development software takes up when exporting.
- **When exporting**: the `iptc` key of an export preset (`iptc = "mine"`) puts the set on
  each written file, a copy without recompression included (`Request::iptc`, at the end of
  `export::run`).
- **On demand**: the `core.file.iptc` action (palette, no default shortcut) opens the
  "Creation metadata" dialog on the selection: the set (a list if there are several), what
  it puts ("replaces what is there" for those in `replace`), "Apply" (in the background),
  "Edit the set…" (the settings window on its page). An empty set says so and leads to the
  settings.
- **Command line**: `tri --iptc <set> <image>…|<folder> [--dry-run]`.
- **Metadata panel**: the fields present (embedded XMP, otherwise the newest sidecar) below
  the other rows (`iptc::read`, `Metadata::creation`).

## API

- `xmp::read_property` / `xmp::apply_property`: any XMP property (`Property`, shape
  `Shape::Text`, `Seq` or `Alt`), read as an attribute or an element of a top-level
  `rdf:Description`, written as an element in the first one (the attribute form is
  removed); `apply_list` (versions, `tri`'s adjustments) goes through it.
- `iptc::apply(xml, set, vars)`: the packet with the set; `iptc::stamp(image, set,
  convention, dry_run)`: in the image or its sidecar; `iptc::read(image)`: the fields
  present.

## Tests

`a_set_fills_empty_fields_and_replaces_only_those_listed`,
`a_raw_gets_its_sidecar_and_a_jpeg_its_own_packet`, `the_default_set_is_chosen_or_the_first`
(`src/iptc.rs`), `the_preset_s_creation_metadata_go_into_the_written_files` (export),
`creation_metadata_go_into_the_selected_images` (interface). Checked by hand with ExifTool
(validated JPEG, darktable sidecar).
