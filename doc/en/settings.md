# Settings: configuration file

Project rule (`CLAUDE.md`, 2026-10-16): **no hard-coded values**. Every "knob" (size,
delay, capacity, quality, threshold, number of workers, color…) is a setting, with its
default value in the code, and can be changed in a configuration file without
recompiling.

## The file

**TOML** format (chosen on 2026-10-16: comments allowed, readable sections, standard in
the Rust ecosystem; JSON stays reserved for the FFI boundary planned by the
architecture).

| System | Default location |
|---|---|
| Linux and other Unix | `$XDG_CONFIG_HOME/tri/config.toml` (if absolute), otherwise `~/.config/tri/config.toml` |
| macOS | `~/Library/Application Support/tri/config.toml` |
| Windows | `%APPDATA%\tri\config.toml` |

Or any file with `--config FILE`, for `tri` as well as `tri-gui`. Command-line options
(`--cache`, `--side`, `--workers`…) take precedence over the file.

**All keys, their default values and their role**: [`config.example.en.toml`](../config.example.en.toml)
(the reference, described in French: [`config.example.toml`](../config.example.toml), same
keys and values; the settings window shows the descriptions in the interface language).
Just copy the lines to change: any missing key keeps its default value, and a missing
file is equivalent to the default settings. Two tests (engine and interface) check that
this example file stays **exactly** equal to the values in the code: a default value
cannot change without updating its documentation.

