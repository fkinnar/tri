# `tri`

## Story

Like many tools, `tri` was born of a frustration: the software out there, free
or commercial, does not fit my workflow. So I tried to build the tool I need.

## Workflow

1. Offload one or several memory cards.
2. Rate the photos (mostly with stars) and pick the best ones.
3. Develop them in my favourite software of the moment.
4. Export different versions of the images (social networks, archives…).

## The problem

- Many images are often similar: comparing them is tedious.
- It is not always easy to tell which photos are perfectly sharp.
- Without developing, it is sometimes hard to judge whether an exposure is right.
- Creating and keeping the different versions of an image weighs the flow down.

## Goals

> `tri` is still under development and therefore not perfect. You have a part to
> play by reporting bugs and ideas for improvements.

### Principles

- No catalogue: it works directly on folders, the ratings go into sidecar files
  next to the images.
- Entirely usable from the keyboard.
- Cross-platform: Windows, Linux, macOS.
- Free software (GPL v3.0 licence).

### Offloading

`tri` can offload memory cards to a local volume, without going through a
third-party tool.

### Showing the raw files quickly

A thorough development is not needed to cull: often, the embedded JPEG is
enough.

### Showing relevant data

#### *Histogram*

In almost every program, as on the cameras themselves, the histogram is computed
from the embedded JPEG. `tri` offers a histogram based on the **raw data**,
which gives a better view of blown or buried data in the image.

#### *Overlays*

`tri` can overlay a contrast mask to judge sharpness. It can also show the blown
or buried areas.

### Filtering and sorting

#### *Ratings*

`tri` can rate images (stars, colour labels, flags), and these ratings are
compatible with the main programs on the market.

#### *Filters*

In `tri`, photos can be filtered and sorted by rating (stars, colours, flags),
but blurry or badly exposed photos can also be excluded automatically. Similar
photos can be grouped to compare them and choose more easily which ones to keep.

### And also

#### *Versions*

`tri` can create several versions of the same developed image, useful for
instance to derive several aspect ratios for social networks. No need to create
virtual copies in your development software.

#### *Developments*

`tri` can call the programs that offer a command-line interface, which lets it
generate a development itself: no need to export a JPEG or a TIFF from your
favourite processing software.

#### *Command line*

`tri` can be used from the command line, which automates some tasks
(processing, sidecars, file actions, offloading, export) without going through
the graphical interface.

#### *Keyboard driving*

`tri` can be used from the keyboard, without ever touching the mouse; with
habit, it is far more efficient and faster. It also offers classic shortcuts by
default, and shortcuts inspired by vim.
