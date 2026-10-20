# LLP 1137: Length ramps

**Type:** RFC
**Status:** Accepted as built, 2026-10-19.
**Systems:** the schema's `lengthRamps` (`kernel/tables/schema.json`, `kernel/build/codec.rs`), relative lengths (`kernel/src/style/relative.rs`), the web targets' relative CSS (`host/web/src/css.rs`, `host/web-js/src/measures.rs`), the grouped sheet's checkmark metric.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:** LLP 1115 D3 (a text style's size, a ramp over the Dynamic Type body sizes), LLP 1115.000 D10.3 (named lengths), LLP 1116.020 D1 (a carried grouped row's checkmark); QUEUE.md's "iOS: a custom grouped row's checkmark keeps its 17-point size as the text grows".

## Summary

A carried grouped row on iOS (a `delete=` row, a row of three lines) draws its checkmark
from the sheet's fixed metrics, 19 × 17 at every text size, where UIKit's list cell grows
its checkmark accessory with Dynamic Type to 48 × 43.33 at AX5. The growth is no multiple
of the font size: measured on the iOS 27 simulator at each content size category, it
follows the text to AX2, steps back at AX3 and grows again
(`~/audits/exact2-no-tells-2026-10-11/grouped-symbol-probe/ramp.txt`). An `em` length
cannot say that, and `calc()` takes no `em`. A text style's size has the same shape and
is already a ramp over the body sizes (`textStyleBodies`), read at the root font size.

## Decisions

### D1 — a length ramp

The schema's `lengthRamps` names lengths, each a length at every `textStyleBodies` body
size, written `-exact-<name>` on any length row but a font's size and line height, as a
named length is. The kernel resolves one at the end of each commit as it resolves a text
style's size: read at the root font size, interpolated between two body sizes, scaled
with the root outside them. Every reader sees pixels. The web's JS target, which keeps no
kernel, writes the ramp's length at CSS's `medium` in `rem`, as it writes a text style.

### D2 — the checkmark

`accessory-checkmark-width` and `accessory-checkmark-height` are UIKit's list-cell
checkmark (the 13 body, which iOS has no category for, is the 14's scaled). The grouped
sheet's `grouped-checkmark-width` and `-height` metrics name them in the iOS column and in
the web's, whose first look is iOS's whole, and the neutral look's, which is the web's
here; the Mac keeps its fixed size, as AppKit's does not grow with a text size.

## The gate

A carried row's checkmark on the iOS 27 simulator measures 19 × 17.33 at the default text
size and 48 × 43.33 at AX5 (`layout`), as UIKit's; the kernel's unit test reads the ramp at
five root sizes; the six checks.
