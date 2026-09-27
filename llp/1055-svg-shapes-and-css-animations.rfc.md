# LLP 1055: Inline SVG shapes and CSS animations

**Type:** RFC
**Status:** Accepted (r2, landed; reviews dispositioned in §0). Charlie, 2026-09-26: "why wouldn't we just do a complete SVG implementation here? i think this will be useful and we're using it now" — SVG is admitted complete; this RFC is its first slice and the rest follows as its own stages.
**Systems:** Kernel (`kernel/tables/schema.json` node types, props, style rows; the SVG subtree outside box layout; `kernel/src/svg/` geometry), Motion (`exact-motion`: `@keyframes`, `animation`, two new animatable properties), Contract (`svg` tags and attributes, the `keyframes` declaration, `animation` and its longhands), Plan (a `keyframes` table), Runner (resolving `animation-name`), Web / Apple / Linux hosts (painting shapes, executing animations)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-26 (r1 and r2)
**Implementer:** Claude (Opus 5.5), branch `feat/svg-anim`, 2026-09-26
**Related:** LLP 1002 / 1003 (motion v1: CSS `transition`, two executors, the web as oracle; this RFC extends that design rather than adding a second one), LLP 1001 (kernel; declared deviations), LLP 1050.000 (the fill policy), LLP 1047 (pay for what you use), LLP 1053 (the list-benchmark gaps; the same house process), `rules/NOT-DOING.md` §Motion (the `@keyframes` and Core Animation lines this moves), `~/bench/cryptobench/SPEC.md` (the consumer)

## Summary

The crypto-list benchmark (`~/bench/cryptobench/SPEC.md`) needs each row to draw a live sparkline. The line draws in when the row appears, and a dot at the last point has a ring that breathes forever. A web developer writes that as an inline `<svg>` with a `<polyline>`, `stroke-dasharray` / `stroke-dashoffset` over `pathLength="1"` for the draw-in, and a CSS `@keyframes` pulse. exact2 has neither SVG shapes nor CSS animations today: the `Svg` node type exists but nothing creates it, and `@keyframes` is on the not-doing list.

This RFC adds both, the web way. §0 records how review and building changed it:
- **Shapes:** a minimum SVG 2 subset (`svg`, `g`, `path`, `polyline`, `polygon`, `circle`, `line`, `rect`) with the presentation properties as CSS style rows. Each element is a kernel node. The subtree is skipped by box layout, and its geometry is parsed once in Rust for every host.
- **Animations:** CSS Animations Level 1 (`@keyframes`, the `animation` shorthand and its eight longhands) as a second row beside `transition`, executed the way LLP 1002 executes transitions: the browser on the web, `exact-motion` wherever the host samples per frame (Linux), and Core Animation on Apple, where the compositor already runs a repeating animation without waking the app.

**Recommendation:** build it as specified here. It trades one NOT-DOING line (`@keyframes`) and the Core Animation executor for this benchmark, and deletes the dead `svgSource` prop in exchange (§9).

## 0. Disposition after review, and what was built (r2)

Astra (`gpt-6-astra`, reasoning max) and Grok (served as `grok-4.6-build`, xhigh) reviewed r1 blind (`llp/reviews/1055-svg-shapes-and-css-animations.{astra,grok}.md`). Both said "build with named changes": the architecture (CSS names, element nodes, the browser as oracle, Core Animation for repeating compositor work, start on mount) stands. The rest of this section supersedes r1 where they differ.

**Accepted and built:**

