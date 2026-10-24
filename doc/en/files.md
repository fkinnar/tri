# Module `tri::files` — actions on files

Rename, delete (trash), reveal in the file manager, open in an external editor (first batch
requested on 2026-10-17). All the work is in the engine: the interface (`gui/src/files.rs`)
only handles input and confirmation, and the command line offers the same actions.

## Principle: an image never travels alone

- Its **XMP sidecars** always follow it: without them, its notes would be lost, or later
  attributed to another image with the same name.
- The other image of its **RAW+JPEG pair** (same name apart from the extension, ignoring
  case) follows it too, by default (`[files] pairs = true`), as for notes: you reject a
  photo, not a file. The **Delete** and **Copy / move to…** dialogs have a "with the other
  file of the pair" checkbox when the selection brings in other files (`adds_pairs`,
  2026-10-21: deleting all the JPEGs of a RAW+JPEG shoot took the RAWs with them);
  unticking it updates the list.
- A **shared** sidecar (`IMAGE.xmp`, Adobe convention, common to the RAF and the JPG) is
  only touched if all the images using it are. Without pairs, renaming the RAF alone
  **copies** this sidecar under the new name (both images keep their notes); deleting the
  RAF alone leaves it in place.

## Prepare, then act

Each action is prepared first, to show the list of affected files before acting, and so
that foreseeable errors happen before the first modification.

| Function | Role |
|---|---|
| `adds_pairs(images) -> bool` | do the pairs add files (the other image not chosen)? |
| `group(images, pairs) -> Group` | images (pairs included), sidecars of their own (`sidecars`), shared sidecars left alone (`shared`) |
| `plan_rename(image, new_stem, settings) -> Result<Rename>` | new paths (`moves`) and copies of shared sidecars (`copies`); refuses an empty name, one with `/` or `\`, `.` / `..` (`InvalidName`) or one already taken (`Exists`); changing only the case is allowed |
| `rename(&Rename)` | executes; never overwrites (checked again just before each file); on an error, what was renamed is put back and the copies deleted |
| `plan_delete(images, settings) -> Group` | files to delete |
| `delete(&Group, trash)` | trash ([`trash`](https://crates.io/crates/trash) crate) or permanent deletion |
| `reveal(path)` | file manager, file selected |
| `editor_command(editor, files)` / `open_with(editor, files)` | external editor (command built separately, to be tested without launching anything); programs launched without input or outputs (their traces do not go to the terminal of `tri`) |

Renaming keeps the extension: the name is typed without it. `IMG.RAF`, `IMG.JPG`,
`IMG.RAF.xmp`, `IMG.xmp` become `NOUVEAU.RAF`, `NOUVEAU.JPG`, `NOUVEAU.RAF.xmp`,
`NOUVEAU.xmp`: each name loses its first characters (the old name) in favour of the new
one, which covers both sidecar conventions.

The preview cache is indexed by path: a renamed image is recomputed; the entries of the old
name become orphaned (1:1 previews are size-limited, the rest is cleared with
`tri --clear-cache`).

## Per system

| | Reveal | Trash | Default application |
|---|---|---|---|
| macOS | `open -R` | Finder (via `trash`: "Put Back" works) | `open` |
| Linux | `org.freedesktop.FileManager1.ShowItems` through `gdbus` (Nautilus, Nemo, Dolphin…), otherwise `xdg-open` of the folder | freedesktop specification | `xdg-open` |
| Windows (untested) | `explorer /select,` | system recycle bin | `cmd /C start` |

Launched programs are not waited for; a thread reaps their exit (no zombie process).

## Settings (`[files]`)

| Key | Default | Role |
|---|---|---|
| `pairs` | `true` | the RAW+JPEG pair follows |
| `trash` | `true` | trash; `false`: permanent deletion |
| `editor` | `[]` | program then arguments, `{files}` replaced by the files (appended at the end otherwise); empty: default application |

Editor examples: `["darktable"]`, `["open", "-a", "darktable"]` (macOS),
`["gimp", "{files}"]`.

## Command line

```sh
tri --rename <image> <nom> [--dry-run]
tri --delete <image>... [--permanent] [--dry-run]
tri --reveal <image>
tri --edit <image>...
```

## Tests

Unit (`src/files.rs`): group of a pair with its two sidecars, shared sidecar kept without
pairs, invalid names, editor command (`{files}`, appended at the end, default application),
encoded URI. Integration (`tests/files.rs`), on temporary files: renaming a pair and its
sidecars with the notes read back, taken name refused without changing anything, shared
sidecar copied, case-only change, (permanent) deletion of the pair and its sidecars
without touching the rest. The real trash test is `#[ignore]`: it puts a file in the
user's trash.

