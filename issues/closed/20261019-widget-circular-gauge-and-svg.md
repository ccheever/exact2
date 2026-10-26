# Widget vocabulary lacks a circular gauge and SVG drawing

**Status:** Closed
**Resolution:** The gauge is built: `SnapshotProgressView` (host/apple/Sources/ExactSnapshot/SnapshotLeaves.swift) draws a `progress` as SwiftUI's `Gauge` with `.accessoryCircularCapacity` in the accessoryCircular family and wherever the author rounds it into a circle. SVG in widgets has no blocked consumer; QUEUE.md records its shape for when one comes (LLP 1126 D4's picture leaf, from the scene host/apple/src/svg.rs flattens).
**Systems:** widgets (contract snapshot compiler, ExactSnapshot), kernel schema snapshot admits
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

A widget showing progress as a ring, or a small drawing, has no vocabulary: the lane generated 101 ring images to stand in for one gauge.

## Cause

The snapshot compiler admits `view, box, row, column, text, image, link, main, section, header, footer, article, aside, hr, time, progress` (`contract/cli/src/snapshot.rs:219-245`); `svg` and its shapes are refused. `progress` draws as a linear `ProgressView` (`SnapshotLeaves.swift:177-200`); there is no `Gauge`.

## Fix

In the circular accessory family, and for a `progress` the author rounds (`border-radius="50%"`), draw SwiftUI's circular `Gauge` with the value; admit `svg` with `path`, `circle`, `rect`, `line`, `polyline`, `polygon` and `ellipse` (fill, stroke, stroke-width) in widgets, drawn as SwiftUI paths.

## Partly fixed (2026-10-19)

`progress` draws as SwiftUI's circular capacity gauge in the `accessoryCircular` family and wherever the author rounds it into a circle (`border-radius="50%"`, or each corner at least half the box), tinted by `accent-color` else the text colour; a timer interval in a circle takes the circular progress style. SVG drawing in widgets is not built: it needs the snapshot compiler to admit the `svg` tags and their props, and a SwiftUI path renderer for them.
