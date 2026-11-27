# Light mode (`tri::light`, #597)

On a NAS or iCloud, computing a folder's whole cache means reading every RAW several
times over the network: camera previews, then exposure (the whole RAW), sharpness,
stacks, colours. In **light mode**, only **thumbnails** are computed in the background;
the rest waits until it is needed.

## What changes

- **Thumbnails**: computed in the background, as everywhere.
- **Preview of the shown image and its neighbours** (camera preview of a RAW, standard
  preview of a flat image): requested as soon as they are shown (`Scheduler::set_light`,
  then `set_shown`), never for the whole folder.
- **Analyses** (exposure, sharpness, stacks, colours): those set to "background" become
  "on demand" (the engine, photo by photo; `light::lighten` for the interface): their tool stays greyed out until the folder is
  computed, and what is already in the cache is read.
- **An image opened in full screen** (double-click, Enter): its whole cache, analyses
  included (`Scheduler::request_all`). Out of 2000 photos where you keep 25, only those
  25 cost a full computation.
- **"Compute all"** on right-click: on a folder, for that folder only (never the whole
  NAS at once); on an image or a selection, for them.

## Where it applies

Section `[light]`:

| Key | Default | Role |
|---|---|---|
| `network` | `true` | Network places (mounted shares, iCloud) in light mode by default |
| `folders` | `[]` | Folders in light mode, subfolders included |
| `except` | `[]` | Folders taken out of light mode, subfolders included |

The **deepest** rule wins (`LightSettings::root_of`): an added folder covers its
subfolders, and so does an exception; at equal depth, the user's choice wins over the
network place. The right-click menu of a folder in the panel (**Light mode** / **Leave
light mode**, action `core.folders.light`, no default key) rewrites these lists as short
as possible (`LightSettings::toggled`): going back on a choice removes the entry instead
of adding an opposite one.

## In All photos (#616)

All photos is under no place: light mode is decided there **photo by photo**, from each
one's folder (`Light::Folders`, the folders whose photos are in light mode,
`Scheduler::set_light_for`). The NAS photos get only their thumbnails in the background,
the others keep all their background work. The engine takes the analyses from the
settings and only reads them from the cache for photos in light mode; the interface
shows their tools "on demand" as soon as one photo is (greyed out while some are
missing). Status bar: the feather and "partly light", the folders on hover. Before
(report of 2026-11-26), All photos was never in light mode: opening it started every
computation, NAS included.

## Reminders

- Folders panel: a **feather** next to the root of light mode (the chosen folder, or the
  share), a crossed-out feather on an exception (`LightSettings::mark`).
- Status bar: the feather and "light" next to the folder name; on hover, the root.
