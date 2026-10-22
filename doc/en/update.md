# Checking for updates (`tri::update`)

Added on 2026-10-21, for beta testers: knowing whether a newer version exists without
going to GitHub.

## Principle

- **On demand only**: "Check for updates" button at the top of the help (F1), action
  `core.app.check_update` (no default shortcut, assignable in `[keys]`), or
  `tri --check-update`. Nothing goes over the network without this gesture: no check at
  startup, no notification.
- **Source**: the public GitHub Releases API (`[update] url`, no account nor token), read
  with **`curl`** (present on macOS, Linux and Windows 10+), timeout `[update] timeout_s`
  (10 s). No HTTP client in `tri`: no TLS stack to ship, no extra licences. Without
  `curl` or without network, the error is shown as is.
- **Comparison**: SemVer numbers (`Version`), pre-releases included (`tri` is in alpha,
  then beta): `1.0.0-alpha.3` < `1.0.0-beta.1` < `1.0.0`. Drafts left out, unreadable
  tags ignored.
- **Answer**: "You have the latest version", or the newer version with its notes (those
  of the Releases page, taken from `CHANGELOG.md`) and a button opening its page in the
  browser. **No automatic install** (after V1: signing, rights, replacing the running
  application).
- **Nightly and personal builds** (2026-10-22, for the forgetful): a binary that is not
  a released version says so, and counts the commits the `main` branch has that it lacks
  (GitHub "compare" API, `[update] compare_url`): "git pull, then rebuild" for a personal
  build, "in the next nightly build" for a nightly one; "up to date", or "cannot compare"
  when its commit is not on GitHub (local commits not pushed yet). The provenance is
  baked in at compile time by `build.rs`: the commit (`git rev-parse HEAD`, rebuilt when
  `HEAD` moves) and `TRI_BUILD_CHANNEL`, set by the CI (`release` on a tag, `nightly` for
  the nightly build, see [packaging.md](packaging.md)); without it, a personal build. The
  version number in the help and on the logo carries it: `1.0.0-alpha.3 (e51c81b)`,
  `1.0.0-alpha.3 nightly e51c81b`.

## API

| Item | Role |
|---|---|
| `UpdateSettings` | `[update]` section: `url`, `compare_url`, `timeout_s` |
| `Build::current()` | this binary: version, commit, `Channel` (release, nightly, personal); `label()` |
| `Version::parse` | "1.0.0-alpha.3" (leading "v" accepted), SemVer order |
| `parse_releases(json)` | published versions from the API answer (drafts left out) |
| `newer(current, releases)` | the newest of the versions newer than `current` |
| `parse_compare(json)` | commits of `main` missing from the build (`ahead_by` of `commit...main`) |
| `fetch(settings)` / `check(build, settings)` | request with `curl`, then `Status`: `Latest`, `Newer(release)`, `Build { behind }` |
| `build_text(build, behind)` | the two lines shown for a nightly or personal build |

In the interface, the request runs in a separate thread (`TriApp::check_update`); the
dialog (`update_dialog`) shows "checking", then the result; Escape closes it.

## Tests

Unit: SemVer order (numeric and textual pre-release identifiers, final version after its
pre-releases, unreadable numbers), reading an answer (draft left out, non-numeric tag
ignored, offered version), personal and nightly builds (commits to pull from a local
"compare" answer, no request without a commit, label). Interface: list served from a
local file (`file://`, read by `curl`, no network): newer version with its notes, then
"latest version"; a personal build with commits to pull.
