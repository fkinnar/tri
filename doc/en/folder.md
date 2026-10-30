# Module `tri::folder`

An **open folder**: what an interface (graphical, TUI) or the command line handles when
one "opens a folder".

- `Folder::open(dir, depth, convention)`: path made absolute (without resolving links:
  `std::path::absolute`, no `\\?\` prefix on Windows), images ([media.md](media.md)),
  ratings ([notes.md](notes.md));
- `Folder::scan(dir, depth, convention)` (2026-10-23, issue #15): like `open`, without
  reading the ratings, which stay empty meanwhile (`NoteBook::pending`). Listing a folder
  is fast; reading its sidecars is not: 2,820 images on an external hard drive, 15 s of
  empty window. The interface thus opens every folder, then starts
  `Loading::start(&folder, depth, watch)`: a `tri-folder` thread reads the ratings
  (`NoteBook::load_counting`, progress `progress()` → (read, total)) and, if asked, the
  folder's state for its watch (`watch::Snapshot`, one disk access per file, best done
  there). `try_take()` returns `Loaded { notes, snapshot }` once done; the interface takes
  the ratings with `NoteBook::adopt`;
- `Folder::scan_counting(dir, depth, convention, stop, found)` (2026-10-29): `scan`,
  interruptible, counting the images found (`media::scan_counting`). On a NAS, even the
  list can take time: the interface reads it in a thread, waits `[gui.folders]
  open_wait_ms` for it (150 ms: nothing changes on a local disk), then lets it finish in
  the background with a box in the middle ("Opening "shoot"… 340 images found", Cancel or
  Escape); the previous folder stays on screen meanwhile. The launch folder (argument, or
  the last one) takes the same path (2026-10-30: on a NAS, the window waited several
  seconds for the list): the window opens at once, the tree unfolds to the folder once it
  is read, and a last folder that cannot be found is shown in the middle (reopened when
  it comes back);
- `Folder::filtered(type, rule)`: filter by type then filter on ratings, combined here
  once and for all (previously: in the interface);
- `Folder::default()` / `is_none()`: no folder open (interface launched without a folder).

- `remember(data, folder)` / `last(data)`: last opened folder, remembered in the
  `last-folder` file of the data folder (`EngineSettings::data_dir`: `[data] dir`,
  otherwise the system one), read back only if it still exists; path as raw bytes on
  Unix (non-UTF-8 names), UTF-8 elsewhere. The interface reopens this folder when it is
  launched without an argument (`[gui] reopen_last`). `remembered(data)` returns it even
  if it is gone (drive unplugged: the interface says so, issue #66).

- `subfolders(data)` / `remember_subfolders(data, yes)`: the "Subfolders" checkbox of
  the folders panel (2026-10-20), kept from one session to the next (`subfolders` file);
  the interface then opens the folder over `1 + [gui.folders] subfolders_depth` levels
  (`Folder::open`, `media::scan`: hidden folders excluded).
- `folder_value` / `remember_folder_value`: a value remembered per folder (stack
  threshold, blur severity), one line "value, tab, path" in a file of the data folder.

Public fields (`dir`, `files`, `notes`): an open folder has no invariant to protect
between them, and the interface modifies the ratings (`folder.notes.edit`).

Why a separate module: the engine must do the real work and stay independent of the
interface (TUI, commands such as "offload a card" without an interface, to come).
