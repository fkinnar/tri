# Module `tri::pp3`: ratings in RawTherapee and ART profiles

Added on 2026-10-31, after [rrdata.md](rrdata.md): RawTherapee keeps its edits and its
ratings in a **profile** `IMAGE.RAF.pp3` next to the image; ART, its fork, does the same
in `IMAGE.RAF.arp`, in the same format. One module serves both; only the extension differs.

## The format

An INI-like file: `[Name]` sections, `Key=Value` lines, sometimes comments (`#`, `;`). The
ratings live in `[General]`:

| Key | Value | In `tri` |
|---|---|---|
| `Rank` | 0 to 5 | the stars |
| `ColorLabel` | 0 none, 1 red, 2 yellow, 3 green, 4 blue, 5 purple | the colour (one only: `tri`'s first) |
| `InTrash` | `true` / `false` | `true` = rejected (the software's trash) |

No equivalent for "accepted": that flag stays in the XMP. As everywhere, **nothing is
lost**: only these three keys change (replaced in place, added when missing, the section
created at the top when missing); everything else, settings, line order, comments and line
endings (`\r\n` on Windows), is written back as is.

## Functions

- `parse(text)` / `apply(text, notes)`: reading and rewriting the content;
- `path(image, extension)`: `IMG.RAF.pp3` or `IMG.RAF.arp`;
- `is_development(text)` / `file_is_development(path)`: true when a section other than
  `[Version]` and `[General]` exists. RawTherapee writes a profile reduced to those when an
  image is rated in its browser without being opened; so does `tri` (`create`). Such a
  profile is **not a development**: no `IMG@rt.RAF` image in the stack, nothing to render
  (`develop::profile_sidecar`), like a ratings-only `.rrdata`;
- `development_content(text)`: the profile without `[General]`, for the render cache key
  (rating does not redo the render, `develop::render_stamp`);
- `create`, `read`, `write`: the file, re-read just before writing, not rewritten when
  nothing changes.

## Where it is used

- [sidecars.md](sidecars.md): the `PP3` and `ARP` formats (`sidecars::Profile`, ids `pp3`
  / `arp`, software `rawtherapee` / `art`), created when rating according to
  `[sidecars.rawtherapee]` / `[sidecars.art]`, or on demand;
- [developments.md](developments.md): `developments::find` recognises `IMG.RAF.pp3` and
  `IMG.RAF.arp` as the main development (no clones in these programs);
- [develop.md](develop.md): the render by `rawtherapee-cli` / `ART-cli`.

Checked against the docs and man pages (RawPedia, `ART.1`); to be confirmed on real
profiles from both programs (testers).
