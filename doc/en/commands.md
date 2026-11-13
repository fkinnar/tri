# Module `tri::commands`: the command line help

Issue 441. `tri`'s commands are described **once**, as data, in `src/commands.rs`
(`COMMANDS`): `tri --help`, the short usage shown on an argument error, the interface's
help (F1) and, when the day comes, the documentation of an API all come from it. No
`clap`: argument parsing (`src/main.rs`) stays hand-written; a test makes sure the two do
not drift apart.

## A command

`Command`: `id`, `theme` (`Theme`: folder and previews, ratings, files, export, offload,
analyses, cache and data, other; the order of the help), `syntax`, `main` (the options
that make the command, `--copy` and `--move`), `options` (the others, each described) and
`example`.

- **Syntax**: `[…]` optional, `a|b` one or the other, `…` repeatable. Words between `<…>`
  are translated (`cli-ph-<word>`: `<folder>` becomes `<dossier>` in French).
- **Texts** in the engine catalogues (`locales/*.ftl`): `cli-theme-<id>`, `cli-cmd-<id>`
  (one sentence), `cli-opt-<option>`; an option that means something else in one command
  has its own text, `cli-opt-<id>-<option>` (offload's `--to`: the root of the sessions;
  `--adjust`'s `--side`: the size of the written image), taken first
  (`Command::option_id`).
- **Example**: `Command::example(folder, images)` fills `<folder>` (the folder), `<images>`
  (the images) and `<image>` (the first one), paths quoted when needed (`shell_quote`:
  single quotes on Unix, double on Windows); other words stay, translated
  (`<destination>`). This is what the interface copies to the clipboard.

## Outputs

- `usage()`: the syntax of every command, and a pointer to `tri --help`.
- `help()`: by theme, every command with its description, its options and an example;
  then the common option `--config`. `tri --help` or `tri -h`.
- `Command::search_text()`: what the interface's help search goes through.

## Tests

- Every option cited by a syntax is in `main` or `options`, and the other way round.
- Every text (themes, commands, options, translated words) exists in every language.
- Every theme has a command, ids are unique; examples filled, paths quoted.
- `src/main.rs`: every option read by `parse()` is cited by a command (an option added
  without help fails the test).
- `tests/cli.rs`: `tri --help` and `-h` succeed and cite every command.
