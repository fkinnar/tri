# Module `tri::neighbourhood` — network neighbourhood

The servers that announce themselves on the local network (Bonjour / mDNS, as in the
Finder, Nemo or Explorer) appear in the "Network" section of the folders panel, without
typing their address (asked for on 2026-10-29 for the beta).

## Listening (`Browser`)

`Browser::start(notify)` listens in the background, through the `mdns-sd` crate (pure
Rust, no async runtime), for these service types, in order of preference to join a
server that announces several: `_smb._tcp`, `_afpovertcp._tcp`, `_sftp-ssh._tcp`,
`_webdavs._tcp`, `_webdav._tcp`, `_ftp._tcp`, `_nfs._tcp`. Each announce or removal
becomes an `Event` (`Found(Service)`, `Removed(full name)`); `notify` is called on each
change (the interface redraws), `Browser::poll` applies them. Listening stops when the
`Browser` is dropped (the application closes).

Listening opens a multicast socket (port 5353): on Windows, the firewall may ask for
permission the first time. `[neighbourhood] enabled = false` turns it off.

## Servers (`Neighbourhood`, `Neighbour`)

`Neighbourhood` keeps the announced services and gathers them by server (same host name,
case ignored): `neighbours()` gives them by name, each with its services from the
preferred one down. The name shown is the one of the preferred announce (the SMB announce
often carries the name chosen for the NAS, the SFTP one that of the machine).

`Neighbour::address(os)`: the server alone for SMB and AFP (`smb://nas.local/`,
`\\nas.local\` on Windows) — its shares are listed in the folder search
([servers.md](servers.md), "Shares of a server") —; otherwise the announced path (`path`
of the TXT record: WebDAV, NFS, FTP), with the port if it is not the usual one.
`Neighbour::place(os)` makes it a `PlaceKind::Neighbour` place ("Photos (SMB, AFP)"),
added by `places::Tree::set_neighbours`.

## In the interface

Listening starts at the first frame (never in tests). A click on a server opens its
address in the folder search: its shares are listed there, Enter joins the one under the
cursor (login box if the server asks for an account). A server leaving the network
disappears from the list.

## Tests

Unit (`src/neighbourhood.rs`): announces gathered by server, preferred protocol first,
addresses read back by `servers::Host` / `Server`, removal. Interface: a fake
neighbourhood listed under "Network", a click that lists the (fake) shares, removal.
Nothing listens to the real network in tests.