| Section | Contents | Read by |
|---|---|---|
| `[preview]` | Sizes (thumbnail, preview "auto" or fixed), `half_size` rule, acceptance thresholds for the camera preview | engine |
| `[render]` | LibRaw development: white balance, brightness, automatic brightness | engine |
| `[cache]` | Folder, JPEG quality, maximum size of 1:1 previews (least recently used eviction) | engine |
| `[scheduler]` | Light workers ("auto" = half the cores), thumbnails first, background exposure analysis | engine |
| `[xmp]` | Convention for a new sidecar (`darktable` / `adobe`), one or several colors | engine |
| `[places]` | Places in the folders panel: hide Time Machine, patterns to exclude, fixed volumes (no ejection), volume monitoring interval | engine |
| `[exposure]` | Saturation and crushed-shadow thresholds (EV), histogram bins, mask size | engine |
| `[offload]` | Card offloading: root, verification, already offloaded files, history | engine |
| `[remote]` | Local copy of a server the system cannot mount (FTP, SFTP): destination, depth, batches | engine |
| `[i18n]` | Text language: `auto` (system), `fr`, `en` (`i18n.md`) | engine |
| `[sharpness]` | Sharpness map: threshold, thickening, isolated points (`sharpness.md`) | engine |
| `[files]` | File actions: allowed or not, pairs that follow, trash or permanent deletion, external editor (`files.md`) | engine |
| `[data]` | Data folder (offload history, last opened folder) | engine |
| `[gui]` | Theme, shortcut profiles, key sequence delay (`gg`), window, RAW render policy, loading budget | interface |
| `[gui.grid]` | Grid dimensions, thumbnails in memory, size of the ratings, opacity of a rejected photo, hover frame | interface |
| `[gui.view]` | Wait before a RAW 100 %, large textures in memory, keyboard pan step, size of the ratings | interface |
| `[gui.folders]` | Folders panel: shown, width, indentation, hidden folders | interface |
| `[gui.histogram]` | Histogram (shown, EV / linear scale, size of the large histogram, EV shown), zones (shown, opacity), badge thresholds | interface |
| `[gui.metadata]` | Metadata panel: shown, width, histogram in the grid and its height | interface |
| `[gui.sharpness]` | Sharpness map shown at startup, rotation colors, opacity | interface |
| `[gui.compare]` | Comparison: maximum images, spacing, frame of the active image | interface |
| `[gui.colors]` | `"#rrggbb"` colors (selection, hover frame, progress bars, badge, errors, full-screen background, stars, flags, rating colors) | interface |
| `[keys]` | Shortcuts per action, which replace those of the profiles (see [gui.md](gui.md#actions-and-shortcuts-actionsrs)) | interface |

### Errors

An unknown key in a section (typo), a value of the wrong type or out of range is
**reported**, with the line and the list of valid keys:

```
warning: configuration ~/.config/tri/config.toml: TOML parse error at line 2, column 1
  |
2 | thumbnial = 300
  | ^^^^^^^^^
unknown field `thumbnial`, expected one of `thumbnail`, `standard`, …; default settings used
```

The program starts anyway, with **all** the default settings (not a mix: a half-applied
file would be more misleading). A typo must not prevent sorting one's photos. The
interface shows "invalid configuration: default settings" in red in the status bar, with
the full message on hover; the command line writes it to standard error. A file
requested with `--config` but not found is reported the same way; a missing file at the
default location is not.

## Settings window

In the interface, `Ctrl+,` opens a window that edits this file without losing its
comments or its other keys (see `config_edit.md` and `gui.md`).

## Module `tri::settings` (engine)

- **`EngineSettings`**: the engine sections (`PreviewSection`, `RenderSection`,
  `CacheSection`, `SchedulerSection`). Sections unknown to the engine (`[gui]`) are
  ignored here, so that the command line reads the same file as the interface.
  - `preview(screen_side)` → `preview::Settings`: resolves the "auto" size (long side of
    the screen; `auto_fallback` without a screen, on the command line);
  - `scheduler(raw_previews)` → `scheduler::Config`: resolves "auto" (half the cores);
    the RAW render policy is a parameter because it depends on the use (the interface
    computes them on demand, the `--previews` demo all in the background);
  - `open_cache()`: cache in the configured folder (otherwise the system one), with the
    configured quality.
- **`Auto<T>`**: `"auto"` or a positive integer, with a clear error message for any
  other value (`"automatique"`, `-1`, `1.5`).
- **`load::<T>(path)`** → `Loaded { settings, path, warning }`: generic over the type of
  the whole file, so that the interface reads the same file with its own type.
- **`parse::<T>(text)`**, **`default_path()`**.

The sections' default values are not copied: they come from the engine types
(`preview::Settings::default()`, `RenderOptions::default()`,
`cache::DEFAULT_JPEG_QUALITY`, `scheduler::Config::default()`). A single source.

Why file types (`PreviewSection`…) distinct from internal types (`preview::Settings`…):
the file contains values to resolve ("auto") and presentation choices (no `half_size`,
set per level); the file contract is thus decoupled from the internal API, in the spirit
of the JSON boundary planned for the FFI.

### Settings and cache

Each computation setting enters the cache key **only of the levels it influences**
(`preview::params`): changing `auto_bright` recomputes the RAW renders (standard and 1:1
previews), not the thumbnails or the camera previews. Two exceptions, deliberate and
documented in the code:

- `cache.jpeg_quality` does not enter the key: entries already written represent the
  same image; clear the cache (`tri --clear-cache`) to rewrite them;
- the `half_size` fallback for thumbnails of RAWs **without** an embedded preview (rare)
  does not put the render in it, so as not to invalidate all thumbnails at each render
  setting change.

## Interface: `gui/src/settings.rs`

`AppSettings` takes the four engine sections and adds `[gui]` (`GuiSettings`,
`GridSettings`, `ViewSettings`, `Colors`). Here, **any** unknown section is an error
(`[previw]` is reported). `Color` is read from `"#rrggbb"`; missing selection and hover =
theme colors. An empty list of shortcut profiles falls back to the default profiles
(otherwise the keyboard would no longer do anything).

## Not yet configurable

Updated at the review of 2026-10-28 (the 1:1 cache eviction, `[cache] zoom_max_mb`, and
the shortcuts, `[keys]`, have been configurable for a long time).

- The scheduler priorities (classes 0 to 5): that is the algorithm, not a knob. Likewise
  the weights of the fuzzy search (`fuzzy`) and the number of lock attempts on a volume
  to eject on Windows (`places`, after Microsoft's sample).
- The polling interval of an external program at work (`darktable-cli`, RapidRAW: 50 ms,
  then 20 ms for its last lines): to make configurable only if it weighed, which it does
  not.
- The drawing of the interface: font sizes, margins, spacing, stroke widths and part of
  the colours (overlay backgrounds and texts, histogram, crop handles, icons). They will
  belong to a future theme system rather than one key per place (see the review report of
  2026-10-28).

## Display state (`gui-state.toml`)

Next to the last opened folder, in the data folder, the interface keeps what is shown or
hidden with a key (focus point, sharpness map, zones, histogram, panels, bars, automatic
advance): these values take precedence over the configuration from the second launch
on. `[gui] remember_display = false` to always start from the configuration; delete
`gui-state.toml` to go back to it once.
