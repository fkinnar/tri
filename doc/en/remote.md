# Module `tri::remote`

Local copy of a folder of a server the system cannot mount: FTP and SFTP on Windows, SFTP
on macOS (see `Protocol::mountable` in [servers.md](servers.md)). The user's choice
(2026-10-29): copy locally, but first announce the number of images and the size, "to be
able to change one's mind before downloading 60 TB of data".

## No dependency

The tools shipped with Windows 10+ and macOS:

| Protocol | Tool | Listing a folder | Copy |
| --- | --- | --- | --- |
| FTP | `curl` | `curl ftp://server/folder/` (`ls -l` format, or IIS's DOS format) | one `curl -o` per file |
| SFTP | `sftp` (OpenSSH) | `ls -l` in batch mode (`-b -`) | `get`s in batches (`[remote] batch`, one connection each) |

`sftp` runs with `BatchMode=yes`: the user's SSH keys (agent included), nothing is asked.
`curl` gets the user of the address, with an empty password. The login box (password) is
the next step.

## Flow (`scan`, `download`)

1. `scan` walks the folder and its subfolders (down to `[remote] depth`, hidden `.…`
   folders left out) and keeps only the **images and XMP sidecars** (`media::kind_of`,
   `.xmp`): `Listing` (files, `images()`, `bytes()`). Cancellable between two folders.
2. The interface announces "1,234 images, 38.2 GB to copy", the editable destination
   (`default_destination`: `[remote] destination/server/folder`, `~/Pictures` by default,
   Tab completion), and that ratings set afterwards stay on the copy. Cancel: nothing is
   downloaded.
3. `download` copies keeping the subfolders; a file already there at the same size is
   skipped (resuming an interrupted copy). Progress (files, bytes) after each file or
   batch; "In the background" closes the box, progress goes to the status bar;
   cancellable. At the end, the copied folder opens.

## Settings `[remote]`

| Key | Default | Role |
| --- | --- | --- |
| `destination` | `"~/Pictures"` | folder of the copies, under `server/folder` |
| `depth` | `8` | depth of the subfolders walked |
| `batch` | `20` | files per run of `sftp` |

## Tests

Listings read (Unix, DOS, links and `.`/`..` left out), image / sidecar filter,
destination and URLs (encoding, port), a file already copied skipped without network;
interface: the confirmation box with a fake server, cancelled without downloading
anything. Never tried against a real FTP / SFTP server so far.
