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

## API

| Item | Role |
|---|---|
| `UpdateSettings` | `[update]` section: `url`, `timeout_s` |
| `Version::parse` | "1.0.0-alpha.3" (leading "v" accepted), SemVer order |
| `parse_releases(json)` | published versions from the API answer (drafts left out) |
| `newer(current, releases)` | the newest of the versions newer than `current` |
| `fetch(settings)` / `check(current, settings)` | request with `curl`, then comparison |

In the interface, the request runs in a separate thread (`TriApp::check_update`); the
dialog (`update_dialog`) shows "checking", then the result; Escape closes it.

## Tests

Unit: SemVer order (numeric and textual pre-release identifiers, final version after its
pre-releases, unreadable numbers), reading an answer (draft left out, non-numeric tag
ignored, offered version). Interface: list served from a local file (`file://`, read by
`curl`, no network): newer version with its notes, then "latest version".
