I'll review the tip commit read-only, starting with the project rules and the previous round's findings so the classification change is checked against what already failed.The review skill would write files and launch a generic reviewer. This request is a read-only check of one commit against the last round, so I’ll read the diff and the layout code directly.The classification lives in `without_viewport_height`. I’ll read the spec and how the host applies the trial height, then walk every `Dimension` form against it.The zeroing rule is stable for the cases the last review named, and it collapses the other shape of the same functions. That collapse blocks landing.

## Blocker

**A height-axis term inside `min`, or in the max slot of `clamp`, becomes 0 and wins.** `kernel/src/style/viewport.rs:105` and `:112-114`, installed as a definite length at `kernel/src/kernel/geometry.rs:156-163`.

`heightless` replaces every `vh`/`svh`/`lvh`/`dvh`/`vmin`/`vmax` term with its added pixels (0 for a bare unit), then runs the comparison. Zero is the identity of `max` and the annihilator of `min`:

- `max(200px, 80vh)` → `max(200, 0)` = 200. Fixed point. The new test is right.
- `clamp(100px, 50vh, 400px)` → `clamp(100, 0, 400)` = 100. Fixed point. The test is right.
- `min(80vh, 300px)` → `min(0, 300)` = **0**.
- `clamp(10vh, 200px, 40vh)` → `clamp(0, 200, 0)` = **0**. The preferred 200 is dropped because a zeroed max sits under it, and `clamp` lets the lower bound win.

On the route, `max-height: min(80vh, 400px)` is a max size of 0. The trial root is a flex column, and its outer size is clamped by that max (`vendor/taffy/src/compute/flexbox.rs:1748-1749`), so the detent is the padding and the rows are gone. A descendant `height: min(80vh, 300px)` is the same: a definite height of 0 contributes 0, and the children are not measured. Bare `max-height: 80vh` still becomes `auto` and would have kept the rows. Adding the px cap is what blanks the sheet. `padding-top: min(5vh, 24px)` is 0 in the trial; at the resulting height the painted padding is the real `min`, so the sheet is short by that padding.

`min-height: min(80vh, 300px)` → 0 does not do this. The minimum does not bind, and the content height is a fixed point.

Fix: give the reduction a parent operator. A height-axis term is +∞ in `min` and in `clamp`'s max slot, 0 in `max` and in `clamp`'s min slot, and 0 in `clamp`'s preferred slot (so the existing test stays 100). Then `min(80vh, 300px)` is 300, `clamp(10vh, 200px, 40vh)` is 200, and `max(200px, 80vh)` stays 200.

## Should-fix

**Inline sizes in a height unit still follow the sheet, and the extent can hunt.** `kernel/src/kernel/geometry.rs:141-149` rewrites only block-axis height, basis, margin, and padding. `width`, `min-width`, `max-width`, and horizontal padding are still resolved by `taffy_style` against `env.viewport_height`. The route's own horizontal padding is then subtracted from the trial width (`geometry.rs:205-208`). `column-width` is resolved the same way for text in the trial (`kernel/src/fragment.rs:200`).

A child `width: 100%; max-width: 50vh; aspect-ratio: 1` in a 390-wide sheet: at 844 the max-width does not bind and the box is 390 square; the viewport becomes 390; the max-width is 195 and the box is 195; then 97.5, and so on down to 0. Each pass publishes a new content height. `padding-left: 30vh` on the route does the same through wrapping. `gap`, `font-size`, and `line-height` cannot be viewport lengths (they are px, rem, em, or a unitless number). `aspect-ratio` itself is a ratio; it only transfers a vh width into a height. `top`/`bottom` on an absolute child do resolve against the sheet and do not change the border box `fit_content_height` returns.

Fix: run the same classification on `width`, `min-width`, `max-width`, horizontal padding, and `column-width`.

**`Dimension::Segment` is never classified.** `kernel/src/style/viewport.rs:130`. `env(viewport-segment-height 0 0)` and `calc(env(viewport-segment-height 0 0) + Npx)` stay live lengths. A sheet resize recomputes segment rects from the viewport (`host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:340-347`). On a phone the segment is undefined and the row falls back to its initial value, so nothing loops. On a fold, `height: env(viewport-segment-height 0 0)` tracks the sheet the way `100vh` did. Comparisons cannot hold a segment (the parser refuses it).

Fix: treat `SegmentVar::Height`, `Top`, and `Bottom` like a height-axis unit. Leave width, left, and right.

## Nit

**A negative height-axis margin is erased.** `kernel/src/style/viewport.rs:105`, applied at `kernel/src/kernel/geometry.rs:168-172`. `-10vh` at a zero basis is 0, so `max(-40px, -10vh)` is `max(-40, 0)` = 0 and the -40px pull is gone. A bare `margin-top: -20vh` is `Dimension::Viewport` → `Auto`, and an auto main margin in this indefinite column contributes 0 (`vendor/taffy/src/compute/flexbox.rs:2329` only distributes positive free space). An authored `margin-top: auto` is left alone, and a negative point margin is left alone. Both of those are right. A negative coefficient should lose a `max` (−∞), and a bare height-axis margin should be 0 points so it cannot absorb free space inside a definite-height descendant.

**`100vmax` is not what the new test locks.** `host/apple/tests/it/fit_content.rs:319`. That height is on the route, and the trial forces the route's height to `auto` afterwards (`geometry.rs:210-213`) whether or not the unit was classified. `max(50vh, 30dvh)` does lock the "nothing but viewport heights → auto" arm, because it is a min-height. Put `100vmax` on a child.

## What holds

`Points`, `Percent`, and `Calc` do not read the viewport and are left alone. `Env` is a safe-area inset, not the sheet height, and is left alone; nested `env()` inside a comparison is resolved with `e.value`. Bare `vh`/`svh`/`lvh`/`dvh` become `auto`. Bare `vw`/`svw`/`lvw`/`dvw` stay, and they follow the sheet's width, which a height-only resize does not change. Bare `vmin` and `vmax` are always treated as height-axis, including when a landscape sheet makes `vmax` the width and when a portrait sheet shorter than it is wide makes `vmax` the width. That cannot oscillate; a child `height: 100vmax` measures as its content. A comparison of only height-axis units, with nothing added, is `auto`. `calc(80vh + 16px)` keeps 16. The extra walk is a style rewrite of the nodes that match, cheaper than the trial layout; the intern mutex clone in `Comparison::expr` is not the cost.

The new tests fail on the parent. At 390×844 the old probe keeps `clamp(100px, 50vh, 400px)` on the 400 plateau and turns `max(200px, 80vh)` into `auto` (44). The basis, margin, and padding test expects 54 while those lengths still resolve against 844. The `100vmax` and `max(50vh, 30dvh)` assertions would pass on the parent.

Yes. The `min` / `clamp` collapse blocks landing.
