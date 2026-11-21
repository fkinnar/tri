# Colour palettes (`tri::palette`)

Issue #492 (user's idea, 2026-11-15): build **moodboards** from one's photos and
reference images. Extract each image's palette, find images with close palettes or that
contain a colour, gather them.

The module computes on one image; the cache per source and the background computation
are described below, the interface comes next.

## A palette

`Palette`: an image's main colours (`Swatch`), from the most to the least present, each
with its **share** of the image (shares add up to 1). Colours are in **OKLab** (Björn
Ottosson, 2020): a perceptual space, where equal gaps look roughly equal, which makes
distances and the merging of close colours meaningful. `Swatch::srgb` gives the colour to
write (`#rrggbb`, clipped to sRGB), `Swatch::working` the colour to show like the cached
images, and `Swatch::hue_chroma` its place on a colour wheel (hue, chroma).

Images come in tri's working space (Rec.2020 primaries, sRGB curve, see
[colour.md](colour.md)); OKLab is computed from linear light.

## Extraction (`extract`)

1. A grid of at most `side × side` points is read from the image (96 × 96 by default):
   the palette does not depend on details.
2. **k-means** with `colors` centres (6), `iterations` passes (12). Started **without
   randomness** (same image, same palette, from one run to the next): the point nearest
   the mean, then each time the point halfway, cumulatively, through the squared
   distances to the centres already taken (a deterministic k-means++).
3. Colours closer than `merge` (0.03, barely distinguishable side by side) are merged;
   those under `min_share` (2 %) are removed, shares computed again.

A few milliseconds per image.

## Distance (`distance`)

Cost of moving one palette's shares onto the other's ("Earth Mover's Distance"), in
OKLab units: 0 for two identical palettes, ~0.1 already quite different. Computed by
serving the closest pairs of colours first: a greedy approach, exact in simple cases,
never smaller than the true one, and instant over thousands of palettes (the exact
computation would need a transport solver for a gain invisible when culling).
Symmetric; an empty palette is at infinite distance. Hue and saturation (a, b) count
`chroma_weight` times (2) against lightness (feedback of 2026-11-21: with equal weights, a
brown photo on a dark background looked as close to a purple photo on a dark background
as the other purple ones).

## Similar palettes (`rank_similar`)

The items under the threshold, closest first; those whose palette is not computed yet
kept at the end, those without a possible palette removed. Slider start and end:
`similar_threshold` (0.12), `similar_max` (0.5).

## Share of a colour (`Palette::share_near`)

The share of the image whose colour is within an OKLab radius of a given colour: "has
enough of this blue". Measured like the distances between palettes (hue and saturation
`chroma_weight` times). "Contains this colour" uses `Palette::nearest` instead: the distance of the closest
palette colour, specks under `colour_min_share` (0.02) ignored; tolerance `colour_radius`
(0.1), `colour_radius_max` (0.4).

## Cache

`Palette::to_bytes` / `from_bytes`: computation version (`VERSION`), number of colours,
then L, a, b and share of each. Another version is refused (computed again).

## Sources (user's decision, 2026-11-21)

Three sources, for three uses, chosen in the palette interface:

1. **camera JPEG** (embedded in the RAW, or the JPEG itself): someone shooting JPEG and
   not touching them;
2. **raw LibRaw data**: a "scientific" look at the data, outside any processing;
3. **finished renders** (developments): moodboard research.

One palette per image **and per source** in the cache (`Level::Palette(Source)`,
`palette::Source`: `Camera`, `Raw`, `Developed(software)`), taken from the image it comes
from (`preview::palette_input`): the **thumbnail** (camera JPEG of a RAW, downscaled flat
image), tri's **RAW render**, or the **software's render**. Its key includes that image's:
remade (render settings changed, development edited in darktable), the palette is too.
`SourceKind` is the user's choice (`camera`, `raw`, `developed`); `SourceKind::source_for`
translates it for each image: a flat image has no raw data (its image, the camera
source), a finished render only exists for a development of the list.

`preview::compute_palette` computes (the source image first, if missing),
`preview::palette` reads the cache. Background computation: see
[scheduler.md](scheduler.md).

## Interface

"Palette" section of the right panel (source, band, wheel and colours of the selected
image), searches in the Filter section (similar palettes, colour); see [gui.md](gui.md).
Sources (`SourceKind`): `camera`, `raw`, or a program (`darktable`…) for its
developments; the former `developed` reads back as `darktable`.

## Command line

`tri <folder> --palette [--source camera|raw|darktable|…]`: each image, its `#rrggbb`
colours and their share, stored in the cache as in the interface. Finished renders: only
those already made by the interface (no software launched from the command line).

## Settings `[palette]`

`colors`, `side`, `iterations`, `merge`, `min_share` (see `doc/config.example.en.toml`).

## Tests

Synthetic colour bands: two colours and their shares, same image same palette, close
colours merged and specks removed, ordered and symmetric distances, share of a colour,
binary round trip, hue of red and chroma of grey, sRGB ↔ OKLab round trip.
