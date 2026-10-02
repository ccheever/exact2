# LLP 1076: Foldables follow the web — `device-posture` and viewport segments

**Type:** RFC
**Status:** Draft
**Systems:** Kernel (`Dimension::Segment`, `Env` grows a segment grid, `Kernel::set_segments`, wire kinds 8–13, `env()` parsing in `kernel/src/style.rs`); Runner (`exactViewport` gains `devicePosture`, `horizontalViewportSegments`, `verticalViewportSegments`); Apple host (`exact_segments` beside `exact_insets`; `UIView.reservedRegions(kind: .division)` and `UIHingeInteraction` in `ExactViewIOS.fit`; macOS constant); Web host and JS target (`navigator.devicePosture`, `window.viewport.segments`; `env(viewport-segment-*)` as CSS text); Linux host (constant, set only by `prefer`); Agent API (`layout.env` names; `prefer posture`, `prefer segments`); Contract (nothing new: `env()` text is the kernel's, the fields are a resource's); `apps/duo-lab`; `scripts/smoke.mjs duo`
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-02
**Related:** LLP 1008 §"Scene and display ownership" (the iPhone Duo sweep of 2026-09-30 and the reserved-region evaluation this builds on); LLP 1001 §2 (the environment: `Dimension::Env`, `Kernel::set_env`, `uses_env`); LLP 1006 §2 (`env()` is text the kernel parses); LLP 1007 §2, §4 (the web resolves `env()` itself; the agent's `env` probe); LLP 1039 (viewport facts: one reserved source filled by field name; D5 no container queries; §6 "other media features wait for an app that branches on them" — `duo-lab` is that app); LLP 1061 D5 and LLP 1069.000 D1 (media features are `exactViewport` fields, never a second source); LLP 1069.007 D2 (`prefer`'s forms; agent substitutes); LLP 1012 §1 (`layout` reports `env{…}`); `rules/DEFERRED.md` "A new fact is a form of `prefer`, never a tenth operation". Web: CSS Environment Variables 1 §2.3 (viewport segment variables), Media Queries 5 (`horizontal-viewport-segments`, `vertical-viewport-segments`), Device Posture API (`device-posture`, `navigator.devicePosture`), CSS Viewport 1 §7 (`viewport.segments`), UIKit 27.1 (`UIView.reservedRegions`, `UIHingeInteraction`).

## Summary

A fold is the one piece of iPhone Duo geometry nothing in the kernel can see (LLP 1008, 2026-09-30: every pose and panel switch already flows as a resize and a safe area). The web already has the vocabulary for it, so that is the whole design: the `device-posture` media feature and the viewport segments, as `exactViewport` fields an app branches on and as `env()` lengths the kernel resolves. Nothing native is added — no hinge angle, no "cover display" fact, no posture names beyond the web's two. Apple feeds it from UIKit 27.1's division regions and hinge interaction; the web from the browser's own APIs; macOS and Linux are flat unless an agent `prefer`s otherwise, which is how every host and the five checks exercise it without a Duo. A lab app, `apps/duo-lab`, is the first consumer and the stress fixture; `bun scripts/smoke.mjs duo` drives it on the Duo simulator through every pose.

## Motivation

