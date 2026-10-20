# Module `tri::filter`

Filters decide which images to show, and in what order or grouping.

## Why groups

Images travel as **`Groups`** (`Vec<Vec<MediaFile>>`), not as a flat list. This prepares
for detecting series or bursts (Capture One style) without changing the filter interface
later. Without grouping, everything fits in a single group.

Invariant: **a group is never empty**. Filters that remove images also remove the groups
they have emptied.

## The `Filter` trait

```rust
pub trait Filter {
    fn apply(&self, groups: Groups) -> Groups;
}
```

A filter takes groups and returns others. It can remove images (filter by type, filter on
ratings from [notes.md](notes.md)), reorder them or group them (series detection). Native
filters and those of future plugins implement **this same trait**, with no special
treatment for built-in filters.

The trait is *dyn-compatible*: a chain of filters is stored as `Vec<Box<dyn Filter>>`.

`apply` takes the groups **by value**: a filter can transform them without copying the
`MediaFile`s.

## Utility functions

- **`ungrouped(files)`**: puts the output of `media::scan` into a single group (or no
  group if the list is empty, to respect the invariant).
- **`apply_all(&filters, groups)`**: applies a chain of filters in order.
- **`retain(groups, keep)`**: keeps in each group the images that satisfy `keep`, then
  removes empty groups. It is the common building block of filters that only sort on a
  criterion.

## Native filters

### `KindFilter`

```rust
pub enum KindFilter { All, RawOnly, FlatOnly }   // default: All
```

Filter by file kind. Use case: an SD card in RAW+JPEG must be viewable in full, RAW only,
or JPEG only. `accepts(kind)` gives the decision for a single image (useful on the
interface side, for example for a counter).

## Example

```rust
use tri::filter::{Filter, KindFilter, apply_all, ungrouped};
use tri::media::scan;

let groups = ungrouped(scan("/carte/DCIM", usize::MAX)?);
let raws = KindFilter::RawOnly.apply(groups);

// or a chain of filters:
let filters: Vec<Box<dyn Filter>> = vec![Box::new(KindFilter::FlatOnly)];
let shown = apply_all(&filters, ungrouped(scan("/carte/DCIM", usize::MAX)?));
```
