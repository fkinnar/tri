# Module `tri::folder`

An **open folder**: what an interface (graphical, TUI) or the command line handles when
one "opens a folder".

- `Folder::open(dir, depth, convention)`: path made absolute (without resolving links:
  `std::path::absolute`, no `\\?\` prefix on Windows), images ([media.md](media.md)),
  ratings ([notes.md](notes.md));
- `Folder::filtered(type, rule)`: filter by type then filter on ratings, combined here
  once and for all (previously: in the interface);
- `Folder::default()` / `is_none()`: no folder open (interface launched without a folder).

- `remember(data, folder)` / `last(data)`: last opened folder, remembered in the
  `last-folder` file of the data folder (`EngineSettings::data_dir`: `[data] dir`,
  otherwise the system one), read back only if it still exists; path as raw bytes on
  Unix (non-UTF-8 names), UTF-8 elsewhere. The interface reopens this folder when it is
  launched without an argument (`[gui] reopen_last`).

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
