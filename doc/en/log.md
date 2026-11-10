# Application log (`tri::log`)

Issue #398 (2026-11-10). A darktable render failed in three folders with nothing but
"1 failure (Darktable: darktable-cli)": neither the image nor the reason (darktable-cli
had written nothing on its error output). The cause, a damaged `.xmp`, was only found by
deleting every sidecar. At a user's, such a report cannot be diagnosed.

Hence the principle: **the status bar message stays short, the full detail goes to the
log**.

## The file

`tri.log`, in the data folder (`[data] dir`; otherwise `~/.local/share/tri` on Linux,
`~/Library/Application Support/tri` on macOS, `%APPDATA%\tri` on Windows). Both the
interface and the command line write to it.

An entry = one header line, then its detail indented by two spaces:

```
2026-11-10 15:03:12.345 ERROR [develop] #4711-17 Darktable failed: can't open xmp
  image: /Volumes/SD/DCIM/DSCF0001.RAF
  command: darktable-cli DSCF0001.RAF DSCF0001.RAF.xmp … --library :memory:
  exit: exit status: 1
  output:
  [xmp] can't open xmp file …
```

- **local date and time**, to the millisecond;
- **level**: `TRACE`, `INFO`, `WARN`, `ERROR`, `CRASH` (English and fixed: the log is
  attached to reports);
- **source**: the part of `tri` (`develop`, `export`, `preview`, `settings`, `gui`,
  `panic`, `gpu`…);
- **identifier** `<process>-<number>`: the status bar uses it to open the log on an error;
- **message** on one line; messages in English for the same reason, except the
  interface's, written as they were shown.

The file reads as is; `tri::log::parse` reads it back (a line that is not a header
extends the detail of the entry before).

## What goes into it

| Source | Level | Detail |
|---|---|---|
| Development software failing (render or export: darktable, RapidRAW, RawTherapee, ART) | `ERROR` | image, full command line, exit code, **all** its messages (`develop::tool_failed`) |
| Export of an image failing | `ERROR` | image, reason |
| Thumbnail, preview or render failing (scheduler) | `WARN` | level, image, reason |
| Invalid configuration | `WARN` | the reading message (excerpt of the file) |
| Any error shown in the status bar | `ERROR` | the message shown (source `gui`) |
| Crash (`panic`) | `CRASH` | version, system, thread, location, backtrace |
| Graphics card error | `CRASH` | wgpu's message; a burst (5 s) leaves only one |
| Detailed trace (`[log] trace`) | `TRACE` | image loading steps, slow frames |

## Size

Beyond `[log] max_kb` (1,024 KB), `tri.log` becomes `tri.1.log`, the old `tri.1.log`
becomes `tri.2.log`…, and `[log] keep` older files are kept (3). The log never makes an
action fail: a write that cannot be done is ignored.

Without a data folder, or without `tri::log::init` (tests, tools), nothing is written.

## Crashes

Until 2026-11-10, crashes went to a separate `crash.log`. At the first launch after, its
content is taken into `tri.log` (one `CRASH` entry "Earlier crashes") and the file is
deleted.

## In the interface

- **Click on an error** in the status bar: the log window opens on that error and on the
  entries the engine wrote just before (its detail), highlighted and unfolded; the
  message is cleared (as the click already did).
- **Log window** (palette: "Application log", `core.app.log`, no default shortcut; the
  help's "Log" button): newest entries first (500 at most, searching goes further back),
  levels shown (errors; warnings and errors, by default; all), fuzzy search (message,
  source, detail, date), detail unfolded on click, "Copy" one entry, "Copy all" (the
  entries shown), "Show in file manager", "Refresh".
- **Report a problem**: the latest entries (`[gui] report_log_entries`, 20) are offered
  in a dialog, **editable**: paths contain the names of personal folders. "Open the form
  with these lines" attaches them to the GitHub form ("Log" field, `logs` parameter,
  ~6,000 characters at most once encoded: the oldest lines go first); "Without the log"
  opens the form alone. With `report_log_entries = 0`, the form opens directly.

## Trace

`[log] trace = true` writes to the log what `TRI_GUI_TRACE=1` writes on the error output
(image loading steps, interface frames slower than `[gui] trace_slow_ms`). For a user
asked to reproduce a problem without a terminal; large, switch it off afterwards. Applied
live from the settings window.

## API

- `init(dir, settings)`: switches the log on (called by `tri` and `tri-gui` at startup);
  `set_settings`: new settings;
- `error`, `warning`, `info`, `trace`, `write`: one entry, returns its identifier;
- `read(dir)`: every entry, newest first; `tail(dir, n)`: the last `n` as text;
- `last_number`, `id_of`: tie an error shown to the entries written before it;
- `set_thread_dir`: a log for the current thread, for the interface tests (in parallel,
  each in its temporary folder).