- The sweep of 2026-09-30 left exactly one gap: a layout that respects the fold. UIKit 27.1 reports the fold as a `division` reserved region (a 40-pt band at the inner panel's centre, with 20-pt margins, active while the hinge is partially open) and the hinge through `UIHingeInteraction` (`closed`, `partiallyOpen`, `fullyOpen`, an angle). The kernel has no row that could carry either.
- LLP 1039 §6 and LLP 1069.000 D4 said a media feature waits for an app that branches on it. Charlie asked for a Duo stress app (2026-10-02); its fold-aware screen is that app, and writing it against the web's vocabulary makes it the spec for the row.
- The web's definitions are settled and small: two postures, two integer counts, six two-index lengths. Inventing anything beside them would reintroduce the disagreeing-default-layers problem CLAUDE.md names.

## Design

### D1 — The vocabulary is the web's, exactly

| Web | Here | Values |
|---|---|---|
| `@media (device-posture: …)` | `exactViewport().devicePosture` | `"continuous"` \| `"folded"` |
| `@media (horizontal-viewport-segments: n)` | `exactViewport().horizontalViewportSegments` | integer ≥ 1 |
| `@media (vertical-viewport-segments: n)` | `exactViewport().verticalViewportSegments` | integer ≥ 1 |
| `env(viewport-segment-width x y)` … `-height`, `-top`, `-left`, `-bottom`, `-right` | the same text in any length-valued style | points, in the layout viewport's coordinates |

`folded` is the posture while the device forms an angle short of flat (the Device Posture API §5: a hinge that is partially open); `continuous` is everything else — flat, closed on a single panel, a device with no fold. Segments exist only when a divider splits the viewport (CSS-ENV-1 §2.3: "only defined when there are at least two such segments"); a viewport with one segment reports `1` and `1` and no segment lengths. The gap between segments (the fold band with its margins) belongs to no segment. Index `x` counts columns from the left edge, `y` rows from the top, both from 0.

Not added, on purpose: the hinge angle (the web has none; an app that wants motion from it is asking for a sensor, LLP 1016.002's business); a "cover display" or "panel" fact (a closed Duo is `continuous`, one segment, 466 points wide — the app reads the width, as it would on a small phone); `UIHingeStatus` as a value (`closed` and `fullyOpen` are both `continuous`).

### D2 — The facts ride `exactViewport`

Three fields join `runner/src/viewport.rs`'s `FIELDS`: `devicePosture`, `horizontalViewportSegments`, `verticalViewportSegments`. They change through the same `replace_viewport` path as `width` and `height`, so every resource over the source recommits in one settlement (LLP 1039 D2, LLP 1005 §6). The bake answers `"continuous"`, `1`, `1`. A `when m.devicePosture == "folded"` or `when m.horizontalViewportSegments == 2` is the media query (LLP 1039 D3; no new construct, LLP 1017 §8's rule). The bake's field check (`bake-viewport-field`) learns the three names; it checks types for none of the fields today (LLP 1061 D5 overstates this; recorded, not fixed here).

### D3 — Segments are `env()` lengths the kernel resolves

`kernel/src/style.rs`'s `parse_env` accepts `env(viewport-segment-<var> <x> <y>)` for the six vars with two non-negative integer indices, alongside `env(safe-area-inset-<edge>)`, and inside the same `calc(env(…) ± <n>px)` form. The dimension is `Dimension::Segment(SegmentVar, u8, u8)` (`x`, `y` ≤ 15 is plenty; refuse beyond). On the wire it is kinds 8–13 (one per var) with two index bytes and no float; the dimension codec's note in `kernel/tables/schema.json` (`_styles`, which today lists only kinds 0–2) names kinds 3–13 so the table stops under-describing the codec. `uses_env` is true for a style holding either kind.

Resolution reads the arena's `Env`, which grows from four insets to `Env { insets, cols, rows, segments: Vec<Rect> }` (row-major, `cols × rows` rects in layout-viewport points). A segment var whose `(x, y)` names a segment resolves to that rect's width, height, top, left, bottom (distance from the viewport's bottom edge) or right (from its right edge), as CSS-ENV-1 defines them. One that names no segment — always, when there is one segment — takes the row's initial value, as an `env()` with no fallback does on the web (invalid at computed-value time): `auto` where the row allows it, otherwise 0. The no-fallback form matches `safe-area-inset-*` here (LLP 1001 §2 refuses the fallback argument; unchanged).

`Kernel::set_segments(cols, rows, rects)` is `set_env`'s twin: it refuses non-finite rects and a count that is not `cols × rows`, restyles and dirties only the `uses_env` nodes, and `reset` keeps it. `set_env` keeps its four-float signature.

### D4 — One ABI entry beside `exact_insets`

Apple: `uint32_t exact_segments(ExactRuntime rt, uint32_t posture, uint32_t cols, uint32_t rows, const float *rects, uint32_t count)` (`posture` 0 continuous, 1 folded; `rects` is `count × 4` floats `x y w h`). The host entry sets the kernel's segments and the runner's three fields in one batch, as `exact_resize` sets the viewport and preferences together (LLP 1039 §4), then lays out. The web's wasm host exports the same under the same name for `glue.js`; the JS target answers the fields from `facts.js` and lowers the `env()` text untouched (D6). Linux: `Host::segments(…)`, reached only from the agent.

### D5 — Apple feeds it from UIKit 27.1

In `ExactViewIOS.fit`, on iOS 27.1 or later, after the viewport frame is known: the view's active `division` reserved regions (`reservedRegions(kind: .division)`) are intersected with the viewport's frame; each one splits the viewport along its axis into the rects on either side of the region's full frame (the band plus its margins, so the segments exclude it); `cols × rows` follows from the vertical and horizontal dividers; coordinates are the viewport's (the region's frame converted into `presenter.viewport`'s space). Posture is `folded` while any division is active, `continuous` otherwise. The view adds one `UIHingeInteraction` whose handler calls `setNeedsLayout`, because a hinge moving from flat to a book angle changes the regions' `isActive` without changing any bounds, and nothing else would relayout. `exact_segments` is sent when posture, counts or rects change, next to the `insets` send. Below iOS 27.1, and on macOS (AppKit has no fold API): `continuous`, 1 × 1, sent once at boot so the facts are never stale from the bake.

Occlusion regions (the camera, the status strip) stay what they already are, the safe-area insets; nothing reads them.

### D6 — The web feeds it from the browser

`host/web/navigation.js` reads `navigator.devicePosture.type` (and listens to its `change`) and `window.viewport.segments` (re-read on `resize`), and `glue.js` calls `exact_segments` beside `exact_resize`. The `env(viewport-segment-*)` text lowers to CSS unchanged (`host/web/src/css.rs` already passes `env()` through; the browser resolves it, as it resolves `safe-area-inset-*`). Where the browser lacks the APIs (every browser but Chromium today): `continuous`, 1 × 1, which is also what Chromium reports on a flat display. The parity oracle is the web, so the kernel's resolution in D3 must agree with Chromium on a two-segment viewport; the conformance run's `prefer segments` form (D7) is how that is checked without a foldable.

### D7 — The agent sees and sets it

`layout.env` gains `device-posture`, `horizontal-viewport-segments`, `vertical-viewport-segments` and `viewport-segments` (an array of `[x, y, w, h]` in viewport points, row-major, empty for one segment), on every host (`AgentIOS`, `AgentMac`, `navigation.js`'s `environment()`, `host/linux/src/presenter.rs`), and the Linux pinned test, `smoke.mjs`'s env check and LLP 1012 §1's row say so. Two forms join `prefer` (the ninth operation; a new fact is a form of it, never a tenth): `prefer posture folded|continuous` and `prefer segments <cols>x<rows> [gap <points>]`, which a host without a fold honours by splitting its viewport evenly with the gap (default 0) centred on each divider; a host with a real fold refuses them ("the device decides"), as `prefer` refuses facts the device owns (LLP 1069.007 D2). The web agent sets them through the same substitutes it uses for other media features where CDP offers none.

### D8 — `apps/duo-lab`, the first consumer

A TypeScript-only app on the `canvas-gallery` template (`apple/`, `web/`, `linux/`; workspace members only), four screens behind `routes`:

1. **Fold** — `exactViewport()` as the shape `{ width, height, devicePosture, horizontalViewportSegments, verticalViewportSegments }`. With two horizontal segments: a list pane `width="env(viewport-segment-width 0 0)"` and a detail pane positioned at `left="env(viewport-segment-left 1 0)" width="env(viewport-segment-width 1 0)"`, so the fold band is empty; with one: one pane, the list above the detail. Selection state is shared, so a fold mid-reading keeps the item. The screen prints the three fields and the viewport, so a screenshot is its own evidence.
2. **Images** — a virtualized list of a few hundred image rows (`estimated-item-height`, `object-fit="cover"`) over a handful of bundled PNGs (`assets/` with `provenance.json`, as `interaction-gallery` does), the raster budget's stress: folded mid-scroll, repeatedly.
3. **Reflow** — one long `text … markup="markdown"` document and a dense feed, the text caches' stress from 951 to 466 points and back.
4. **Combinations** — a pushed route with an absolutely positioned sheet (the `interaction-gallery` pattern) holding a `textarea`, so keyboard + sheet + pushed screen + fold is one taps' sequence.

### D9 — `bun scripts/smoke.mjs duo`

A suite in `scripts/smoke.mjs`'s dispatch, with its helper source at `scripts/duo/hinge_helper.c` (MIT, from `artemnovichkov/hinge`, attributed), compiled per run with the selected Xcode's simulator SDK and run inside the simulator by `xcrun simctl spawn` to set the hinge; `xcrun devicectl` reads the angle back and captures each panel. Prerequisites are stated, never assumed: `EXACT_SIM` must name a booted-or-bootable `iPhone Duo` on an iOS ≥ 27.1 runtime under the selected `DEVELOPER_DIR`, else the suite prints `duo: unsupported — …` and exits 0, the smoke's convention for a missing carrier. It drives `duo-lab` (and the `insets` and `keyboard-bar` corpus fixtures) through open 180°, book 130°, half 90° and closed 0°, checking at each: the cover-root and inset invariants `smoke.mjs ios` checks; the Fold screen's two panes exactly on the division's segments (and one pane when flat or closed); `devicePosture` folded at 130° and 90° only; keyboard, sheet, push and scroll state across a fold; the two-session host's panes. It is run by hand like `smoke deploy`, not from the async lane: it needs a beta Xcode and a simulator the lane's machines may not have (Q2).

The same invariants, minus the Duo, run everywhere through `prefer segments 2x1 gap 40` and `prefer posture folded` on a `contract/corpus/segments.contract` fixture: a kernel unit test and a Contract CLI test for D3's parsing and resolution, a Linux pinned test for D7's report, and the web conformance run for D6's parity with Chromium's own resolution.

### D10 — What is refused

`env(viewport-segment-* x)` with one index, more than two, a negative one, a non-integer, or an index above 15; a fallback argument (as for `safe-area-inset-*`); `prefer segments 0x1`, `1x0`, or a gap wider than the viewport. Each is refused by name at the bake (the compiler probes the kernel's parser) or by the agent, never silently zero.

## Open questions (for Charlie)

- **Q1 — Is a lab app consumer enough?** LLP 1039 §6 waits for an app that branches on a feature. `duo-lab`'s Fold screen branches on all three; no shipping app does. The alternative is to land only the facts (D2) and the agent's view of them, and hold the `env()` lengths until a real app wants two panes. Proposed: land all of it; the lengths are the smaller half and the parity test (D9's fixture) pins them.
- **Q2 — Where does `smoke duo` run?** By hand, like `smoke deploy`, since it needs Xcode 27.1 beta and a Duo simulator. If the M5 mini gets the beta, the async lane could run it on commits under `host/apple`.
- **Q3 — `prefer segments` gap.** Default 0 (a divider with no width, as a two-screen device joined at the bezel) or 40 (the Duo's band)? Proposed: 0, and the smoke passes 40.
- **Q4 — Resolution when a segment is missing.** D3 takes the row's initial value, as the web does for an undefined `env()` with no fallback. The alternative is to resolve to the whole viewport (as if one segment were segment (0, 0)), which is friendlier but not what a browser does. Proposed: the web's.
- **Q5 — The division's margins.** UIKit's division frame includes 20-pt margins "for interactive content". D5 excludes the whole frame from both segments. Chromium's segments on Android foldables exclude only the hinge's physical band. Proposed: UIKit's frame, since UIKit is saying where content should not sit.

## Not doing

Rotation and Split View on the Duo simulator (the 27.1 beta ignores CoreDevice orientation; LLP 1008); a hinge-angle fact; a posture-driven engine policy (an app chooses, as with reduced motion); container queries (LLP 1039 D5); `env(keyboard-inset-*)` (LLP 1008 §9, still not built).
