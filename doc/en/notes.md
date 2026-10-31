# Module `tri::notes`

The culling ratings of a folder **in memory**, how they are changed, and the filters on
these ratings. The sidecar formats are the business of [sidecars.md](sidecars.md) (XMP:
[xmp.md](xmp.md); RapidRAW: [rrdata.md](rrdata.md)): since 2026-10-31 a photo has **all
its sidecars** (read from the most recent, all written, the missing ones created according
to `[sidecars]`); `sidecars(image)` gives them, `sidecar(image)` the first.

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
  `shared(image)` gives everything that receives the same ratings: the pair, and the
  photo with its main darktable and RapidRAW developments (XMP and `.rrdata`, issue
  #234, 2026-10-31; see `developments.md`); the history takes its snapshot from it.
- `reload()`: rereads everything (ratings changed by another program).
- **Background reading** (2026-10-23, issue #15): `NoteBook::pending(images, convention)`
  creates a book without reading anything (empty ratings); `load_counting(&files,
  convention, &counter)` reads everything while counting (in another thread,
  `folder::Loading`); `adopt(read)` takes what was read, **except** the images read or
  written meanwhile (a rating set during the reading, a sidecar changed elsewhere), which
  are fresher; `seed(&old)` keeps a previous book's ratings for the same folder meanwhile
  (folder reread after a rename); `is_pending()`. An image rated before the reading ends
  is read alone, with its pair.
- **Disk accesses** (issues #15 and #29): a sidecar is only opened if it exists
  (`SidecarIndex::existing_sidecar`, no `stat` when there is only one); after a write,
  only the image and its pair are reread (before: the whole folder, on every rating; a
  sidecar is only shared between images of the same name).

Why everything in memory: **filtering must stay instantaneous** (`CLAUDE.md`, "analysis ≠
filtering"); only a modification touches the disk.

Measurement (2026-10-17, Apple Silicon Mac, release, copies of 131 real darktable
sidecars): **7 ms** to load (23 ms cold), i.e. ~0.06 ms per image. On an external hard
drive (2026-10-23, 2,820 DNG and their Lightroom sidecars): **15 s**, ~5 ms per image (one
head seek per file); hence the background reading above.

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