| Finding | Disposition |
|---|---|
| D5 matching by name is insufficient (both) | New lists are walked from the end, each name pairing with the last unmatched old animation of that name; duplicates are separate animations; a reorder restarts nothing (`motion/src/engine/animate.rs`). |
| An unknown name starts no animation (Astra; Grok said the opposite) | CSS Animations 1 §3 and Chrome agree with Astra: no `CSSAnimation` exists for a name without a rule. `Animations::resolve` drops it and the runner journals it; Contract refuses a literal unknown name at compile time (`lower-animation-name`). |
| Transition precedence reversed (Astra) | A running transition wins over an animation on the same property (the CSS cascade's transition origin); the animation shows again when it ends. Tested. |
| Iteration progress, fills, directions, zero duration and zero iterations need the full timing model (both) | `Animation::directed_progress` is Web Animations §4.5–4.8 (phases with boundary times, active time, overall and simple iteration progress, the `simple == 1` endpoint rule, current iteration, direction parity). Seventeen samples are pinned to headless Chrome 154's `getComputedStyle` at a paused `currentTime`: `alternate` over three iterations, a fractional `reverse` with `forwards`, `steps(4, jump-start)` with a negative delay, and the benchmark pulse (`motion/tests/it/animation.rs`, `samples_match_chrome`). All match to 5e-6. |
| Implicit endpoints are per property and live (both) | Tracks are built per property at sample time from the current underlying value, which is the slot's presented value, so a running transition feeds them. |
| D7 lowering too broad (both) | Apple lowers only `opacity`, `stroke-dashoffset` and `r`: `Engine::set_lowered_properties`. Transform animations are sampled by the engine per frame, as transitions are, so no matrix-composition deviation exists. On the web every property is lowered (the browser). |
| Reverse needs time-reversed easings; `alternate-reverse` and odd counts; steps as holds; duplicate offsets (both) | Each direction becomes one explicit CA period: forward, reversed (times mirrored, each cubic `(x1,y1,x2,y2) → (1−x2,1−y2,1−x1,1−y1)`), or both joined over twice the duration, so a fractional `repeatCount` ends mid-period exactly as CSS does. `steps()` becomes hold pairs a hair apart; `linear()` keeps its stops. Equal offsets merge in the parser. |
| Agent clock versus the compositor (both) | Under an agent-owned clock every lowered animation is `speed = 0` at the engine's seek, and `SvgHost.seek` re-seeks on each clock change. One CA quirk was found and handled: a held time at or past a finite end wraps to the next cycle's start, while CSS holds the last frame. The time is clamped just inside. |
| `motion` keeping the display link awake (Astra) | `quiescent()` now separates sampled from lowered work. A lowered infinite pulse leaves `batch.motion` false (asserted in `host/apple/src/svg_tests.rs`). `settle_time` still counts finite lowered animations. |
| Web: `createElementNS`, exact attribute case, unitless numbers, settle with infinite animations, base CSS (both) | The create op carries `ns` for SVG tags. SVG props map to their case-sensitive names. `fill-opacity`, `stroke-opacity`, `stroke-miterlimit`, `stroke-width` and `stroke-dashoffset` are emitted unitless and `r`/`cx`/`cy` as px. `svg` is `display: block` in the page CSS. The glue's seek never calls `finish()` on an infinite animation, and settle skips infinite and author-paused ones (the latter recorded at registration). `@keyframes` rules go into one page-owned stylesheet, once per name, from the navigation-side motion proxy (no after-paint piece needed). |
| Nodes without boxes need a contract (both) | SVG elements keep never-attached Taffy leaves, so publication, export and rebuild see ordinary nodes with zero frames. `sync_children` gives an `svg`, `g` and shapes no layout children (`NodeType::lays_out_children`). On Apple, SVG elements are not views: `create`, `update`, `children`, `destroy` and `present` for them fold into the owning `svg`'s scene. That is the declared exception to "the view tree mirrors the kernel tree", beside inline runs. |
| Hit testing and accessibility of shapes (both) | v1 shapes are decorative: Contract refuses handlers and box attributes on SVG elements (`lower-svg-attr`); the `svg` is the one hit and accessibility box. |
| Content box and borders | Found while building, not by the reviews: CSS's initial `border-width` is `medium` (3 px) and counts only when a style draws a border. The Apple scene now does the same; the bug had shrunk every chart by 3 pt a side. |
| Unit rules for geometry (Astra) | A `px` suffix is accepted only on CSS lengths (keyframe `r`, `translate`, `stroke-dashoffset`) and dash lists. `points`, `d` and `viewBox` take numbers only. |
| Default `preserveAspectRatio`, negative and zero `viewBox` sizes, `pathLength` 0 and negative (both) | `xMidYMid meet`; a negative size invalidates the view box and a zero size renders nothing; `pathLength` ≤ 0 is ignored (scale 1). Unit tests pin the view-box equations. |
| iOS pooling (both) | Rows holding an `svg` stay ineligible for `NodePoolIOS` in v1 (its kind list omits `svg`); `SvgHost.forget` clears scenes and box animations on destroy. Pooling SVG rows is owed (QUEUE). |

**Accepted as declared deviations or limits (not built):**
- **Geometry properties.** `x`, `y`, `rx`, `ry` and `d` are CSS geometry properties in SVG 2 (both reviewers). They stay attributes here, so they are not animatable in v1. Only `cx`, `cy` and `r` are rows.
- **Arcs.** Arcs become cubic quarters, whose length runs 0.014% long against Skia's exact conics (0.004 units on a 10-unit half circle). The parity tolerance covers it.
- **Keyframes are resolved into the row at bind time.** CSS keyframes are live, but a plan's rules never change.
- **`display: none` does not cancel an animation** on native hosts (Astra); the web's does. Hidden nodes paint nothing either way.
- **Inherited animated values do not flow to descendants on native hosts.** An animated `stroke-dashoffset` on a `g` animates the `g`'s own value.
- **A circle with both an animated `r` and dashes** dashes against its static length on Apple.
- **Steps ignore the before flag** at exact boundaries (the existing easing).
- **Intrinsic sizing.** An `svg` with a view box but no width or height is 300 × 150, not the view box's ratio. Authors give both.
- **Capture paths.** Linux retained regions and iOS canvas capture (`Shadow.swift`) copy no SVG scene or presentation state. SVG inside a content region or a captured canvas is unsupported in v1.
- **macOS screenshots.** The agent's default macOS screenshot renders model layers, which is why the frozen ring is missing from it; `screenshot … window` shows presentation.

**D9, corrected:** the collection that exists (fresh ids per mounted key, one viewport of overscan plus velocity lead, bounded retirement, pins) decides when an animation starts: at mount, as on the web. A row kept mounted by overscan, a pin or deferred retirement does not replay when it scrolls back. LLP 1050.000's `complete` and D3 are rulings on an RFC not yet built; nothing here depends on them.

**The NOT-DOING take (both):** deleting the dead `svgSource` is hygiene, not a doing-list take. The consumer (the crypto-list benchmark) is also outside the v1 bar. Charlie's ruling (2026-09-26) admits SVG complete with no take named; `rules/NOT-DOING.md` records it.

**Not accepted:** Grok's suggestion to capability-link the path parser (LLP 1047). The parser is about 350 lines and the web wasm grew by it; it is kernel geometry every host needs, and an `svg`-free plan pays only its code size. That can be measured if it matters.

### As built (hosts)

| Host | Shapes | Animations |
|---|---|---|
| Web | real SVG DOM (`createElementNS`) | the browser, from a page `@keyframes` sheet; seek and settle through WAAPI |
| iOS, macOS | one `svg` scene op per root; `CAShapeLayer` per shape, `CALayer` per `g` (`SvgScene.swift`, shared) | Core Animation for `opacity`, `stroke-dashoffset` (`lineDashPhase`, scaled by length ÷ `pathLength`), `r` (`path`, circles about the origin, layer at `cx, cy`); a box's own `opacity` animation goes to its view's layer; the rest are engine-sampled |
| Linux | `Backend::svg_path` on vello and tiny-skia (dashes, caps, joins, miter limit, even-odd) inside the view-box transform, clipped by the `svg` | `exact-motion`, sampled per frame (`Presented.svg` carries `r` and the dash offset) |

### Parity against Chrome (`apps/sparkline`)

The eight top charts, cropped by their layout boxes (margin 10 pt, compared at 2 px/pt after registering each crop to the Chrome crop within ±20 px; Chrome at 1×):

| Host | Frozen: mean \|Δ\| | pixels off by > 32 | Mid draw-in (clock 300 ms): mean \|Δ\| | off > 32 |
|---|---|---|---|---|
| Linux (CPU painter) | 1.04 / 255 | 0.77% | 0.75 | 0.59% |
| iOS simulator (402 pt, 3×) | 1.98 | 3.14% | 1.42 | 2.32% |
| macOS (window capture, 2×) | 1.96 | 3.65% | 1.39 | 2.69% |

The residue is antialiasing and Chrome's 1× raster; no crop shows a geometric difference.

## 1. What exists today

Verified against origin/main `0fd40388`:
- **`NodeType::Svg` (id 7) and the `svgSource` prop (id 8) are dead.** No Contract tag makes an `Svg`. The web renders one as a `<div>` and sends `svgSource` as `data-svgsource`. Apple makes a plain box, and Linux paints only its background. No Rust, Swift or JavaScript reads `svgSource`.
- **Motion animates four properties (`translate`, `scale`, `rotate`, `opacity`), plus the registered `height` trial,** through the `transition` row (`kernel/tables/schema.json` `_transitions`; `exact_motion::Transitions`). The web emits CSS and uses WAAPI only for springs. Apple and Linux run `exact_motion::Engine` per display-link frame while `!engine.quiescent()`.
- **Virtualized rows are new kernel nodes.** The collection engine (`runner/src/instance/collection/`) creates a row's views when its key enters the window and destroys them when it leaves. It never reuses a view id across keys. iOS recycles the *UIViews* of destroyed rows (`NodePoolIOS.swift`), but only after the kernel has destroyed the old nodes and created new ones.
- **Contract has named style sets** (`style Name` + `class=`) with literal rows. It has no keyframes form.

## 2. Decisions

### D1. The subset: seven element types, SVG 2 names and grammar

| Element | Geometry (attribute or CSS property, as SVG 2 defines it) |
|---|---|
| `svg` | `viewBox` (four numbers), `preserveAspectRatio` (`none` or `x{Min,Mid,Max}Y{Min,Mid,Max}` with optional `meet`/`slice`); size is the box's CSS `width`/`height` |
| `g` | none (a group: opacity and inherited paint) |
| `path` | `d`: the whole path grammar, M/m L/l H/h V/v C/c S/s Q/q T/t A/a Z/z. Arcs become cubics in the shared parser, so no host sees an arc. An error stops the path at the last good segment, as SVG 2 §9.5.4 says. |
| `polyline`, `polygon` | `points` (an odd trailing coordinate is dropped, as in browsers) |
| `circle` | `cx`, `cy`, `r` (**CSS properties** in SVG 2, so animatable) |
| `line` | `x1`, `y1`, `x2`, `y2` |
| `rect` | `x`, `y`, `rx`, `ry` (attributes), and `width`/`height` (the existing CSS rows: SVG 2 makes a rect's size the CSS `width`/`height` properties) |

Every shape takes `pathLength`. Coordinates are unitless user units; a `px` suffix is accepted and equal to a user unit. Percentages and other units are refused by name in v1.

**`ellipse` and `transform` are not in the minimum.** `ellipse` needs two more rows for five minutes of work, so it waits for a consumer. The `transform` attribute and CSS transforms on SVG elements need `transform-box` / `transform-origin` (whose SVG default is `0 0` of the view box, not the element's centre). Both are refused by name, with the reason.

### D2. Presentation properties are CSS style rows, inherited as SVG says

New rows (`schema.json`):

| Row | Codec | Initial | Inherited |
|---|---|---|---|
| `fill` | `paint` (`none` / `currentcolor` / a colour) | black | yes |
| `stroke` | `paint` | `none` | yes |
| `stroke-width` | f32 | 1 | yes |
| `stroke-linecap` | enum `butt round square` | `butt` | yes |
| `stroke-linejoin` | enum `miter round bevel` | `miter` | yes |
| `stroke-miterlimit` | f32 | 4 | yes |
| `stroke-dasharray` | `dasharray` (`none` or a list of numbers; odd lists repeat, as SVG says) | `none` | yes |
| `stroke-dashoffset` | f32 | 0 | yes |
| `fill-opacity`, `stroke-opacity` | f32 | 1 | yes |
| `fill-rule` | enum `nonzero evenodd` | `nonzero` | yes |
| `cx`, `cy`, `r` | f32 | 0 | no |
| `animation` | `animations` (§D5) | none | no |

`opacity` is the existing row. On `g` it is group opacity, as in CSS.

**Which inputs are props and which are rows follows SVG 2.** A property CSS can set (and so animate) is a row. An attribute CSS cannot set (`points`, `d`, `x1`…`y2`, `x`, `y`, `rx`, `ry`, `pathLength`, `viewBox`, `preserveAspectRatio`) is a prop. `d` is a CSS property in Chrome only; it stays an attribute here, so it is not animatable in v1.

`paint` and `dasharray` use the kernel's existing `CssValue` codec shape (the wire carries the CSS text, as `clip-path` and `aspect-ratio` do), so the web host emits exactly what was authored.

### D3. In the kernel, every element is a node; the subtree is outside box layout

**Choice:** one kernel node per element, one node type per element (`Svg`, plus `SvgGroup`, `SvgPath`, `SvgPolyline`, `SvgPolygon`, `SvgCircle`, `SvgLine`, `SvgRect`), rather than one replaced node carrying a display list.

Weighed on the three costs the task named:
- **Layout cost.** `svg` is a replaced box: intrinsic 300×150 like `<video>` and `<canvas>`, sized by its rows, and never sized by its content. Its children never enter Taffy: `sync_children` gives an `Svg` no layout children, as it already gives a `Text` none. A shape node costs an arena slot and its rows, and nothing in the layout pass.
- **Updates when `points` changes every tick.** One `SetProp` on one node. With a display list, the same edit rewrites the list. Either way the host rebuilds one path of 48 points. The node form lets the web host touch one attribute and leave the DOM's own diffing alone.
- **The virtualized list.** Rows are new nodes on every host already (§1), so shapes come and go with their row. Four extra nodes per row at about 20 visible rows is noise beside the row's text.
- **What decides it: animation targets and the oracle.** CSS animates an *element*: the ring's `r` and `opacity`, the line's `stroke-dashoffset`. Nodes give each target a `NodeKey`, the same key the motion engine already uses for transitions. The web oracle needs real SVG elements in the DOM anyway, so a display list would have to be unpacked back into nodes there.

**The kernel enforces the content model.** An `Svg` or `SvgGroup` holds only SVG element children. Shapes hold nothing. An SVG element under anything else is a typed `ApplyError`. Contract refuses the same at compile time, naming the tag. Nested `svg` is refused in v1.

**Geometry is parsed once, in Rust** (`kernel/src/svg/`): path data, points and basic shapes become one path type (move, line, cubic, quad, close). It also computes the path's length (for `pathLength` and dashes) and the `viewBox` → content-box transform (SVG 2 §8.2's equations). Every host is Rust at its boundary, so no host parses SVG itself, and no two hosts can disagree about an arc.

**Declared deviations** (added to LLP 1001's list):
- **`svg` is `display: block`**, not HTML's inline replaced box. The kernel has no inline boxes other than text runs. In the flex rows lists use, the two are identical (flex items are blockified), and the web host emits `display:block` on `svg` as it does on `img` and `video`.
- **`svg` clips by default:** its tag sets `overflow: hidden`, as the UA stylesheet's `svg:not(:root) { overflow: hidden }` does. Authors write `overflow="visible"` to let a pulse ring cross the box's edge.

### D4. How each host paints

- **Web: real inline SVG.** `createElementNS` in the SVG namespace, with geometry props as attributes and rows as CSS declarations. The browser is the renderer and the parity oracle, as for layout.
- **iOS and macOS: `CAShapeLayer`.** The Apple Rust side does not send SVG element nodes to Swift one by one. For each `svg` whose subtree changed, it sends one `svg` op with the whole scene: the view-box transform, and per element its flattened path, paint, stroke parameters, dash pattern (already scaled by `pathLength`) and animations. Swift keeps one `CALayer` per `svg` (with the view-box transform as its `sublayerTransform`), one `CAShapeLayer` per shape and one `CALayer` per `g`. It diffs by element id, so an unchanged layer is untouched and a running animation is not restarted. A circle's path is drawn about the origin, with the layer positioned at (`cx`, `cy`), so a moving pulse and an `r` animation never fight over one path. Implicit layer actions are off. The same Swift serves both platforms (`SvgLayer.swift`), since `CAShapeLayer` is the same on each.
- **Linux: the painter.** The paint walk draws an `svg`'s subtree inside its content box through two new `Backend` calls, `fill_path` and `stroke_path` (vello/kurbo on the GPU, tiny-skia on the CPU oracle; both have dashes, caps, joins and miter limits).

### D5. `@keyframes` and `animation`: CSS Animations Level 1

**Authoring.** A top-level `keyframes` declaration is CSS's `@keyframes`, in Contract's indented form. Selectors are `from`, `to`, percentages, or a comma list of them. Values are literals of the animatable properties (D6), and a keyframe may set `animation-timing-function`:

```
keyframes pulse
  from r=3 opacity=0.5
  to r=9 opacity=0

keyframes draw
  from stroke-dashoffset=1
  to stroke-dashoffset=0
```

A node takes the `animation` shorthand (`animation="pulse 1.2s ease-out 600ms infinite"`) or any of its longhands: `animation-name`, `-duration`, `-timing-function`, `-delay`, `-iteration-count` (a number or `infinite`), `-direction` (`normal reverse alternate alternate-reverse`), `-fill-mode` (`none forwards backwards both`) and `-play-state` (`running paused`). Values are CSS's, including comma lists of up to eight animations. A longhand overrides the shorthand's part whatever the attribute order. If a longhand is computed, it must be a single value, or every other animation part must be literal. The lowering composes one shorthand text per node.

**Representation.** The `animation` row sits beside `transition`, with its type in `exact-motion` (`Animations`). Each entry carries its resolved keyframes, so every host has everything it needs from the node's style. That is what LLP 1002 D1 says for transitions: the style row is the binding.
- The plan gets a `keyframes` table (name, CSS text).
- The runner resolves `animation-name` against it when it sets the row, whether the text was literal or computed.
- An unknown name is an animation with no keyframes. CSS keeps such an animation (its timing runs and it animates nothing), and so does this.
- Keyframe values are literal in v1. A keyframe value that reads a slot needs a per-node keyframes copy, which waits for a consumer.

**Semantics, in `exact-motion`** (CSS Animations 1 §3–4 over the Web Animations timing model):
- An animation starts when its name appears in a node's row, including when the node is created. It is matched to the previous list by name.
- Changing any other longhand re-times the running animation without restarting it.
- Removing the name cancels it. Adding it back restarts it.
- The phases are before, active and after; `fill-mode` decides what shows outside the active interval.
- The iteration progress is `(t − delay) / duration`, directed by `animation-direction`.
- The timing function applies per keyframe interval, not to the whole iteration: a keyframe's own `animation-timing-function` if it has one, else the animation's.
- A property missing from the `0%` or `100%` keyframe takes the node's underlying value there, which is its style value or its running transition's value.
- `paused` holds the current time, and `running` resumes from it. A paused animation with a negative delay shows that phase, which is how a frozen screenshot pins the pulse (`BENCH_FREEZE`).
- While an animation (or its fill) applies to a property, it wins over the property's transition, as in the CSS cascade.

Every value is a closed-form function of clock time, so the seek-not-wait rule of LLP 1002 D3 holds.

### D6. What can animate

Six properties, all numeric with no relayout: the existing four (`translate`, `scale`, `rotate`, `opacity`) plus **`stroke-dashoffset`** and **`r`**. `Property` gains `StrokeDashoffset = 5` and `R = 6`. Their CSS names join `transition-property` too, so `transition="stroke-dashoffset 300ms"` works.

A keyframe that names anything else (colours, `stroke-width`, `cx`, layout rows) is refused at compile time by name. Colour interpolation needs text re-rendering per frame on native, which is not compositor work; it gets its own RFC when a consumer needs it. (The benchmark's price flash is a colour transition; see §7.)

### D7. Who executes: the three executors of LLP 1002, plus Core Animation

- **Web: the browser.** The web host emits the row as CSS `animation: …` and adds each resolved `@keyframes` rule once to a stylesheet it owns, keyed by name. Nothing runs per frame. The agent's `clock` seeks `Animation.currentTime`, as it already does for transitions.
- **Linux: `exact-motion`.** The engine samples every running animation per frame, as it samples transitions. The painter reads `r` and `stroke-dashoffset` from the presented values.
- **Apple: Core Animation** (LLP 1002 §4 left this open "for the Apple host lane"; this RFC takes it for animations only). The engine runs in *lowered* mode there. It tracks each animation's start and state but never enters the per-frame set, so an infinite pulse does not keep the display link or the main thread awake. The Apple Rust side lowers each animation to one `CAKeyframeAnimation` per property:
  - `keyTimes` and values come from the keyframes, with the underlying value filled in where CSS would.
  - Each interval gets a `CAMediaTimingFunction` from its `cubic-bezier` control points. CSS `ease` is `(0.25, 0.1, 0.25, 1)`, not CA's default curve.
  - `steps()` and `linear()` intervals are sampled into linear sub-keyframes by the engine, as springs are for the web.
  - `reverse` mirrors the keyframes, and `alternate` sets `autoreverses` with `repeatCount = iterations / 2`.
  - `infinite` is `.infinity`, the fill mode is `fillMode` (plus `isRemovedOnCompletion = false` when filling forwards), and the delay goes into `beginTime`.
  - Paused is `speed = 0` with `timeOffset` set to the held time.
  - Properties map to key paths: `opacity` → `opacity`, `stroke-dashoffset` → `lineDashPhase` (scaled by the path's length over `pathLength`), `r` → `path` (circles about the origin interpolate exactly), and the transform properties → `transform`, composed per keyframe.
  - The engine is still the source of truth: an agent-owned clock (`session.clock != nil`) pauses every lowered animation at the engine's seek, so screenshots and `clock` are deterministic.

**Parity.** The engine and the browser are held together by fixtures, as LLP 1003 does for transitions: sampled values at fixed times for each direction, fill mode and iteration count, pinned against Chrome's `getComputedStyle` at the same `currentTime`. Core Animation is held to the engine at key times by an XCTest that reads `presentation()` under a paused layer.

### D8. Restarting when a row is reused

CSS starts an animation when an element that has it is inserted, so an inserted row replays its draw-in. exact2's collection makes a new node for every row that enters the window (§1), so the same rule falls out on every host:
- **Web:** new elements, which the browser starts.
- **Linux:** new engine nodes.
- **Apple:** new layers. iOS's view pool recycles a UIView only after the old node is destroyed. Its reset contract (`NodePoolIOS.swift`) is extended to remove all CA animations and the `svg` scene layer, so a recycled view never carries an old row's animation.

A row that stays mounted while its data changes keeps its node, so a live tick redraws the line at once with no replay. The pulse keeps its phase while `cx`/`cy` move it to the new last point, which is what the browser does. On Apple, a geometry change during the draw-in re-lowers the dash animation with the new length and the *same* begin time, so its phase is kept.

### D9. The fill policy (LLP 1050.000)

**Animations do not change what is built or when.**
- An animation's clock starts when its node is created (the web: when the element is inserted). It does not start when the row scrolls into view.
- Under `complete` (D1 of 1050.000), the visible rows are built before they paint, so the draw-in starts as the row appears.
- Rows built ahead in overscan start their draw-in early, exactly as a browser's virtualized list does.
- A row that D3 of 1050.000 defers to rest starts its animation when it is finally built.

The spec's "each time a row comes on screen" is therefore "each time a row is mounted", which is the web's rule. Where the two differ (overscan), the web wins.

### D10. The agent clock and settling

- `clock settle` settles finite motion and ignores infinite iterations: an infinite animation never settles, so it is excluded from `Engine::settle_time` and from the web's settle candidate. Seeking still moves it.
- `Animation.finish()` is never called on an infinite animation (it throws).

### D11. Reduced motion

As LLP 1002 §4: the producer emits `animation: none` when the host reports the preference, as a stylesheet's media query would. The engine has no opinion.

### D12. What this refuses

Named refusals, each with a stable id and the reason:
- **Paint servers and effects:** gradients and patterns (`fill="url(#…)"`), filters, masks, `clip-path` on SVG elements, markers, and `paint-order`.
- **Structure:** `vector-effect`, text in SVG (`text`, `tspan`, `textPath`), `use` / `symbol` / `defs`, `foreignObject`, `image` inside `svg`, nested `svg`, and the `transform` attribute.
- **Animation outside CSS Animations 1:** SMIL (`animate`, `animateTransform`, `set`), `animation-composition`, `animation-timeline` (scroll-driven animations), and animation events (`animationstart` / `animationend` handlers).
- **Values:** `!important` in keyframes, and colour animation (D6).

## 3. The benchmark row, as authored

```
keyframes draw
  from stroke-dashoffset=1
  to stroke-dashoffset=0
keyframes breathe
  from r=3 opacity=0.5
  to r=9 opacity=0

svg width=96 height=32 viewBox="0 0 96 32" overflow="visible" margin-left=12
  polyline points=coin.chart fill="none" stroke=lineColor stroke-width=1.5 stroke-linejoin="round" stroke-linecap="round" pathLength=1 stroke-dasharray="1" animation="draw 600ms ease-out both"
  circle cx=coin.lastX cy=coin.lastY r=3 fill=lineColor
  circle cx=coin.lastX cy=coin.lastY r=3 fill="none" stroke=lineColor stroke-width=1.5 opacity=0 animation="breathe 1200ms ease-out 600ms infinite"
```

The data crate scales the series into the 96×32 box (min at the bottom, max at the top), so no stroke is ever scaled non-uniformly. Under `BENCH_FREEZE=1`, the draw-in is `none` and the ring is `breathe 1200ms ease-out -300ms infinite paused`: a fixed phase that is identical on every host with no clock.

## 4. Costs

- **Rows:** 15 new style rows (bits 100–114 of the mask), 2 enums plus 1 enum, 13 props, and 7 node types.
- **Size:** the web wasm grows by the path parser (arcs included) and the animation codec. Motion's animation evaluator links on the web only where the engine already does (springs); the browser runs CSS animations itself, so under LLP 1047 an app with animations but no springs links no engine.
- **A second Apple executor.** Core Animation now runs lowered animations, while the engine keeps sampling transitions. The fixtures and the paused-layer XCTest are what keep the two honest.
- **The fixture corpus is load-bearing** (LLP 1002 §5, again). A browser value the fixtures do not pin is a divergence nobody sees.

## 5. Plan

1. The schema: node types, rows, props, codecs; delete `svgSource`.
2. `exact-motion`: `Keyframes`, `Animations`, the shorthand parser, sampling, engine integration (lowered and sampled modes), and settle.
3. The kernel: the content model, the layout exclusion, `kernel/src/svg/` (path, points, shapes, length, view box), the `animations` wire codec, and `motion_sync` carrying animations and the two new properties.
4. Contract: tags, attributes, the `keyframes` declaration, longhand composition, and refusals. The plan's `keyframes` table. The runner's name resolution.
5. Hosts:
   - web (SVG DOM, the CSS animation stylesheet, settle);
   - Apple (the `svg` scene op, `SvgLayer.swift`, CA lowering, the agent clock, the pool reset);
   - Linux (`fill_path` / `stroke_path` and the painter walk).
6. A fixture app (`apps/sparkline`) with the benchmark's row, parity-checked against Chrome.
7. The benchmark app (`~/bench/cryptobench/exact-svg`), run on the iPad.

## 6. Questions for Charlie

1. **The NOT-DOING trade (§9):** answered by Charlie (2026-09-26): SVG complete, no take; the subset refusals in this RFC are stages still to build, not permanent refusals.
2. **Apple execution:** Core Animation for animations only (transitions stay on the engine), or should transitions follow once the fixtures exist?
3. **D9:** "starts when mounted" (the web's rule) rather than "starts when first visible". Agreed?

## 7. What the benchmark needs that this does not provide

The price flash ("green at once, then back to the text colour over 400 ms") is a **colour** transition. Motion does not animate colours (D6), and this RFC does not add them. The benchmark's other apps may use one; the exact2 ports need either colour motion (a separate RFC) or a flash without interpolation. This RFC records the gap and does not close it.

## 8. Verification

- Unit tests for the path grammar (every command, arcs, errors), lengths and the view-box equations, against values computed in Chrome (`getTotalLength`, `getScreenCTM`).
- Motion fixtures for animation sampling (directions, fills, iterations, delays, pause, per-keyframe easing), against Chrome's `getComputedStyle` at a seeked `currentTime`.
- Kernel tests for the content model, the layout exclusion and the `motion_sync` of animations.
- Contract tests for the declaration, the attributes, longhand composition and every refusal.
- Hosts:
  - the web's SVG DOM and CSS;
  - Linux's pixels on the CPU oracle;
  - Apple's layer tree and paused-layer presentation in XCTest.
- A pixel parity case: the fixture app's rows (frozen) on iOS, macOS and Linux against Chrome.

## 9. The NOT-DOING trade

**Onto the doing-list:**
- the SVG subset of D1;
- CSS `@keyframes` / `animation` (D5–D7), which moves "`@keyframes`" and "repeat drivers" off §Motion's list;
- Core Animation as the Apple executor for CSS animations, which moves "A Core Animation executor" off the list for animations only.

This unblocks the crypto-list benchmark's SVG port, the "what a web developer writes" case, and any list with inline icons or charts.

**Take:** the `svgSource` prop and its dead `Svg`-as-markup path are deleted (no host reads it, and no tag writes it). Decay and sequence drivers stay out. Colour motion, SMIL and scroll-driven animations stay out, as do every refusal in D12.
