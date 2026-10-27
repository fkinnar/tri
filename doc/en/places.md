# Module `tri::places`

Places offered for choosing a folder, and the folder tree, **without an interface**: the
graphical interface only draws the rows of [`Tree::rows`]; a TUI or another display can
do the same.

## Places (`places()`)

| Kind | macOS | Linux and other Unix | Windows |
|---|---|---|---|
| `Volume` | `/Volumes/*` | `/media/<USER>/*`, `/run/media/<USER>/*`, `/mnt/*` | drive letters `C:\`… |
| `Home` | `$HOME` | `$HOME` | `%USERPROFILE%` |
| `Root` | `/` | `/` | — |

Why not just the home folder: an SD card is mounted in `/Volumes` (macOS) or
`/media/<user>` (Linux), not in `~` (user feedback, 2026-10-17). The root gives the full
tree. Symbolic links in `/Volumes` are skipped (`Macintosh HD` points to `/`). The OS and
the environment are injected into the internal functions (`volume_dirs`, `home_dir`) so
they can be tested on every platform, like `settings::default_path`.

## Hidden places (`PlaceSettings`, section `[places]`)

- `hide_backups` (default: true): **Time Machine** backup volumes are hidden, recognized
  by their **content** and not by their name (which everyone picks):
  `backup_manifest.plist` at the root (APFS disk), `Backups.backupdb` (HFS disk, local
  snapshots volume `com.apple.TimeMachine.localsnapshots`). Checked on the development
  machine (2026-10-17): "TimeMachine" and the snapshots disappear, "Backup" (an ordinary
  disk) and "Work" remain;
- `exclude`: patterns matched against the **name** (`"Backup"`) or the **path**
  (`"/mnt/*"`), case-insensitively, with `*` and `?` (small `wildcard` function, no
  dependency); applies to every place (the root can be hidden);
- `places_filtered(&filter)`; `Tree::new(filter, hidden)` keeps the filter for
  `refresh`.

## Volume monitoring (`Monitor`)

A plugged-in disk or card shows up by itself (requested on 2026-10-17):
`Monitor::start(settings, on_change)` rereads the list of places every
`[places] watch_ms` (2000 ms; 0: no monitoring) in a separate thread, and calls
`on_change` only when it changes. The interface responds with `Tree::set_places` (what
was expanded stays expanded).

- **Why reread rather than listen to the system**: mount notifications differ on macOS,
  Linux and Windows and require a dependency; rereading `/Volumes` costs a few system
  calls, for at most two seconds of delay.
- **Why a separate thread**: a network volume that stops responding can block a read for
  several seconds; then it is the thread that waits, not the interface.
- The initial list is read **before** starting the thread: a volume plugged in right
  after the call is reported correctly (race found by the test).
- Stopped on destruction (`Drop`): the thread is woken up (`unpark`) and joined, without
  waiting for the end of the interval.
- `Monitor::with_source(interval, source, on_change)` monitors any list (tests: a
  temporary folder where a volume is "plugged in").
- Checked on macOS (2026-10-17) with a disk image mounted by `hdiutil attach`: appears in
  under 2 s, disappears on `detach`.

## Unmounted media (`unmounted_devices`, `mount`, #113)

A system that no longer mounts anything by itself (feedback of 2026-10-26, Linux Mint:
the card only showed under "Devices" in the file manager): on Linux, `places()` adds
the media plugged in but not mounted, read with `lsblk -J -b -l` (`parse_lsblk`):
partitions, or disks without partitions, with a file system (no swap, LUKS, LVM…), on
removable media (`RM`) or hot-plugged (`HOTPLUG`: USB card readers often say they are
fixed). Older `lsblk` versions write booleans and numbers as text, read too. They become
`PlaceKind::Device` places whose path is the device's (`/dev/sdb1`), named by their
volume, otherwise by their size ("31.9 GB media"). macOS and Windows mount everything by
themselves: nothing to add.

`mount(device)` mounts as the file manager does, without a password for removable
media: `udisksctl mount -b` (Linux; folder read from its answer, "Mounted /dev/sdb1 at
/media/me/X-T4", otherwise from `/proc/mounts`), `diskutil mount` (macOS). `eject` of an
unmounted medium powers it off (`udisksctl power-off`). `[places] auto_mount` (on by
default, user's choice on 2026-10-26): the `Monitor` mounts by itself a medium **that arrives** (one unmounted on
purpose afterwards stays unmounted).

## Ejection (`eject(volume)`)

With the system tool (requested on 2026-10-17): `diskutil eject` (macOS); on Linux,
`gio mount -e` (the desktop environment's tool, like the file manager), otherwise
`udisksctl unmount` then `power-off` on the device read from `/proc/mounts`
(`device_of`, spaces written `\040`); Windows (issue #192: the Explorer "Eject" verb run
through PowerShell failed silently): the system API, Microsoft's method (KB 165721) —
open `\\.\E:`, lock it (refused while a file is open on it, retried for `[places]
eject_lock_ms`, 5 s, then "volume in use" with the reason), dismount it, allow removal,
eject the medium; a disk whose medium does not eject (USB hard drive) then goes through
the "Eject" verb (**to be checked on Windows**). Blocking (from 0.4 s to a few seconds): the interface calls it in a
separate thread. On refusal (internal disk, open file), the system's message is returned
as is. A "fixed" USB disk (external hard drive) ejects like a card, as in the Finder.
Checked on macOS with a disk image: ejected in 0.36 s; missing volume:
"diskutil : Failed to find disk".

The place rows of `Tree::rows` carry their kind (`Row::kind`) and `Row::ejectable`: only
non-**fixed** volumes offer ejection. `[places] fixed` (same patterns as `exclude`)
designates external disks that are always plugged in (work, backup);
`PlaceSettings::ejectable`, and `Tree::can_eject` to check before `eject` (the interface
does so when confirming).

## Personal and network folders (2026-10-20)

- **Usual folders**, like the Finder sidebar: Desktop, Documents, Downloads, Pictures,
  after the home folder (`user_dirs`). On Linux, those from `~/.config/user-dirs.dirs`
  (`xdg-user-dirs`: "Bureau", "Images" on Mint in French, absolute path allowed);
  otherwise the English names. Missing ones: omitted.
- **Network shares** (`PlaceKind::Network`): mount points of a network file system (SMB,
  AFP, NFS, WebDAV, SSHFS, rclone…), read from the output of `mount` (macOS) or
  `/proc/mounts` (Linux); a share mounted in `/Volumes` or `/media` changes section, the
  others (`/etc/fstab`, `~/nas`) are added. On Linux, also the file manager's **GVFS**
  mounts (`/run/user/<uid>/gvfs`: `smb-share:server=nas,share=photo` becomes
  "photo (nas)"); a phone (`mtp`) or a camera (`gphoto2`) there is a volume. No ejection
  offered for a share; nothing on Windows (network drives: drive letters).

## Subfolders (`subfolders(dir, hidden)`)

Sorted by name, case-insensitively; hidden folders (`.xxx`) only if `hidden`; links to
folders followed; error if `dir` is unreadable.

## Tree (`Tree`)

- nothing is walked ahead of time: a folder is read when it is **expanded** (`expand`,
  `toggle`), then kept; instant even on a NAS;
- `rows()`: visible rows, flattened (`Row { path, name, depth, expanded, error, kind,
  ejectable, favorite, section }`), in display order; an unreadable expanded folder
  carries its error;
- **sections** (requested on 2026-10-20: sorted, and going through the root as little as
  possible): `Section::Devices` (removable media, mounted or not, #113), `Drives` (fixed
  drives, `[places] fixed`; `PlaceSettings::section`), `Network` (network shares), `Personal`
  (home folder and its usual folders), `Favorites`, and only then `System` (the root,
  collapsed until opened); `PlaceKind::section`, and `Row::section` on each row
  (subfolders take the one of their place): the interface draws a title and a line at
  each section change;
- **folded sections** (2026-10-27): `set_folded(section, bool)`, `is_folded`; the rows of a
  folded section leave `rows()` (the keyboard cursor skips them), its title stays
  (`sections()`: the sections that have rows, folded ones included); `Section::key()` gives
  a stable id, to remember the folding;
- `reveal(path)`: expands the parent folders of `path` under the most specific place
  that contains it (a volume before the root), to show the open folder;
- `refresh()`: rereads the places (card inserted) and the expanded folders; `reload()`:
  only the folders;
- the same folder present under two places (`/Volumes/SD` under "Volumes" and under the
  root) is expanded in both spots (state stored by path).

## Folder search (2026-10-19)

`/` or Ctrl+F in the folders panel. `parse_dir_query(text, current, home)` works out
where the search starts: `~/…` from the home folder, `/…` from the root or from the
longest existing path typed at the start (`/Volumes/Work/dehors`), otherwise from the
pointed folder; the rest is the pattern, fuzzy-matched (`tri::fuzzy`) against the
relative path. `walk_dirs` walks breadth-first (closest first), depth-limited
(`[gui.folders] search_depth`), without hidden folders or Time Machine backups,
cancellable. `DirSearch` does it in the background: results as they come (`take`),
`search_results` at most, stopped when dropped (new letter). `home()`: the home folder.

## Favorites (2026-10-18)

Folders chosen by the user, shown at the top of the tree (before the volumes), stored in
the data folder (`[data] dir`), file `favorites`, one path per line, in order of
addition:

- `favorites(data)`: the list, missing ones included: a favorite on a card or an
  unplugged disk comes back when the volume is remounted;
- `set_favorite(data, dir, on)`: adds (at the end) or removes, returns the list;
- `rename_favorites(data, from, to)`: after a rename, favorites that are `from` or inside
  it follow;
- **A favourite's name** (2026-10-22: several "Sources" folders in different places): on
  its line of the file, after a tab (a file without names is still read as is);
  `name_favorite(data, dir, name)` (empty: the folder's name comes back),
  `favorite_names(data)`, kept when another favourite is added or removed and when the
  folder is renamed; `Tree::set_favorite_names` shows it in the favourites section only
  (elsewhere, the folder keeps its name);
- **Separate unfolding** (2026-10-22): in the favourites section, a folder unfolds
  independently of the same folder in the tree (the unfolded state is keyed by
  (favourites or not, folder); folders already read stay shared); an interface unfolds a
  row by its rank, `Tree::toggle_row`;
- `Tree::set_favorites(list)`: depth-0 rows of kind `PlaceKind::Favorite`; folder
  existence is checked there, and on each `set_places` / `refresh`, not on each draw
  (slow NAS); `Row::favorite` also marks the folder wherever it appears in the tree.

Not yet: reordering favorites (order of addition), following an *undone* rename (`u`) or
one done outside `tri` (the favorite then disappears from the list, until the folder
comes back).

## Keyboard cursor (2026-10-18)

`Tree` keeps a row pointed at by the keyboard (tmux-style folders panel, see `gui.md`):
`cursor` (index in `rows`), `cursor_path`, `set_cursor`, `point_at`, `move_cursor`,
`cursor_to_end`, `cursor_right` (expands; already expanded: first subfolder),
`cursor_left` (collapses; already collapsed: parent folder). The cursor remembers its
index and its folder: a folder can appear under two places (the index decides), and rows
can appear above it (the folder finds its row again). In the engine, for a future TUI.
