# A large backdrop blur allocates an uncapped mirrored pad

**Status:** Closed
**Systems:** SVG raster blur (`host/svg-raster/src/lib.rs`, `host/svg-raster/src/filter.rs`), Linux GPU backdrop (`host/linux/src/gpu/backdrop.rs`), text shadow (`host/linux/src/paint/text_shadow.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A backdrop blur or a text-shadow blur whose radius is large allocates a mirrored rectangle of `ceil(3σ) + 2` pixels on every side, as four `f32` channels, with no ceiling. A full-screen box with a radius of tens of thousands of pixels asks for tens of gigabytes. A non-finite or enormous radius makes the pad saturate and the size multiply overflow. The ink effect path already downsamples a sigma above 12 before it blurs.

**Cause.** `backdrop_blur` sets `pad` from `(3.0 * sigma).ceil() as usize + 2` and builds `(w + 2 * pad, h + 2 * pad)`. `Img::clear` allocates `w * h * 4` floats. Nothing clamps `sigma`. `host/linux/src/gpu/backdrop.rs` passes the CSS sigma times the transform scale straight through. `shadow_pass` does the same with `blur / 2`. `host/linux/src/ink/effects.rs` halves sigma while it is above 12 and only then runs a small kernel. A cast of a non-finite float saturates to `usize::MAX`, and the following addition and multiply wrap in a release build.

**Reproduction.** Paint a box of a few hundred pixels with `backdrop-filter: blur(10000px)` on the CPU or GPU raster path. The process allocates until it is killed, or traps on the size arithmetic. The same radius through the ink effect path downsamples first.

**Fix.** Cap or downsample sigma the way the ink path does before building the pad, and refuse a pad whose byte length does not fit in `usize` without wrapping.

**Acceptance.** A blur of `10000px` on a screen-sized box completes with a bounded allocation. A non-finite radius paints the unblurred backdrop. A radius of a few pixels still matches the current mirror test.

**Resolution.** Large blur radii downsample before convolution, with bounded padding and checked image dimensions. Non-finite radii leave the image unchanged.

**Verification after the fix.** All seven raster tests passed, including uniform images blurred with radius 10000 and f32::MAX, invalid radii, dimension overflow and the existing edge-spread case.
