# Checking colours

*[Version française](../fr/colour-check.md)*

`tri` manages colours: it reads the profile of each image (sRGB, Adobe RGB, Display P3,
ProPhoto…) and converts to your screen's profile. This sheet is for checking that it is
right **on your machine**, in ten minutes. It matters most if you have a **calibrated
screen** or a wide-gamut screen, especially on **Windows** and **Linux**, where `tri`
itself converts to the screen (on macOS, the system does).

## What you need

- A version of `tri` from **2026-11-12 or later** (nightly, or built from `main`). An
  older version shows every image as sRGB.
- The **charts**: the [`doc/charts`](../charts) folder of the repository. Download it
  whole (or the repository zip) and copy it wherever you like. If you build `tri`, you
  can also make them, 16-bit TIFFs included:
  `cargo run --release --example colour_charts -- <folder>`.
- A **colour-managed reference program**: Photoshop, Lightroom, darktable, GIMP, or a web
  browser (Chrome, Firefox: drop a chart into it). On Windows, the Photos viewer is not
  reliable for this.

## The charts

They are **the same image**, saved in several colour spaces, each with its profile
embedded (plus an sRGB JPEG without a profile). In a colour-managed program, they must
all **look identical**.

From top to bottom:
1. red, green, blue, cyan, magenta, yellow;
2. skin, sky, foliage, orange, purple, mid grey;
3. a grey gradient;
4. gradients from black to red, green and blue;
5. a hue sweep;
6. colours **more vivid than sRGB** (those of a Display P3 screen), next to the sRGB
   red.

The last row is the exception: in the `srgb…` files, these colours are clipped (that is
normal, sRGB does not contain them), and a little in the `adobergb…` files. On a
wide-gamut screen they are more vivid in the other files; on an sRGB screen, nearly the
same everywhere.

## The checks

1. **The charts against each other, in `tri`.** Open the charts folder. Go from one to
   the next, in the grid and then full size (left / right arrows). Rows 1 to 5: nothing
   should change. If one file is duller or more saturated than the others, note which.
2. **`tri` against your reference program.** Open `srgb.jpg` then `prophoto.jpg` in
   both, side by side, on the same screen. The colours should be the same.
3. **The screen profile `tri` found** (Windows, Linux). In the settings (`Ctrl+,`),
   search for "trace" and tick `[log] trace`. Restart `tri`, open the log (command
   palette, `Ctrl+Shift+P` or `:` in the vim profile: "Application log") and look for
   the line **"profil de l'écran : …"** (screen profile). It gives the name of the
   profile in use; "sRGB" means no profile was found. Untick the trace afterwards (the
   log grows fast).
   - On Windows, the profile is the one associated with the screen in "Color
     Management". Move the window to another screen: the line should change.
   - On Linux, it is the profile colord (or `dispwin`) publishes for the **primary
     screen**, under X11 (under Wayland, only if the compositor publishes it).
   - If no profile is found, you can give it by hand: setting `[gui] display_profile`,
     the path of an `.icc` or `.icm` file.

## What to send back

- System, screen, and whether it is calibrated (with which profile).
- The files that do not look like the others in `tri` (check 1).
- `tri` and the reference program: the same or not, and which reference program (check
  2). A photo of the screen helps more than a screenshot: a screenshot records the
  values sent to the screen, not what it shows.
- The "profil de l'écran : …" line (check 3).

Thank you!
