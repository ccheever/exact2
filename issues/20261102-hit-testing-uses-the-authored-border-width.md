# Hit testing insets a clip by the authored border, which layout treats as zero

**Status:** Closed
**Systems:** Runner hit walk (`runner/src/geometry.rs`), kernel border widths (`kernel/src/style/border.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A box with `overflow` other than `visible` and no painted border refuses hits in a 3px band inside every edge. A clipping box smaller than 6×6 refuses every descendant. Layout and paint give that border no area. The pointer falls through to whatever is behind the band.

**Cause.** `kernel_hits` insets the clip with `style.border_width_*` (`runner/src/geometry.rs`, the `clipped` test). Those fields keep the CSS initial `medium`, 3, when `border-style` is `none` or `hidden`. `StyleProps::border_widths` returns 0 for those styles (`kernel/src/style/border.rs`), and `border_widths_in` is what layout uses. `kernel/tests/it/style_dynamic.rs` asserts both facts on a default style. Intersection already zeros `none` and `hidden` in `padding_box`. The hit walk does not. The descendant walk is skipped when `clipped` is true.

**Reproduction.** A 20×20 box with `overflow: hidden` and no border style. `elementFromPoint` / a tap in the outer 3px misses the box and its children. A clipping box of 4×4 misses its children at every point inside it. An authored `border-width` with `border-style: none` insets by the authored width while layout insets by 0.

**Fix.** Inset with `style.border_widths_in`, the used widths layout already applies. A side whose style is `none` or `hidden` contributes 0.

**Acceptance.** The 20×20 box hits across its border box. The 4×4 box hits its descendants. A solid border still insets by the used (device-pixel) width, matching layout. The existing intersection fixtures stay put.

## Resolution

Overflow hit-test clips use resolved painted border widths, the same widths layout uses.

## Verification

`an_unpainted_border_does_not_clip_descendant_hits` passed at both edges of a 4-pixel clip with an authored 30-pixel border and border-style none. The runner geometry suite passed.
