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

## Similar colours (`rank_similar`)

The items under the threshold, closest first; those whose palette is not computed yet
kept at the end, those without a possible palette removed. Slider start and end:
`similar_threshold` (0.12), `similar_max` (0.5).

## Share of a colour (`Palette::share_near`)

The share of the image whose colour is within an OKLab radius of a given colour: "has
enough of this blue". Measured like the distances between palettes (hue and saturation
`chroma_weight` times). "Contains this colour" uses `Palette::nearest` instead: the distance of the closest
palette colour, specks under `colour_min_share` (0.02) ignored; tolerance `colour_radius`
(0.1), `colour_radius_max` (0.4).

**Several colours** (`Palette::colours_cost`, `ColourQuery`, #492): `None` if one of the
colours is further than the tolerance; otherwise, absolute search, the mean of the
distances (a single colour: its distance, the previous order); hierarchical, each colour
weighs `colour_hierarchy` (0.4) times the previous one and its cost is 1 − its share of
the image (`share_near`) plus `colour_proximity` (0.25) × its distance relative to the
tolerance. Measured on 2026-11-23 on 300 photos of a session (`--find`): with the
distance on an equal footing and 0.6, the order barely changed when swapping the colours
(an image with 7 % of the first colour in 5th place); presence first and 0.4 put first
the images where the first colour covers the most.

## Fine coverage (`Coverage`)

Feedback of 2026-11-21 (video): skin in shadow, orange and saturated, was blended into an
average palette colour, much less saturated; picked on the photo, the colour did not find
the photo. Each palette therefore also keeps its sample points counted in small OKLab
cells (L by steps of 0.04; a and b by steps of 0.016, from -0.32 to 0.32). "Contains this
colour" (`Palette::nearest`) measures the distance within which at least
`colour_coverage` of the image lies (0.5 %: a picked highlight is enough) (`Coverage::reach`), and `share_near` the real share
of nearby pixels (`Coverage::share_within`). Measured on a session (131 images): ~1.2 KB
per palette (0.4 to 3 KB). A palette from an earlier version (without coverage) is
computed again.

## Cache

`Palette::to_bytes` / `from_bytes`: computation version (`VERSION`, 2), number of colours,
then L, a, b and share of each, then the coverage (number of points, of cells, then cell
and points as u16). Another version is refused (computed again).

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

"Colours" section of the right panel (source, band, wheel and colours of the selected
image), searches in the Filter section (similar colours, colour); see [gui.md](gui.md).
Sources (`SourceKind`): `camera`, `raw`, or a program (`darktable`…) for its
developments; the former `developed` reads back as `darktable`.

## Stacking by palette

The user's idea (2026-11-21), after a trial on a real session: mixing the palette into
the burst signature added no cut (between neighbours of one session, 0.01 to 0.03). As a
**criterion of its own** ("Stack: by palette (series)", under "Stack automatically",
kept per folder: `similarity::folder_by_palette`), it separates **series** (one light, one
background) rather than bursts. `stack_links`: the distance of neighbouring palettes
divided by `[colours] stack_scale` (0.5, capped at 1) takes the signatures' scale, which
the stacks slider and its computed centre are set on; **without the time** (a series is
one light, not one moment: fully left, everything joins); an unknown palette cuts. Without a chosen palette source, the camera JPEG.

## Command line

`tri <folder> --colours [--source camera|raw|darktable|…]`: each image, its `#rrggbb`
colours and their share, stored in the cache as in the interface. Finished renders: only
those already made by the interface (no software launched from the command line). With
`--find "#d0a080,#6a7a80"` (the most important first) and `--hierarchical` (or `--mean`,
the average colour, `Palette::mean`, compared with the first one): then the
images that contain all these colours, best first, with their cost and the share of each
colour (tolerance: `--threshold`, otherwise `colour_radius`). With `--posterize K --to
FOLDER`: each image in K colours (`palette::posterize`; 1: its average colour), written in
sRGB (`NAME-pK.jpg`).

**Posterize** (`palette::posterize`, #492, 2026-11-23): the image's k main colours (the
k-means of `palette::extract`, factored into `cluster`, without merging close colours or
dropping specks), then each pixel replaced by the closest (OKLab; a pixel already seen is
not converted again); k = 1, the average colour. Returns the image and its colours, pixel
shares decreasing. About 10 ms for a thumbnail.
In **perceived colours** (`posterize_with(…, true)`, experiment): the image in OKLab, a
and b blurred over a square of `perceived_surround` (0.2) of the long side (two passes of
running means, `box_blur`) give the surround's hue; each pixel loses the
`perceived_adaptation` share of it (0.8; the eye adapting to the dominant light), then a
and b are multiplied by a **decreasing** saturation: `perceived_saturation` (1.4) at
`perceived_saturation_colours` (4) colours, the extra varying as (4 / k) ^
`perceived_saturation_falloff` (0.5) — 1.8 for 1 colour, 1.28 for 8, the whole image
(step 0) left as is (feedback of 2026-11-23: removing the dominant light took colour away from
everything else, "infinitely more natural" but dull; then "the more colours, the less
correction"); in the
k-means (weighted, `cluster_weighted`), a point weighs `1 + perceived_salience × gap to its
surround / 0.1`. On the user's photo under violet light, the face comes out orange from 2
colours (pinkish beige in raw values).

**Masses** (`masses`, the user's idea, 2026-11-24: a clear map of the photo, for
composition, without the details): on the posterized image, each (4-connected) patch
smaller than `min_area` (share of the image) takes the label most present around its
border, smallest first (`absorb_small`, two passes); then a majority filter of radius
`smooth` (share of the long side: a running mean per colour, `box_blur`, the pixel takes the
most present) smooths the edges, and patches that became too small again are taken. Shares
are computed again. Before that, the split into colours (`posterize_with`) counts
lightness `posterize_lightness` times against hue (1: OKLab as is; more: a value study,
light areas together), and its k-means start from two points, its own and centres as far
apart as possible (`cluster_best`, `spread_centres`), the better one kept: with one only, a
light shirt stayed stuck with the dark background (local minimum). With lightness weighing
more, the rendered colours stay those of the hue-only split, each mass taking the closest
(`steady_colours`): only the masses' extent follows the slider (feedback of 2026-11-24: the
colour changed).

**Cached perceived palette** (2026-11-24): next to the measured palette, the cache keeps
its perceived-colour version (`Palette::perceived`, `extract_perceived`: the same
computation at step `colors`, then merges and specks as in `extract`, with its coverage).
The image is first reduced to `perceived_side` (512 px): the computation covers every
pixel, and the surround is measured as a share of the image. Both palettes make a single
entry, in the cache and in the search index (`VERSION` 3: earlier palettes are computed
again once); the key also counts the `perceived_*` settings. `Palette::view(perceived)`
gives one or the other. In the interface, the eye of the Colours section applies to
everything: the panel's palette, the large colours, "Colour", "Similar colours", the
binoculars and stacking by palette (redone when it toggles). On the command line,
`--perceived` applies to the printed palettes and `--find`.

## Settings `[colours]`

`colors`, `side`, `iterations`, `merge`, `min_share` (see `doc/config.example.en.toml`).

## Tests

Synthetic colour bands: two colours and their shares, same image same palette, close
colours merged and specks removed, ordered and symmetric distances, share of a colour,
binary round trip, hue of red and chroma of grey, sRGB ↔ OKLab round trip.