## Copy / move to a folder (2026-10-18)

`plan_transfer(images, dossier, mode, réglages)` prepares, `transfer(plan, progression)`
executes. As for renaming, the **pair** (`[files] pairs`) and the **XMP sidecars** follow,
under the same name; a sidecar shared with an image that stays (`IMAGE.xmp` of a pair of
which only one image leaves) is **copied**, never moved: both images keep their notes.

- **Never overwrite**: a name already present in the destination makes the plan fail,
  before any write. A missing destination, or the source folder: refused too.
- **Verified copy**: written under a temporary name (`.NOM.tri-tmp`), read back and
  compared with the original (BLAKE3, as for offloading), **modification date kept** (it
  decides between the two sidecar conventions, see `xmp.md`), then renamed.
- **Move**: a simple rename on the same volume; to another volume (disk, card, NAS:
  `CrossesDevices`), verified copies of **all** the files, then deletion of the originals,
  never before.
- **On an error**, what was done is undone (copies deleted, renames put back).
- Recent destinations: `recent_destinations` / `remember_destination` (`destinations` file
  in the data folder, at most `[files] recent`, vanished ones omitted). Input completion:
  `complete_dir` (subfolders, case ignored, hidden ones only after a ".") and
  `common_prefix`.
- Command line: `tri --copy|--move <image>...|<dossier> --to <dossier> [--dry-run]`.
- Interface: `y` (copy, vim's *yank*) and `Ctrl+X` (move, "cut"; `x` deletes, as in vim),
  on the selection, and right-click buttons; dialog with a field (Tab completes, up / down
  arrows: recent ones, missing folder created if only one level is missing), copy / move
  toggle, progress during the copy (in a thread: the window stays alive).

## Copy / move a folder (2026-10-20)

`plan_transfer_dir(dossier, dest, mode)` prepares the copy or move of a whole folder,
subfolders included (`DirTransfer`: files, subfolders to create, target `dest/nom`);
`transfer_dir(plan, progress)` executes it.

- **Refused**: a folder into itself or one of its subfolders (`IntoItself`), where it
  already is (`SameFolder`), over a folder with the same name (`Exists`).
- **Move on the same volume**: a single rename, instantaneous.
- **Another volume, or a copy**: subfolders created, verified copies of each file (as for
  images); for a move, the originals only go afterwards, and the emptied subfolders with
  them. A file that appeared in the source in the meantime stays there, with its folders.
  On an error, what was written is removed.
- **Undoable** (`history::Step::TransferDir`): a move goes back to its place; a copy is
  only removed as it was left (nothing added or modified since).
- Interface: folders panel, `Ctrl+C` (classic) / `y` (vim) and `Ctrl+X`, or right-click
  "Move to…" / "Copy to…"; the same dialog as for images. **Drag and drop**: a folder, or
  images from the grid (the selection if the dragged image is part of it), dropped on a
  folder of the panel open this dialog, filled in, which always asks for confirmation; copy
  by holding **Option (⌥)** on macOS (as in the Finder; Ctrl + click is the right click
  there), **Ctrl** elsewhere (Nautilus, Explorer). What is being dragged, and whether it is
  copied or moved, is written under the pointer. Places (volume, home folder, root) do not
  move.
- Dialog (user feedback of 2026-10-20): title without a verb ("Folder "Séance" to…", "3
  images to…"), the Move / Copy choice, and **Confirm**: the verb appears only once. Recent
  destinations compared as paths (`~/Images/` completed by Tab and `~/Images` no longer
  make two lines). **To do**: offer to **merge** when the destination already contains a
  folder with the same name (refused for now).

## Trash and undo (2026-10-18)

`delete` returns the trashed files (`Trashed`: original location, and what is needed to find
them again), `untrash` puts them back (see `history.md`). On macOS, moving to the trash goes
through `NSFileManager` (exact location returned) rather than through the `trash` crate,
which does not provide it.

## Folders (2026-10-18)

`create_dir(parent, nom)`, `rename_dir(dossier, nom)` (same parent; the case alone can
change), `dir_contents` (files and subfolders, for the confirmation),
`delete_dir(dossier, corbeille)` (all the content; to the trash, with what is needed to get
it back out). Names checked as for images (`check_name`). Undoable (see `history.md`).
