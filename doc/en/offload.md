# Module `tri::offload`

Offloading a card (or any volume) to a local folder, then culling in the same tool
(requested on 2026-10-17: "empty the card, cull, develop"). No interface: the command
line and the graphical interface use it as is.

## Choices (2026-10-17)

The defaults follow the user's own practice, everything is configurable; principle: cover
everyone's needs, not just his.

- **Destination**: root + session folder. The root is **remembered** from one offload to
  the next (`root` / `remember_root`, file `offload-root` in the data folder);
  `[offload] destination` (`~/Pictures`, `~` accepted) is only used for the first one.
  Suggested session folder: `AAAA-MM-JJ_` (`suggest_folder`, capture date of the oldest
  image to copy, read from its metadata in local time; failing that, its modification
  date in UTC; card already offloaded: the oldest on the card), to be completed
  ("_Self") or replaced entirely. The year, if wanted, goes into the root
  (`…/Sources/2026`): feedback from 2026-10-17, the `AAAA/AAAA-MM-JJ_` suggestion mixed
  the two, and stayed empty for a card already offloaded. A
  `_` left alone at the end of the name is removed. The name can neither be absolute nor
  go up (`..`): copies only go under the root. Later: templates (placeholders).
- **Original names**, flat: the card's subfolders (`DCIM/100_FUJI`…) are not
  reproduced. A name already taken by *another* file gets a suffix
  (`DSCF0001-2.RAF`, camera counter reset); nothing is ever overwritten.
  Later: renaming with templates.
- **Duplicates**, judged on the **destination folder** (feedback from 2026-10-17: the
  same card must be offloadable twice, to two places): an identical copy already present
  (same content, hash compared) is skipped; before copying, the dialog announces how many
  images are already in the chosen destination (`already_in`: same name, same size;
  recomputed when the folder changes). As an option (`[offload] skip_known`, off by
  default), a **global history** (`offload-history.tsv` in the data folder, or
  `[offload] history`) skips what has already been offloaded anywhere; a file is
  identified there by name, size and date (numbers restart at zero: the name alone is not
  enough). The history is kept in every case.
- **Verification** (`[offload] verify`): BLAKE3 hash computed during the copy, then the
  copy is read back and compared; original modification date preserved; atomic write
  (hidden temporary file, then rename).
- **Choices made before the copy, chained at the end** (feedback from 2026-10-17: having
  to choose between "open" and "eject" made no sense): checkboxes, defaults in
  `[offload]`:
  - **RAW only** (`raw_only`, off): flat images (camera JPEGs) stay on the card
    (`Report::left`); they do not count as "already offloaded" afterwards;
  - **open the folder when done** (`open_after`, on);
  - **eject the card when done** (`eject_after`, on), unless the copy was interrupted;
  - **delete copied and verified files from the card** (`delete_after`, off):
    `delete_sources` — those copied during this offload **and** those already present
    identically in the destination (`Report::already_there`, hash compared: card
    offloaded a second time to the same place; added on 2026-10-17, they stayed on the
    card), never those known only to the global history (verified nowhere), nor errors,
    nor videos; each copy is checked once more — present, same size — just before; not
    after an interruption.

  - **also delete the JPEGs left on the card** (`delete_left`, off; offered only with
    "RAW only" and deletion checked): `delete_left`, for those who keep only the RAWs
    (they are not copied: lost).

  Order: deletion, opening, ejection. If the system refuses ejection (volume in use by
  another application: seen on 2026-10-17 with ForkLift open on the card, "Unmount was
  dissented by PID … ForkLift"), the message is shown with a "Retry ejecting" button in
  the status bar; the summary stays displayed, with buttons for what was not checked.
- Only images (RAW and flat) are copied; videos and camera files are counted
  (`Plan::ignored`) and left.

## Several cards in a row (2026-10-25, issue #37)

With the dialog left open on the summary of a card (ejected or not), a new card plugged
in: the dialog starts again on it (`OffloadDialog::next_card`, called by the volume
watcher, `TriApp::set_volumes`), towards **the same session folder** (root and name
taken from the previous offload), with the same choices; only the copy is left to start.
Only ejectable media count (not a fixed drive, `[places] fixed`). The previews of each
session are computed apart: the application keeps a list of their schedulers.

## In the background (2026-10-17)

To cull one session while another card is being offloaded: during the copy, "Continue in
the background" (or Esc, or a click outside) closes the dialog; the copy continues, the
progress shows in the status bar (a click reopens the dialog, with "Interrupt"). At the
end, in the background, the checked deletion and ejection happen as planned, but the
session is **not opened automatically** — that would change folders right under the
user's eyes —: the status bar offers "Open the session" (and "dismiss"), with the
summary. Dialog left open until the end: automatic opening as before. Only one offload at
a time: the "offload" icon disappears from the volumes while an offload exists.

## Previews during the copy (2026-10-25, issue #40)

A card is offloaded to work on it: waiting for the end of the copy to compute the
previews wastes time, while the copy (reads and writes) leaves the processor almost idle.
Each copied and verified photo (or one found identical in the destination) joins a
background computation at once: thumbnails, camera JPEGs, histograms, sharpness,
grouping, as when a folder is opened.

- **From the copy, not from the card**: the photo has just been written, the system
  still has it in memory, and the card reads are not slowed down.
- **A separate scheduler** (`Scheduler::add_files`: photos added one by one, nothing
  interrupted), with the preview settings of the interface's engine: the same keys as the
  folder opened afterwards. The application keeps it, not the dialog (which may close
  before the end); it stops when the session is opened (the folder's engine takes over
  what is left, what is done is read from the cache) or once everything is computed.
- **`[cache] location`** is applied to the session before the copy: its cache fills up
  in its place directly.
- **Setting** `[offload] previews` (on by default), checkbox "Compute the previews during
  the copy" in the dialog. On the command line, `--previews`.
- Known limit: in the background, while another folder is viewed, two LibRaw workers may
  run at once (the displayed folder's and the session's).

## API

- `History::open(fichier)`, `History::open_configured(&réglages)`, `History::in_memory()`;
- `plan(carte, &historique) -> Plan`: images (`Item`: source, size, date, `known`),
  other files; `Plan::to_copy`, `Plan::bytes`;
- `suggest_folder(&plan, skip_known)`, `destination(racine, nom)`;
- `run(&plan, destination, &réglages, &mut historique, &annulation, progression,
  copie) -> Report` (`copie`: each copy as soon as it is made, for the previews) (copied, skipped, errors, interrupted, duration); `Progress`: files (without those left on the card) and bytes;
- `delete_sources(&report)`.

## Command line

```
tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied] [--eject]
```

On the command line, nothing happens on the card without an explicit option (`--eject`,
`--delete-copied`; `open_after`, `eject_after`, `delete_after` only apply to the
interface). `--dry-run` lists what would be copied; `--previews` computes the session's
thumbnails, previews and analyses during the copy, then waits for the end of the
computations. Measured (2026-10-17, local disk image,
Apple Silicon Mac): 4 RAFs (109 MB) copied and verified in 0.23 s; second pass: "nothing
to copy".

## Interface

"Offload" icon next to the eject icon of each non-fixed volume in the folders panel:
dialog (modal, near the click) with inventory in the background, editable root and name
(full destination displayed), the checkboxes above, background copy with a progress bar
and "Interrupt", then the chained follow-ups and summary (the session opens in the grid
and its computations start). See [gui.md](gui.md).
