# Module `tri::process`: external programs

`tri` runs external programs: system tools (`diskutil`, `udisksctl`, `lsblk`,
PowerShell for ejecting on Windows, `explorer`, `xdg-open`…), development software
(`darktable-cli`, RapidRAW), the external editor, `curl` for updates. All go through
`process::command(program)`, which creates the command **without a console window on
Windows** (`CREATE_NO_WINDOW`): a graphical application starting a console program
opened one (issue #159, 2026-10-26: ejecting a card opened a PowerShell window; the
editor, a CMD window). Elsewhere, a plain command.
