# Module `tri::reset`: resetting a folder

Issue #411 (decided with the owner on 2026-11-11): one dialog, one checkbox per thing to
clear, among four.

| Part | What | Undo |
|---|---|---|
| Cache | The images' entries (`Cache::clear_folder`), kept with the photos (`.tri/cache`) or in the central cache: works on every folder | no (recomputed) |
| tri's data | The folder's data files ([store.md](store.md): filters and sort, sliders, covers, preferred developments, ratings memory); the "keep tri's data here" switch stays | no |
| Ratings | Stars, colours, flags **reset inside the sidecars, never deleted** (owner's idea): only the fields tri writes change, the rest (darktable history, Lightroom keywords…) is rewritten identically, so it also works on sidecars touched by other software. Every existing source of the photo (XMP, `.rrdata`, `.pp3`, `.arp`, and the image itself when it receives ratings automatically); no sidecar is created (`NoteBook::clear`). A photo in conflict is skipped and reported | yes |
| Versions | The `tri:Versions` list emptied | yes |

Ratings and versions are undone together, in one step (`history::Step::Reset`).

## API

- `Parts { cache, data, notes, versions }`, `Parts::parse("cache,data,ratings,versions")`;
- `folders(folder, depth)`: the folder and its subfolders (hidden ones excepted) on
  `depth` levels (1: the folder alone);
- `survey(cache, data, folders, convention) -> Survey`: what would be cleared (cache size,
  data files, rated photos, photos in conflict, versions), for the dialog (in the
  background: every image's ratings and versions are read);
- `run(cache, data, folders, parts, convention) -> Outcome`: freed space, data files,
  ratings reset and versions removed (`Outcome::step` gives the history step), images
  left alone and why. At best: an image that cannot be written is left, the rest goes on.

## Interface

Right click on a folder, Preferences group: "Reset this folder…" ("this drive…"), action
`core.folders.reset` (no default key). "Clear the cache of this folder…"
(`core.folders.cache_clear`) opens the same dialog with only "Cache" ticked. Defaults:
cache and data ticked, ratings and versions unticked; "Subfolders included" unticked on a
folder, ticked on a drive. The figures are measured in the background; a row at zero is
greyed out, the button too while nothing is left to clear. **Ratings only when "File
actions" is ticked** (owner's remark, 2026-11-11: resetting a whole folder's ratings is
a file action); otherwise the checkbox is greyed out and its tooltip says where to allow
it. The open folder, when inside, is read again at the end.

## Command line

`tri <folder> --reset cache,data,notes,versions [--depth N] [--dry-run]` (`ratings` is
accepted for `notes`): `--dry-run` only counts.
