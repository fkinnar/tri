# Module `tri::watch`: watching the open folder

Requested for V1 (2026-10-18, "a real comfort"): images copied into the open folder
(export, offload by another tool, tethered shooting), removed or modified, and ratings
set elsewhere (darktable…) show up by themselves, without F5.

## Polling rather than notifications

`FolderWatch` rereads the folder listing (images and XMP sidecars, with size and
modification date; everything else is ignored) every `[watch] interval_ms` (1000 ms by
default, 0: no watching), in a separate thread. A deliberate choice over the `notify`
crate (FSEvents, inotify…): no dependency, and the same behaviour on a local disk, an SD
card or a network share, where system notifications are often missing. Same principle as
volume watching (`places::Monitor`). Cost: one folder read per second (a few milliseconds
for a few thousand local files). On a slow share, one read can take minutes (GVFS on
Linux, one network round trip per file): the next one then waits accordingly, so that no
more than `[watch] max_share` % of the time is spent reading (10 %: a 3 s read waits
27 s; `Pace::wait_after`, 2026-10-29).

Folder opened with its subfolders (panel checkbox, 2026-10-20):
`FolderWatch::start_depth` / `Snapshot::of_depth` look at the same levels as the grid,
hidden folders excluded as in `media::scan`.

## One signal per change, once the folder is stable

A change is only signalled when two successive reads are identical: a copy of 500 files
in progress, or a file whose size is growing, gives only one signal, at the end. The
signal (`Changes`) lists the images and sidecars added, removed or modified since the
previous signal.

`FolderWatch::sync()` takes the current state as reference without signalling anything:
after a change made by `tri` itself (rename, trash, folder reread), already taken into
account.

## In the interface

- changed images: the folder is reread, the selection stays on the same image; message
  "folder reloaded" in the status bar;
- sidecars only: `NoteBook::reload_sidecars` rereads the ratings of only the images
  concerned (`IMG.RAF.xmp` → `IMG.RAF`, `IMG.xmp` → `IMG.RAF` and `IMG.JPG`) and returns
  those that really changed: nothing when `tri` has just written them. The ratings filter
  and the stack covers are then updated.

Watching restarts with each folder opened, or if the interval changes in the settings.
Its starting state is taken with the ratings, in the background (`folder::Loading`), then
given to `FolderWatch::start_from`: taken when the watch started (`start_depth`), it was
one disk access per file in the interface thread (issue #15).
