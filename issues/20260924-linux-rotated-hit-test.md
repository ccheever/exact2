# Linux hit-testing of a rotated node uses the bounding box

**Status:** Open
**Systems:** Linux host
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1001

Paint stores the axis-aligned bounds of the transformed rectangle (`bbox` in `host/linux/src/paint.rs`). `PaintedBox::contains` tests that rectangle. Projective canvas placements inverse-map the point. A CSS `rotate` does not: its `projective` is `None`, and the hit box is the bounds of the four transformed corners.

Scale and translate of an axis-aligned box stay tight. A 45° rotate does not. A click in a corner of those bounds, outside the diamond, hits the node. The web hit-tests the transformed border box.

Hit-test the rotated quad, the same way the projective path maps back into the node's rectangle. Done when a corner of the axis-aligned bounds that is outside the rotated shape misses, and an unrotated box still hits as it does now.
