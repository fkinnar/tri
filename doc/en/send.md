# Module `tri::send`: send plugins

Sending images somewhere: AirDrop first (macOS, what the user uses), then other
destinations as needed (synced folder, KDE Connect…). Planned for "after V1", brought
forward to 2026-10-19 to put the plugin architecture to the test.

## The trait and the list

`Sender`: `id` (short name), `name` (displayed), `available` (on this machine), `send`
(files). `senders()` lists them, `sender(id)` finds one. Each plugin is written in Rust
and compiled with `tri`: static traits and a list, as decided.

Its action in the interface is in the **plugin namespace**: `plugin.<id>.send`
(`plugin.airdrop.send`). It has no default key: it is set in `[keys]` or the settings
window. It has a button (share icon) in the right-click box, and appears in the help,
only where the plugin is available. It applies to the selection (or the current image).

## AirDrop

The macOS share sheet (`NSSharingService`, "Send with AirDrop" service) called from Rust
with `objc2-app-kit`: the system shows the devices in range, the user chooses, the
sending happens in the background. AppKit interface: **main thread required**;
elsewhere, `send` refuses cleanly (checked by a test, since tests run in separate
threads). Checked by hand on 2026-10-19: the sheet opens, macOS accepts the files.
Outside macOS: unavailable.

Files are sent as they are (the RAW for a RAW). Later, with the output profiles of the
"after-development" work: send a version (resized JPEG…).
