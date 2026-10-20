# Module `tri::overlay`: overlays

User's idea (2026-10-19): check in full screen that a photo suits a purpose, for example
that a text or a logo stays readable at a given spot, or that it fits a publication
format (4:5, 9:16). Plus composition grids (thirds, golden ratio, diagonals). The first
"plugin" of `tri`, to put the architecture planned in `CLAUDE.md` to the test.

## Shapes, never pixels

"Pure geometric overlay" family: the engine computes nothing on the image. The `Overlay`
trait returns **vector shapes** (`Shape`: segment, rectangle, text, image) in coordinates
relative to the image, (0, 0) at the top left and (1, 1) at the bottom right. The
interface draws them at its own resolution, zoom included; a future TUI would do what it
can. `shapes(aspect)` receives the image's width / height ratio: a 4:5 frame, a margin or
a reciprocal diagonal depend on it.

## Two kinds of plugins, a single trait

- **Written in Rust** and compiled with `tri`: the built-in grids (`builtin`, `Grid`:
  thirds, golden ratio, golden spiral — requested on 2026-10-19: "some people like it" —,
  diagonals and reciprocal diagonals). The spiral: quarter circles in the successive
  squares of the golden rectangle, approximated by segments, stretched over the image as
  in Lightroom (rotated for a portrait image, otherwise it would be squashed); can be
  flipped in a file (`flip`). Colour and opacity: `[gui.colors] overlay`,
  `[gui.view] overlay_opacity`.
- **Defined by the user, without code**: one TOML file per overlay in the `overlays`
  folder of the data folder (`load_dir`, `user_dir`), read on each key press (a modified
  file shows up immediately). A badly written file is reported (status bar) without
  blocking the others.

No `libloading` and no scripting language: as decided, static traits and a list as long
as no external plugin needs them.

## File format

Complete commented example: `overlay.example.toml` (a test checks that it can be read). A
name (default: the file name) and `[[shape]]` shapes, according to their `kind`:

| `kind` | Keys | Defaults |
|---|---|---|
| `text` | `text`, `anchor`, `size` (height, fraction of the short side), `margin`, `color`, `opacity` | bottom right, 0.04, 0.03, white, 1 |
| `image` | `path` (relative to the file, or `~/…`), `anchor`, `width` (fraction of the width), `margin`, `opacity` | bottom right, 0.15, 0.03, 1 |
| `frame` | `ratio` (`"4:5"`, width:height), `color`, `opacity`, `shade` (veil outside the frame) | white, 1, 0.5 |
| `grid` | `grid` (`thirds`, `golden`, `spiral`, `diagonals`), `color`, `opacity`, `flip` (`x`, `y`, `xy`) | white, 0.6, none |

Anchors: `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`,
`bottom`, `bottom-right`. An unknown key is an error (typo reported).

**Shapes that follow a frame are placed inside that frame**: the logo and text of a 4:5
post land inside the post, not in the outer area that will be cropped, and a grid after a
frame is the grid of the crop.

## In the interface

`o`: next overlay (none, thirds, golden ratio, spiral, diagonals, then the user's), as in
Lightroom; its name in the status bar. `Shift+O` rotates the lines (the eye of the spiral
moves from one corner to another, 4 orientations, kept from one image to the next); a
text or a logo is not flipped. In full screen and in comparison, beneath the focus point
marker. Lines of `[gui.view] overlay_line_width` points, with a dark outline; logos loaded
once (reloaded if the files are reread).
