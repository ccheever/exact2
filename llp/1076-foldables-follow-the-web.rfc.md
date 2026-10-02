# LLP 1076: Foldables follow the web — `device-posture` and viewport segments

**Type:** RFC
**Status:** Accepted by Charlie, 2026-10-02 (Q1 "for now", Q2 "ok", Q3 "ok rec for now", Q4 "rec prob is fine", Q5 "follow UIKit"), to land once the integrated lane passes the checks, the smokes and the Duo run that day; more testing "once the phone is actually released"
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

## Rulings (Charlie, 2026-10-02)

- **Q1** — land all of it, for now: a lab app is consumer enough.
- **Q2** — by hand, like `smoke deploy`.
- **Q3** — gap default 0; the smoke passes 40.
- **Q4** — the web's: a missing segment takes the row's initial value. The alternatives named (the whole viewport as segment (0, 0); zero) were considered; a page guards its segment lengths with a `when` on the count anyway, and the web is the parity oracle.
- **Q5** — follow UIKit: the division's whole frame, margins included, belongs to no segment.
- **Working set** — LLP 1076 stays out of `llp/current/` (the set is at its cap of 15); it may leave for good once today's tests have run and the kinks they find are worked out. Rotation and Split View wait for a released phone.

## Open questions (for Charlie)

