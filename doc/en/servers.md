# Module `tri::servers`

Join an SMB share by its address, like the Finder's "Connect to Server" (asked on
2026-10-28: on Windows, in WinBoat, the user's photos are on a `\\truc\machin` share that
nothing could reach). Before, `tri` only saw shares already mounted (the "Network" section
of the folders panel, see [places.md](places.md)).

## Addresses (`Server::parse`)

- `\\server\share[\folders]` (Windows; forward slashes accepted: `\\nas/photo`);
- `smb://[user@]server/share[/folders]`, everywhere (`%20` decoded).

A server and a share are required. The folders under the share
(`\\nas\photo\2026\session`) open once connected; only the share is remembered. Two
addresses name the same share if their first server names and their shares are equal,
case ignored (`NAS` = `nas.local` = `nas._smb._tcp.local`, the name given by Bonjour; an
IP address is compared whole).

## Protocols (`Protocol`, 2026-10-29)

Asked by the user: "the most common protocols, FTP included". The address gives the
protocol; what follows the server depends on it (`Server::share`, what gets mounted):

| Address | What is mounted | Linux | macOS | Windows |
| --- | --- | --- | --- | --- |
| `smb://`, `\\` | the share (1st folder) | ✅ GVFS | ✅ | ✅ UNC |
| `afp://` | the share | ✅ | ✅ | ❌ |
| `ftp://` | the whole server | ✅ | ✅ (read-only) | ❌ |
| `sftp://` | the whole server | ✅ | ❌ | ❌ |
| `dav://`, `davs://` (`webdav(s)://`) | the whole path | ✅ | ✅ (`http(s)://`) | ✅ UNC `\\nas@SSL\…` (WebClient service) |
| `nfs://` | the whole path (the export) | ✅ | ✅ | ❌ |

`[user@]` and `:port` everywhere (`ftp://me@nas:2121/`). The ❌ cases give a clear message
(`Protocol::mountable`), except FTP and SFTP: **local copy**, confirmed first (number of
images and size announced, see [remote.md](remote.md)). A mount is recognised if it covers the start of the wanted path
(`sftp://nas/home/me/photo` in a mount of `sftp://nas/`), over SMB or AFP for the same
share, over WebDAV encrypted or not.

## Connecting (`Server::connect`): the system mounts, `tri` asks for the account

No SMB client in `tri`: the system mounts. When the server asks for an account that the
system does not ask for itself, `connect` returns `ConnectError::NeedsLogin` and the
interface opens its **login box** (2026-10-29): user, domain (SMB), password, "Remember
the password"; then `connect` is called again with the `Login`.

| System | How | Account |
| --- | --- | --- |
| Windows | UNC path read directly (`\\nas\photo`); access refused (codes 5, 86, 1326, 1244): the box, then `WNetAddConnection2` | Windows credentials; "Remember": Credential Manager (`CredWrite`, like `cmdkey /add`) |
| macOS | `osascript -e 'mount volume "smb://…"'`, mounted in `/Volumes` | system dialog, keychain (no `tri` box) |
| Linux | `gio mount smb://…` (GVFS, like Nemo and Nautilus), its questions answered by `tri` | GNOME keyring if already saved; otherwise the box; "Remember": `secret-tool` (desktop keyring, attributes read by GVFS; best effort) |

On Linux, `gio mount` asks its questions on its output (`User [fabrice]: `,
`Domain [WORKGROUP]: `, `Password: `, `LC_ALL=C`); `tri` reads them as they come and
answers with the account typed. Without an account, the first question opens the box; a
question after the password means a refusal: the box comes back with "user name or
password refused". A multiple-choice question (unknown SFTP host key…) stops with its
text: connect once from the file manager. The password never goes on a command line and
is never written by `tri` (only by the system, when "Remember" is ticked).

Errors are explained by cause: server unreachable (name, network), share unknown (type
the server alone to see its shares), already connected under another account (Windows,
`net use`), account refused.

A share already mounted is not mounted again: its folder is found among the mounts
(`Mounts`: output of `mount` on macOS — `smbfs`, `afpfs`, `ftp`, `webdav`, `nfs` —,
`/proc/mounts` for CIFS, NFS, sshfs and davfs, and the GVFS mounts on Linux), **over SMB as well as AFP** (feedback of 2026-10-29: a NAS that advertises both is
often mounted over AFP by Nemo, `afp-volume:host=…,volume=…`, and was not recognised).
Connecting can take several seconds: the interface waits for it in a separate thread.

## Shares of a server (`Host`)

An address without a share, followed by a slash (`smb://nas.local/`, `\\nas\`): `tri`
asks the system for the server's folder shares (`Host::shares`, hidden `…$` shares left
out), like Nemo's "Windows shares on nas" (feedback of 2026-10-29: `smb://nas.local/`
followed by Enter did nothing). Not before the slash: the name is probably still being
typed; Enter on a server alone (`\\host.lan`, feedback of the same day in WinBoat)
adds the slash and starts the listing, as the "Enter: shares of host.lan" line under the
field says.

| System | How |
| --- | --- |
| Windows | `NetShareEnum` (no `net view` output to read, translated in Windows' language) |
| macOS | `smbutil view -N` (keychain credentials, asking nothing), otherwise `-g` (guest) |
| Linux | `gio mount smb://nas/` (anonymous browsing) then `gio list smb://nas/` |

## Remembered shares

File `servers` in the data folder (`[data] dir`): one `smb://` address per line
(`remembered`, `remember`, `forget`). A joined share is added there; it shows in the
"Network" section (`Server::place`):

- on Windows, its UNC path, a folder like any other (`PlaceKind::Network`);
- elsewhere, while it is not mounted, a greyed `PlaceKind::Server` row (path = its
  address) that a click joins; once mounted, it shows like the other mounted shares.

## In the interface

- "Server" icon in the folders panel header, or `c` when the panel has the keyboard
  (`core.folders.connect`): the search field opens with `\\` (Windows) or `smb://`; while
  the address is incomplete, it shows the expected format.
- A complete address typed in the search (also without the icon): first row "Connect to
  "photo (nas)"", Enter or click.
- A server alone (`smb://nas.local/`): one "Connect" row per share (list asked in the
  background, "looking for the shares…" meanwhile); typing the start of a name
  (`smb://nas.local/kp`) narrows the list. ↑ / ↓ and Enter, or a click.
- Right click on a remembered share: "Connect" (if it is not), "Forget this share"
  (`core.folders.forget`, Shift+C).
- On Windows, a drive letter mapped to a share (`Z:`) goes under "Network".

## Later

A real network neighbourhood (discovering servers, Bonjour / mDNS): next step. The login
box has not been tried against real servers yet (Windows, Mint); "Remember" on Linux
depends on the desktop keyring.
