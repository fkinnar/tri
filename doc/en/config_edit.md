# Module `tri::config_edit`: editing the configuration from an interface

Decided on 2026-10-17, done on 2026-10-18: settings have become numerous
(`config.example.toml`), and they must be changeable without opening the file by hand.
The interface (settings window, `Ctrl+,`, see `gui.md`) relies on this module, which
depends on no display (a future TUI could do the same).

## `Schema`: the list of settings comes from the example file

`Schema::parse(text)` reads `doc/config.example.toml`: sections (`gui.colors`), keys,
default values and **comments**. Why the example rather than a list written in the code:

- a test already checks that the example stays equal to the code's default values
  (`settings.md`); the list is therefore correct, and every new setting (which must be
  born in the example, a project rule) shows up in the window by itself;
- the example's comments are the documentation of each setting: the window shows them
  as they are.

A comment above a group of keys ("Badges de la grille : …" above `badge_over` and
`badge_under`) describes the group: `Group { doc, entries }`. Only the last paragraph of
a comment counts (the file header, separated by a blank line, is not the description of
`[preview]`). A section without keys (`[keys]`, whose keys are action identifiers chosen
by the user) is listed without settings: the interface gives it its own pane (its
comment, with no key below it, gets lost: toml_edit puts it at the end of the document).
Keys only mentioned in a comment (`# selection = "#1e6f7f"`, optional setting with no
default value): to be added by hand in the file.

Known limitation: the example's comments are in French; an English example would have
to be added for English descriptions.

## `ConfigFile`: editing without losing anything

`ConfigFile::open(path)` (missing file: empty, created on save), `get`, `set`, `remove`
(back to the default value), `text`, `save`. The file is held by `toml_edit`, which keeps
the original text around what is changed: comments, order, layout, keys unknown to the
interface. A replaced value keeps its line's comment (`thumbnail = 600 # à moi`). A
created subsection (`[gui.colors]`) does not create an empty `[gui]`. Saving goes through
a renamed temporary file: never a half-written file.

Values: `Value::{Bool, Int, Float, Str, Toml, Strings}`; `Toml` for lists and tables,
edited as TOML text (`[1280.0, 800.0]`); `Strings`, a list of strings written by the
interface (shortcuts of an action: `"core.flag.accept" = ["Maj+P", "a"]`, identifier in
quotes, escaping done by toml_edit). `parse_color` recognises `"#rrggbb"`.

## Validation: up to the caller

The module does not know the full type of the settings (the interface adds its own to
the engine's). The caller validates the modified text with `tri::settings::parse::<T>`:
the settings window refuses a value that would make the file unreadable (wrong type,
value outside the enumeration), restores the old one and shows the `serde` message
(which lists the possible values).