- **Q1 — Is a lab app consumer enough?** LLP 1039 §6 waits for an app that branches on a feature. `duo-lab`'s Fold screen branches on all three; no shipping app does. The alternative is to land only the facts (D2) and the agent's view of them, and hold the `env()` lengths until a real app wants two panes. Proposed: land all of it; the lengths are the smaller half and the parity test (D9's fixture) pins them.
- **Q2 — Where does `smoke duo` run?** By hand, like `smoke deploy`, since it needs Xcode 27.1 beta and a Duo simulator. If the M5 mini gets the beta, the async lane could run it on commits under `host/apple`.
- **Q3 — `prefer segments` gap.** Default 0 (a divider with no width, as a two-screen device joined at the bezel) or 40 (the Duo's band)? Proposed: 0, and the smoke passes 40.
- **Q4 — Resolution when a segment is missing.** D3 takes the row's initial value, as the web does for an undefined `env()` with no fallback. The alternative is to resolve to the whole viewport (as if one segment were segment (0, 0)), which is friendlier but not what a browser does. Proposed: the web's.
- **Q5 — The division's margins.** UIKit's division frame includes 20-pt margins "for interactive content". D5 excludes the whole frame from both segments. Chromium's segments on Android foldables exclude only the hinge's physical band. Proposed: UIKit's frame, since UIKit is saying where content should not sit.

## Not doing

Rotation and Split View on the Duo simulator (the 27.1 beta ignores CoreDevice orientation; LLP 1008); a hinge-angle fact; a posture-driven engine policy (an app chooses, as with reduced motion); container queries (LLP 1039 D5); `env(keyboard-inset-*)` (LLP 1008 §9, still not built).

## As built (core half: D1–D7, D10; 2026-10-02, lane/duo-core)

**Kernel (D3).** `Dimension::Segment(SegmentVar, x, y, plus)` in
`kernel/src/style.rs`; the grammar, `Edge`, `Env`, `Rect` and `SegmentVar`
moved to the new `kernel/src/style/env.rs` (style.rs was 1,439 lines).
`env::parse` returns `Ok(None)` for text that is not an `env()` form at all
and `Err(EnvRefusal)` for one that names a kernel variable wrongly, so the
bake refuses by name (D10) through `StyleValueError::BadEnv` — one index,
three, a negative or non-integer one, one past 15, a fallback argument, an
unknown variable — and `contract/lower/src/values.rs` prints the reason.
`Env` grows `cols`, `rows`, `segments: Vec<Rect>` and stops being `Copy`;
`Env::new(t, r, b, l)` is still the four insets (one segment).
`Kernel::set_segments(cols, rows, rects)` refuses a zero count, a count
that is not `cols × rows` (any rect on a 1 × 1 grid), and a non-finite rect
(`LayoutError::InvalidSegments`); `set_env` sets the insets only and keeps
the grid; both re-derive and dirty exactly the `uses_env` nodes; a `reset`
and a rehydration keep both. Wire kinds 8–13 (`SegmentVar` order: width,
height, top, left, bottom, right) carry the kind byte, **the `f32` the other
kinds carry (the points `calc()` adds), then the two index bytes `x`, `y`**
— the RFC's "no float" would have lost the `calc(env(…) ± Npx)` form D3
asks for, so the float stays and the indices follow it; an index past 15 on
the wire is `UnknownDimensionKind`. The schema's `_styles` note now names
kinds 3–13.

Two choices where the RFC left room, both the web's:

- **Edge semantics.** `viewport-segment-bottom` and `-right` are the
  segment's bottom and right edges measured from the viewport's top and
  left — a `DOMRect`'s `bottom`/`right`, which is what Chromium's
  `StyleEnvironmentVariables` sets them to (`segment.bottom()`,
  `segment.right()`) and what CSS-ENV-1 §2.3's two-column example relies on.
  D3's parenthetical ("distance from the viewport's bottom edge") was the
  draft's guess; the parity oracle is Chromium, so the kernel follows it.
  On the Duo at 130°, `env(viewport-segment-right 0 0)` is 455.5 and
  `-left 1 0` is 495.5: the band between them is the fold.
- **Initial value, exactly.** An undefined segment (one segment, or an index
  past the grid) is invalid at computed-value time. `Dimension::resolve`
  gives `Auto` as the stand-in, and `StyleProps::to_taffy` substitutes the
  row's own initial value from the table before lowering — `auto` for a
  width or an inset, **0 for a margin** (CSS's initial, though the row
  admits `auto`; Q4's "auto where the row allows it" would have centred a
  box) — on a clone made only for a style that holds an undefined segment.
  The authored row is untouched, so the next grid resolves it.

**Runner (D2).** `viewport::FIELDS` gains `devicePosture`,
`horizontalViewportSegments`, `verticalViewportSegments`, filled from
`Viewport.fold: Fold { posture: Posture, cols, rows }` (flat by default: the
bake answers `continuous`, 1, 1; `bake-viewport-field` and the JS target's
`facts.rs` learn the names from the same slice). `Runner::set_fold` is
`set_preferences`'s twin through `replace_viewport`; `set_viewport` and
`set_preferences` keep the fold. `viewport::even_segments(w, h, cols, rows,
gap)` is the one even split the Linux agent uses (the Swift and JS hosts
carry the same formula: `Segments.even`, `evenSegments`). `state.device`
reports the three fields.

**ABI (D4).** `exact_segments(rt, posture, cols, rows, count)` on Apple —
the rects as `count × 4` little-endian floats in the input buffer, as
`exact_set_place`'s text travels, since `host/apple` denies `unsafe` and a
`const float *` would need a raw read (`host/apple/src/host.rs::set_segments`: the kernel's grid,
the env readers' dictionaries re-sent in preorder, paragraphs, then the
runner's fold committed into the same batch through `commit_into`, or a
layout when only styles moved; a count past 255 or the kernel's refusal is
an error on the batch and changes nothing). The web's wasm host exports
`exact_segments(posture, cols, rows)` — no rects: the browser resolves the
lengths itself, the runner answers the fields. Linux: `Host::set_segments`,
reached from the presenter's `set_segments`, which keeps the rects for
`layout.env`.

**Apple (D5).** UIKit 27.1 is resolved through the Objective-C runtime
(`host/apple/Sources/ExactKit/IOS/ReservedRegions.swift`), not compiled
against: the default Xcode 27.0 SDK lacks `UIViewReservedRegion` and
`UIHingeInteraction`, and the 27.1 beta ships the same Swift 6.4
(`swiftlang-6.4.0.34.1`), so neither `#if compiler(>=…)` nor
`canImport(UIKit, _version:)` can tell the SDKs apart. `-[UIView
reservedRegionsOfKind:options:]` is called through its IMP with
`+[UIViewReservedRegionKind divisionRegionKind]` and `IncludeInactive`, each
region's `frame`, `margins` and `isActive` read by KVC;
`UIHingeInteraction` is `alloc`/`initWithUpdateHandler:` with a
`@convention(block)` handler, added as a `UIInteraction`. A 27.0 build finds
neither class and reports flat; a 27.1 build on the Duo finds both. In
`ExactViewIOS.fit`, the container's active divisions are moved into the
viewport's frame (and scaled by an agent window override) and
`Segments.split` (pure, in the shared `Segments.swift`) cuts columns with
bands taller than wide and rows with the others, each band with its margins
belonging to no segment (Q5: UIKit's frame); `folded` while any is active.
`exact_segments` goes out when the fold changes, beside the insets; the
hinge interaction's handler calls `setNeedsLayout` (and marks the device as
having a fold once a hinge is reported). `rebooted()` resends the fold
unconditionally on iOS and macOS, so every boot — including a dev reload's
fresh runner — gets `continuous` 1 × 1 at least once, never the bake's
answer by default. `SegmentsTests.swift` holds the split and the even split
without a simulator.

**Web (D6).** `navigation.js` reads `navigator.devicePosture.type` and
`window.viewport.segments` (two or more rects; columns are the distinct
lefts, rows the distinct tops), `foldEnv()` joins `environment()`, and
`glue.js` calls `exact_segments` once after boot and on `resize` and the
posture's `change`. `css.rs` lowers `Dimension::Segment` to CSS-ENV-1's
text untouched. The JS target's `facts.js` answers the three fields from the
same readings and re-answers on the same events.

**Agent (D7).** `layout.env` carries `device-posture`,
`horizontal-viewport-segments`, `vertical-viewport-segments` and
`viewport-segments` on every host (`AgentIOS`, `AgentMac`, `navigation.js`
for both web targets, `host/linux/src/presenter.rs`; the Linux pinned test
and the smoke's env check hold the names). `prefer` gains the `fold` group
on the wire — `{"fold": {"posture", "cols", "rows", "gap"}}` — and the CLI
forms `prefer posture folded|continuous` and `prefer segments <cols>x<rows>
[gap <points>]` (Q3: gap defaults to 0). macOS, Linux and a fold-less iOS
split their viewport evenly; an iOS device that reported a division or a
hinge refuses ("the device decides"); `0x1`, `1x0`, a negative gap and a gap
wider than the viewport are refused by name on every host (D10). On the web
the driver uses Chromium's own overrides — `Emulation.setDevicePostureOverride`
and `Emulation.setDisplayFeaturesOverride`, one feature per divider, `gap`
wide, where the even split puts it — so the page's `navigator.devicePosture`,
`window.viewport.segments` and CSS's `env(viewport-segment-*)` all change
(the parity oracle); a browser whose CDP lacks them gets the glue's
substitute (`preferFold`), which stands in for the facts and `layout.env`
but not for CSS's own resolution, and says so in `navigation.js`. The
reply's `fold{…}` is the four `layout.env` names. `agent-inspect.mjs` prints
`posture P · segments C×R [x,y w×h]…` when the device is not flat.

**The hinge's first report.** `UIHingeInteraction` reports a turn or two
after the view attaches, so a drive's first `prefer` on the Duo arrived
before the device had said it has a fold and was honoured once. The agent
now turns the main run loop for up to 0.3 s until the interaction has
reported (`presenter.hingeReported`), then refuses on a device with a fold.

**Numbers (2026-10-02).**

- *The Duo simulator* (Xcode 27.1 beta, iOS 27.1, `B82DBA04`; this beta has
  the simulator in portrait, so the inner panel is 669 × 951 with the
  division a horizontal band). Caltrain, a non-cover viewport (the panel less
  its 82-pt top and 34-pt bottom insets), `bun scripts/agent.mjs ios layout
  state` after `hinge_helper set <angle>` and `devicectl … hinge-angle`
  reading it back:
  - 180° (`Angle:180.0°`): `viewport 669×835`, `device-posture continuous`,
    1 × 1, `viewport-segments []`; `state.device.devicePosture
    "continuous"`, counts 1 and 1.
  - 130° (`Angle:130.0°`): `viewport 669×835 · posture folded · segments 1×2
    [0,0 669×373.5] [0,413.5 669×421.5]` — the division's 40-pt band from
    373.5 to 413.5 in the viewport (455.5 on the panel, less the 82-pt top
    inset), with its 20-pt margins inside the band; `state.device`
    `"folded"`, 1 and 2.
  - 0° (closed, the cover panel): `viewport 594×432`, `continuous`, 1 × 1.
  - `prefer segments 2x1 gap 40` and `prefer posture continuous` at 0° and at
    130°: refused, `the device decides (it has a fold)`.
  - `devicectl device motion hinge-angle --session-timeout 2` prints the
    angle and then does not exit on this beta; the drive wraps it in
    `timeout`.
- *The iOS 27.0 simulator* (iPhone 18 Pro, the default Xcode): the build
  compiles with the 27.0 SDK (the dynamic route finds neither class),
  `smoke ios`'s insets, viewport-fact and fold checks pass with
  `continuous` 1 × 1; the UIKit XCTests: 122, 0 failures.
- *macOS*: `smoke macos` passes, `layout.env` carrying the four names flat;
  the XCTests (`SegmentsTests` among them): 533 executed, 1 skipped, 0
  failures.
- *Linux*: `smoke linux` passes with `env {…, "device-posture":
  "continuous", "horizontal-viewport-segments": 1,
  "vertical-viewport-segments": 1, "viewport-segments": []}`.
- *Web* (Chrome 154.0.8037.97, the `insets` fixture at 420 × 900): `prefer
  segments 2x1 gap 40` through CDP — `viewport.segments` `[0,0,190,900]`,
  `[230,0,190,900]`; Chromium resolves `env(viewport-segment-right 0 0)` to
  `190px`, `-bottom 0 0` to `900px`, `-left 1 0` to `230px`, `-width 1 0` to
  `190px`, `-top 0 0` to `0px` (the DOMRect edges the kernel uses) — and
  `(horizontal-viewport-segments: 2)` matches. `prefer posture folded`:
  `navigator.devicePosture.type` `folded`, `(device-posture: folded)`
  matches. `prefer segments 2x2 gap 10`: this Chrome allows one display
  feature, so the glue's substitute answers `2×2 [0,0 205×445] [215,0
  205×445] [0,455 205×445] [215,455 205×445]`; `1x1` clears; `2x1 gap 500`
  is refused by name. `smoke web` passes.
- *Web-core sizes* (brotli-11 `app.wasm`, as `metrics.mjs` measures them;
  before at `a74a8489b`, after at this lane): realworld 299.2 → 300.3 KiB
  (ceiling 304), video-player 242.6 → 243.6 (249), caltrain 305.1 → 306.3
  (310). About 1.1 KiB each: the segment grammar sits on the dimension
  decoder every core links, not behind a use. Within the ceilings; a size
  lane could link the grammar by the first `viewport-segment` row an app
  authors.
- *Kernel*: 120 unit and 333 integration tests; runner, contract CLI and
  Linux pinned tests green; `cargo test -p exact-apple` adds one test and
  keeps the 9 content-region and transform-drag failures the base commit
  already has.

**Open after this lane.** Rotation on the Duo (the beta's simulator
decides); `apps/duo-lab`, `contract/corpus/segments.contract` and `smoke
duo` (the sibling lane); linking the segment grammar by use.

## As built (app half, 2026-10-02)

Built on `lane/duo-app` from a74a8489b, before the core half (D2–D7) existed, against this
document's names exactly; the integrator checks them against the core lane.

- **`apps/duo-lab`** (D8), on the `canvas-gallery` template: `app.contract`, `app.json`
  (`com.exact.duolab`, iPhone + iPad, a 951×669 macOS window), `app.ts` (every resource
  generated deterministically: 24 items, 300 image rows over four PNGs copied from
  `interaction-gallery` with their provenance, a ~3,000-word Markdown document, a
  200-row feed), `apple/`, `web/`, `linux/`; workspace members and the conformance list
  in `scripts/async.mjs`. Four `routes` tabs — `fold "/"`, `images "/images"`,
  `reflow "/reflow"`, `combos "/combos"` with the pushed `note "/combos/note"`. The
  Fold screen's shape is `Viewport { width, height, devicePosture,
  horizontalViewportSegments, verticalViewportSegments }` over `exactViewport()`; with
  `horizontalViewportSegments == 2` the list pane is `width="env(viewport-segment-width
  0 0)"` and the detail pane `position="absolute" left="env(viewport-segment-left 1 0)"
  width="env(viewport-segment-width 1 0)" height="100%"`, each inset by the safe areas on
  its own side; otherwise one pane, the list above the detail; `selected` is shared. It
  prints the facts as `fact-posture`, `fact-h`, `fact-v`, `fact-size`; the panes are
  `pane-list` and `pane-detail`, the rows `item-<n>`, the detail `detail-title`; the tabs
  `tab-fold|images|reflow|combos`; `image-list`, `document`, `reflow-scroll`,
  `open-note`, `note-sheet`, `sheet-handle`, `note-title-input`, `note-textarea`,
  `note-dismiss`, `note-back`, `draft-echo`. The root is `viewport-fit="cover"` with
  `interactive-widget="resizes-content"`, so the sheet rides the keyboard.
- **`bun scripts/smoke.mjs duo`** (D9): the dispatch in `scripts/smoke.mjs`, the suite in
  `scripts/smoke-duo.mjs`, the helper at `scripts/duo/hinge_helper.c` (MIT,
  artemnovichkov/hinge, attributed), compiled per run with the selected Xcode's simulator
  SDK, ad-hoc signed, run by `simctl spawn`; the angle read back and both panels captured
  by `devicectl`. Without `EXACT_SIM` naming an `iPhone Duo` on iOS ≥ 27.1 it prints
  `duo: unsupported — …` and exits 0. The matrix is open 180°, book 130°, half 90°,
  closed 0°, never rotated; at each pose the `insets` and `keyboard-bar` fixtures (cover
  root, insets, keyboard, a fold while editing), the four screens (facts, panes on the
  segments from `layout.env`'s `device-posture`, `horizontal-viewport-segments`,
  `vertical-viewport-segments`, `viewport-segments`; a selection, a scroll offset, a
  pushed screen with its sheet and keyboard across a fold) and the two-session host.
  When the open pose does not report the recorded 951×669 inner panel, the panel sizes
  and the segments are checked against what the device reports (see below).
- **`contract/corpus/segments.contract`** and `contract/cli/tests/it/segments.rs`: all
  six `env(viewport-segment-*)` variables on two panes, the facts as text; the test
  builds it and asserts the one-segment stack. The two-segment case
  (`Kernel::set_segments(2, 1, …)`, D3) is written into the test's doc comment for the
  integrator, since the API did not exist here.

**Pending the core lane.** The compiler refuses the three `env()` lengths
(`lower-attr-value`), so duo-lab's three crates do not bake on this base, the segments
test fails at compile, and the sweeps that compile every app and corpus file
(`lint::the_element_lint_finds_nothing_in_any_app`,
`fmt::every_corpus_file_and_app_round_trips_through_the_printer`,
`incremental::every_app_plan_updates_incrementally_exactly_as_it_does_in_full`) fail with
the same refusal; the bake will refuse the three fields next. The Fold checks of
`smoke duo` fail with `undefined` for posture and segments. Everything else was verified
with an uncommitted variant of the Fold screen (the three fields dropped, the segment
lengths as `calc(50% ± 20px)`, two panes above 900 points): the three other screens on
the iOS 27.0 iPhone 18 Pro simulator, macOS and the web (JS target), and the Duo matrix.

**The Duo, as found on 2026-10-02.** The same simulator (Xcode 27.1 beta) no longer sits
in the state LLP 1008 §9 recorded: `devicectl` reports `landscapeLeft`, the inner panel
669×871 under an 80-pt strip, the cover 678×386 of a 678×466 screen, and `orientation
set` is accepted and changes nothing, so the recorded 951×669 and the division's
segments could not be asserted. A SpringBoard "Open in Exact2 Go?" prompt (another
session's `openurl`) sat over the inner panel for the first run and held the keyboard; a
SpringBoard restart cleared it. With it gone the insets, keyboard-bar, Images, Reflow
and host checks passed at every pose; a later run saw no software keyboard anywhere,
the Mac's `ConnectHardwareKeyboard` preference being on, and one session stopped
answering `clock` for 120 s. Under the driver's `type`, a `textarea` takes text and
focus but raises no keyboard on either simulator (an `input` does), which is why the
sheet carries a title input beside its textarea.
