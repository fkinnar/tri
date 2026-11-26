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
  Suggested session folder: `AAAA-MM-JJ_` (`suggest_folder`, capture date of the
  **newest** image to copy, read from its metadata in local time; failing that, its
  modification date in UTC; card already offloaded: the newest on the card; the oldest
  until 2026-11-12, which dated the folder after a session already taken from a card of
  several days; the suggestion follows the "RAW only" and "Offload again" checkboxes as
  long as it was not changed), to be completed
  ("_Self") or replaced entirely. The year, if wanted, goes into the root
  (`…/Sources/2026`): feedback from 2026-10-17, the `AAAA/AAAA-MM-JJ_` suggestion mixed
  the two, and stayed empty for a card already offloaded. A
  `_` left alone at the end of the name is removed; an **empty** name offloads into the
  root itself (#281: "offload here" into a folder just created). The name can neither be
  absolute nor
  go up (`..`): copies only go under the root. Later: templates (placeholders).
- **Original names**, flat: the card's subfolders (`DCIM/100_FUJI`…) are not
  reproduced. A name already taken by *another* file gets a suffix
  (`DSCF0001-2.RAF`, camera counter reset); nothing is ever overwritten.
  Later: renaming with templates.
- **Duplicates**, judged on the **destination folder** (feedback from 2026-10-17: the
  same card must be offloadable twice, to two places): an identical copy already present
  (same content, hash compared) is skipped; before copying, the dialog announces how many
  images are already in the chosen destination (`already_in`: same name, same size;
  recomputed when the folder changes). A **global history** (`offload-history.tsv` in the
  data folder, or `[offload] history`) also skips what has already been offloaded
  anywhere, **by default** since 2026-11-12 (`[offload] skip_known = true`; owner's
  request: do not offload again what was already taken; first an option of the
  configuration only, off by default). The dialog says it: "1686 images: 1200 already
  offloaded, 486 new", and offers, if there are any, an "Offload again the n images
  already offloaded" checkbox (unticked) to offload a card a second time elsewhere. A file is
  identified there by name, size and date (numbers restart at zero: the name alone is not
  enough), without a database: a text file, one line per copy. The history is kept in
  every case. It belongs to each machine (data folder): a card offloaded on the Mac is not
  known on Linux.
- **Full destination** (feedback from 2026-11-12: the offload failed without a word,
  every following file in error). A copy that fails for lack of room (`is_out_of_space`:
  disk full or quota exceeded) is not an ordinary error: `run` stops on that file and
  calls `full` with an `OutOfSpace` (files already copied, files and bytes still to copy,
  free room, `free_space`: `statvfs` on macOS and Linux, `GetDiskFreeSpaceExW` on
  Windows). `true` retries the **same** file, `false` stops there
  (`Report::out_of_space`): the rest stays on the card, neither copied nor in error, and
  nothing is deleted from the card nor ejected. In the interface, the dialog comes back to
  the front (even in the background) with what is done, what is left and the free room,
  and stays, without a time limit (Escape does not close it), until "Retry" (after making
  room) or "Give up". Before the copy, the dialog compares the size of the files to copy
  with the free room and warns if it is not enough; the button then reads "Offload
  anyway" (knowingly). The messages stay short, sizes in GB (feedback from 2026-11-18:
  long and confusing): "Not enough room: 12.4 GB to copy, 7.8 GB free.", the rest in
  the button's tooltip. On the command line, no question: a message, the stop, no eject,
  and an error exit code.
- **Verification** (`[offload] verify`): BLAKE3 hash computed during the copy, then the
  copy is read back and compared; original modification date preserved; atomic write
  (hidden temporary file, then rename).
