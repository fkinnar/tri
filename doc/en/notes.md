# Module `tri::notes`

The culling ratings of a folder **in memory**, how they are changed, and the filters on
these ratings. The sidecar format is the business of [xmp.md](xmp.md).

## `NoteBook`: the ratings of a folder

- `NoteBook::load(images, convention)` / `load_files(&files, convention)`: reads the
  ratings of all images. Each folder is listed only **once** (`xmp::SidecarIndex`), then
  only the existing sidecars are read.
- `notes(image)`: ratings in memory (empty for an image without a sidecar or an unknown
  one); `error(image)`: unreadable or malformed sidecar; `sidecar(image)`: the sidecar
  read, or the one that will be created.
- `edit(image, |notes| …)`: modifies, **writes**, then returns the images whose ratings
  changed. In the Adobe convention, `IMG.RAF` and `IMG.JPG` share `IMG.xmp`: rating one
  changes the other, and both are returned to be refreshed on screen. After writing, the
  folder is reread (the sidecar may have been created). On error, memory does not change:
  the display stays faithful to the disk. A malformed sidecar is never modified.
- **Pairs** (choice of 2026-10-17): `siblings(image)` gives the images with the same name
  apart from the extension, case-insensitively (`IMG.JPG` for `IMG.RAF`, but not
  `IMG_01.JPG`); `edit` gives them the same ratings, whatever the convention (darktable
  convention: `IMG.RAF.xmp` and `IMG.JPG.xmp`; Adobe: a single `IMG.xmp`, written once).
  A pair that already differs is realigned. A malformed sidecar in the pair is reported
  (`XmpError::Image`, which names the image) without blocking the other writes.
- `reload()`: rereads everything (ratings changed by another program).

Why everything in memory: **filtering must stay instantaneous** (`CLAUDE.md`, "analysis ≠
filtering"); only a modification touches the disk.

Measurement (2026-10-17, Apple Silicon Mac, release, copies of 131 real darktable
sidecars): **7 ms** to load (23 ms cold), i.e. ~0.06 ms per image. Synchronous when a
folder is opened for now; to be moved to the background if a slow SD card calls for it.

Operations on `xmp::Notes`: `set_stars(n)` (the flag does not change: a rejected photo
keeps its stars), `toggle_color(c, multiple)` (single colour: another one replaces it,
the same one removes it; multiple: add / remove, as in darktable), `clear_colors()`.

`reload_sidecars(&sidecars)` rereads only the images concerned by sidecars modified
elsewhere (folder watching, `doc/watch.md`) and returns those whose ratings changed.

## Filters

`NotesRule` groups the criteria, combined with "and"; `None` = criterion ignored:

- `stars: Option<StarsRule>`: `AtLeast(n)`, `Exactly(n)` (`Exactly(0)`: unrated),
  `AtMost(n)`;
- `flags: Option<Vec<Flag>>`: accepted flags (`[None, Accepted]` hides rejected ones);
- `colors: Option<ColorRule>`: one of the `any_of` colours, and/or no colour (`none`).

`rule.filter(&book)` gives a `NotesFilter`, which implements the same
[`Filter`](filter.md) trait as the filter by type: no special code for native filters,
as `CLAUDE.md` intends.
