# Module `tri::process`: external programs

`tri` runs external programs: system tools (`diskutil`, `udisksctl`, `lsblk`,
PowerShell for ejecting on Windows, `explorer`, `xdg-open`…), development software
(`darktable-cli`, RapidRAW), the external editor, `curl` for updates. All go through
`process::command(program)`, which creates the command **without a console window on
Windows** (`CREATE_NO_WINDOW`): a graphical application starting a console program
opened one (issue #159, 2026-10-26: ejecting a card opened a PowerShell window; the
editor, a CMD window). Elsewhere, a plain command.

## "Busy" executable (`process::spawn`)

Development software is started with `process::spawn(&mut command)`. On Linux, an
executable just written can be refused for a moment with "Text file busy" (`ETXTBSY`): a
process created at the same time elsewhere in `tri` inherited the file still open for
writing, until its `exec`. `spawn` then tries again up to 5 times, waiting 5 ms, then 10,
20, 40 and 80 ms (155 ms in all); any other error comes back at once. Seen on the tests'
fake tools (Ubuntu CI, 2026-11-11, test
`rawtherapee_and_art_export_into_a_folder_then_tri_shrinks`); possible too for a tool just
installed or extracted from an AppImage. Test: `a_busy_executable_is_launched_once_freed`.