- **Choices made before the copy, chained at the end** (feedback from 2026-10-17: having
  to choose between "open" and "eject" made no sense): checkboxes, defaults in
  `[offload]`:
  - **RAW only** (`raw_only`, off): flat images (camera JPEGs) stay on the card
    (`Report::left`); they do not count as "already offloaded" afterwards. Exclusive with
    "Group RAW and JPEG" (#469): both checkboxes stay in place, one greyed while the
    other is checked, a tooltip says why;
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

Feedback of the same evening: the dialog no longer has to stay open. Summary closed, or
copy done in the background: the application keeps the last finished offload
(`TriApp::last_offload`: session and the dialog's choices), and a camera card (with a
`DCIM` folder) plugged in afterwards opens the dialog on it, towards that session
(`OffloadDialog::propose_session`). Closed without offloading, nothing more is offered.
The new card may be mounted **at the same path** as the previous one (two cards of the
same camera have the same volume name, `/media/…/disk`): it has just appeared, so it is
another card.

## A card of several sessions (2026-11-25, #507)

A card can hold several sessions; often only one is to be offloaded.
`Plan::sessions(&settings)` first cut the card by shooting day (#600: also at pauses, see below) (the file date, set by the camera,
in local time; a photo taken before `[offload] day_cut` o'clock, 4 by default, counts for
the day before: an evening ending after midnight stays one session). A RAW+JPEG pair stays
in the session of its first image. `Plan::subset` restricts the inventory to the chosen
images: the verified copy, the history and deleting from the card afterwards only touch
them, never the rest of the card.

- **Dialog**: with several sessions, one row per session (checkbox with the day and times, folder, number of images
  and size to copy, or "already offloaded") instead of the "Session folder" field.
  Checked by default: those with something new. **One folder per checked session**
  (`YYYY-MM-DD_` to complete), copied one after the other in the same thread; the reports
  are put end to end (`Report::absorb`), the newest session opened at the end.
- **Command line**: `tri --offload <card> --session YYYY-MM-DD` only offloads that day
  (folder named after it; `YYYY-MM-DD_HHMM` when the day has several sessions); `--dry-run` lists the sessions.
- **Choosing the photos** of a session ("Choose the photos…" on its row,
  `offload_picker.rs`): a gallery instead of the dialog (user's idea, 2026-11-25: simpler
  than `tri`'s grid, which would have had to open on the card and then be given back as it
  was). Thumbnails from the embedded JPEGs, read in the background on the card without
  cache; a RAW+JPEG pair is one photo; adjustable size (slider); a check on each photo,
  all checked at first, "Select all" / "Deselect all" and the count. The grid's gestures:
  click = cursor (it does not uncheck everything, unlike the Finder), Cmd/Ctrl+click
  toggles, Shift+click checks the range, click on the check; Space toggles, `v` /
  Shift+Space the visual mode (moving checks), Shift + arrows extend, arrows or hjkl,
  Cmd/Ctrl+A all, with Shift none. Double-click or Enter: the photo large, fitted (the
  embedded preview, no 1:1), arrows for the next one. Escape leaves the visual mode, the
  large view, then closes without changing anything; OK goes back to the dialog ("Change
  the choice…"), only the selection is copied.

## Sessions cut where you want (2026-11-26, #600)

Days are only a special case (the owner's decision): the card is cut into sessions
**where you want**, and day changes are one of the proposed cuts. Example: the morning in
Brussels, the afternoon in Antwerp, two sessions on the same day.

- **Proposed cuts** (`Plan::proposed_cuts`): at each change of day (`[offload]
  split_days`, cut at `day_cut`) and where shooting stopped for at least
  `split_pause_minutes` (120 by default, 0: never). At most `max_sessions` sessions (30):
  beyond that, days then the longest pauses.
- **A cut names its first photo** (`Cut::first`, a file), not a time: a FAT card dates
  its files to 2 s, the photos of a burst often share the same time. `Plan::shots` gives
  the photos in shooting order (a RAW+JPEG pair is one photo, never cut);
  `Plan::sessions_at(&cuts, day_cut)` cuts the card.
- **Folder names**: the day (`YYYY-MM-DD_`), and the start time when other sessions of the
  card are on the same day (`2026-11-25_0900_`, `2026-11-25_1300_`).
- **Gallery of the whole card** ("Sessions…" in the dialog, next to the folder or the
  session rows): the sessions one after the other, each on a new row under a header
  (number, day, times, checked photos, why it starts there: new day, pause, chosen cut).
  With the keyboard, `c` starts a session at the photo under the cursor, or joins it to
  the one before. With the mouse, as on an editing table (the owner's idea): a toolbar
  always visible at the top, **scissors** (a click between two photos cuts, also after the last photo of a row; the
  aimed cut lights up) and **glue** (a click on a session's header joins it to the one before); the
  pointer becomes the tool, Alt or Ctrl (Cmd on macOS) gives the other one (on Linux Mint,
  Alt+click moves the window), Escape goes back to selecting.
  Right-clicking a photo cuts or joins too. Checks, gestures and the large view as in the gallery of one
  session, which stays on each row. Validating rebuilds the dialog's rows: a session is
  checked if one of its photos is, with its choice of photos; a folder typed by hand is
  kept if its session still starts at the same place. Cuts only last while the dialog is
  open.
- The gallery's thumbnails are centred in the window (#601).
- **Gallery images kept** (the owner's idea): thumbnails and large previews are stored in
  the engine cache, under the same keys as the grid. The same card opens faster, and the
  copy carries these entries (`preview::carry`, like a `tri` copy, #592; moved if the card
  is emptied afterwards): the offloaded session shows its thumbnails at once.
  `offload::run` gives (card file, copy) for each copy for this.

## Forgetting a card (2026-11-26, #602)

"Forget this card" (offload dialog, when images are already known; confirmed) and
`tri --offload <card> --forget`: `History::forget` removes the lines of the card's images
from the history (file rewritten in one go, the other lines kept as they are). The images
become new again; the copies already made are not touched. Use: reusing a test card.

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
- **"Keep tri's data on this drive"** (2026-11-11, issue #378, [store.md](store.md)):
  checkbox ticked by default when the destination is on a removable drive (otherwise
  "…in the session folder", not ticked); the switch is set before the copy, so the
  cache and the session's data go straight onto the drive. Unticked: `[cache] location`
  is not applied either.
- **Setting** `[offload] previews` (on by default), checkbox "Compute the previews during
  the copy" in the dialog. On the command line, `--previews`.
- Known limit: in the background, while another folder is viewed, two LibRaw workers may
  run at once (the displayed folder's and the session's).

## API

- `History::open(fichier)`, `History::open_configured(&réglages)`, `History::in_memory()`;
- `plan(carte, &historique) -> Plan`: images (`Item`: source, size, date, `known`),
  other files; `Plan::to_copy`, `Plan::bytes`;
- `suggest_folder(&plan, &settings)`, `destination(racine, nom)`;
- `run(&plan, destination, &réglages, &mut historique, &annulation, progression,
  copie) -> Report` (`copie`: each copy as soon as it is made, for the previews) (copied, skipped, errors, interrupted, duration); `Progress`: files (without those left on the card) and bytes (a skipped file, already offloaded or already there, counts at once: the bar follows the files), and the current step (`Phase`: preparing, copying, writing to the disk `sync_all`, verifying; #470, to tell a slow copy from a stuck one). The dialog shows the step, the file, the throughput, and "Nothing has moved for n s" after 3 s without progress;
- `Plan::shots`, `Plan::proposed_cuts`, `Plan::sessions`, `Plan::sessions_at` (`Cut`, `CutReason`, `Session`), `History::forget`;
- `delete_sources(&report)`.

## Command line

```
tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied] [--eject] [--session <name>] [--forget]
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
dialog (modal, in the centre) with inventory in the background, editable root and name
(full destination displayed), the checkboxes above, background copy with a progress bar
and "Interrupt", then the chained follow-ups and summary (the session opens in the grid
and its computations start). See [gui.md](gui.md).
