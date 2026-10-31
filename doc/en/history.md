# Module `tri::history`: undo, redo

Done on 2026-10-18, following the decisions made with the user the same day.

## Principles

- **Session history**: in memory, nothing is kept from one launch to the next (you do
  not undo, days later, a rating that darktable has picked up in the meantime).
- **One action = one step**, even on 30 selected images; the RAW+JPEG pair and the
  sidecars follow, as on the way forward.
- **Never overwrite work done elsewhere**: before undoing (or redoing) a step, we check
  that the files are in the state that step left them in. Otherwise the step is
  **refused with the reason** ("DSCF0305: ratings changed elsewhere since"), and stays
  where it is.
- **Outside the history**: navigation, filters, selection (nothing is modified),
  permanent deletion (already confirmed with a warning), card offload (too big, and it
  can delete from the card).

## Steps (`Step`)

| Step | Recorded | Undo | Checked before |
|---|---|---|---|
| `Notes` | ratings before / after for each image, pair and main developments included (`snapshot` over `NoteBook::shared`, then `notes_step`) | previous ratings, image by image (`NoteBook::write_only`: each image of a pair gets back **its own** ratings) | ratings on disk = ratings after |
| `Rename` | the executed plan | reverse rename; copies of a shared sidecar deleted | new names present, old ones free |
| `Transfer` | the plan, and the fingerprint (size, date) of the written files (`transfer_step`) | copy: copies deleted; move: moved back (same mechanism, verified copies across volumes) | written files present and **unchanged** (a copy edited since is not deleted) |
| `CreateDir` | the created folder | deleted, **only if it is still empty** | folder present and empty |
| `RenameDir` | old and new name | reverse rename | new name present, old one free |
| `Trash` | where each file went (`files::delete` → `Trashed`) | `files::untrash` | original location free, file still in the trash |
| `Developments` | deleted developments (`files::delete_plan` → `Deleted`): sidecars in the trash and darktable XMP rewritten without history (`Rewrite`, content before / after) | XMP put back as before, then `files::untrash` | XMP still as written (rated in darktable since: refused), files still in the trash |

Trash: the `trash` crate can only restore on Linux and Windows
(`os_limited::restore_all`, the item found by its original location). On macOS, `tri`
moves to the trash itself via `NSFileManager.trashItemAtURL` (`objc2-foundation`,
already present through `trash`), which returns the exact location in the trash; undo
takes it back from there (rename, or verified copy if the trash is on another volume).

## `History`

`push` (a new step clears the undone steps), `undo`, `redo`, `go_to(position)` (history
list: undoes or redoes one by one, stops at the first refusal), `entries`, `position`,
step cap (`[gui] history`, 200). `Applied` says what was done: label, images to show,
files moved (folder to reread) or only ratings.

## Interface

`u` / `Ctrl+Z` (and `Cmd+Z` on macOS, the only exception to "Cmd reserved for the
system"): undo; `Ctrl+R` / `Ctrl+Shift+Z` (`Cmd+Shift+Z`): redo; `Ctrl+H`: the list (most
recent at the top, undone steps struck through, clicking a step goes back to it, "State
at launch" undoes everything). After each step: the image concerned is selected, the
folder reread if files moved, and "Undone: 3 stars: DSCF0305.RAF" in the status bar; a
refusal gives its reason there.

Not tested automatically: the real trash (tests never touch it, see `tests.md`); to be
checked by hand.
