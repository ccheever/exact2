# LLP 1065: Vector paths — a `path` node drawn along its pen path

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Kernel (`schema.json` node type 7 `Path`, props 8 `pathData`, 122 `viewBox` and 123 `preserveAspectRatio`, style bits 106–117, codecs `paint` and `dash-array`; `kernel/src/vector.rs`, `clip.rs`, `style/paint.rs`; `motion::node_targets`, `paint_targets`), Motion (`Property::StrokeStart`/`StrokeEnd`/`Fill`/`Stroke`), Contract (`path`, `d`, `viewBox`, `preserveAspectRatio`, SVG's painting attributes, `stroke-start`/`stroke-end`, keyframes), Web host (`vector.rs`, `css.rs`, `element.rs`, spring lowering, one `glue.js` line), Apple host (`vector.rs`, `paint.rs`, `VectorPath.swift`, `PathViewIOS.swift`, `PathViewMac.swift`, `ClipPath.swift`), Linux host (`paint/vector.rs`, `paint_motion.rs`, both painters)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1002/1003 (motion: one representation, two executors), LLP 1057 (keyframes), LLP 1034 (`light-dark()`), LLP 1043.000 (`clip-path`, the other SVG path in the kernel)

## Summary

grnl's first launch draws its wordmark along the pen path — the stroke grows
from nothing to whole over about 1.6 s — then moves the drawn mark into
place. exact2 had no vector primitive: symbols are images, and `clip-path`
only masks. This adds one: a `path` node, SVG's `<svg viewBox><path d>` as a
single box, with SVG's painting properties and two animatable fractions,
`stroke-start` and `stroke-end`, measured along the whole path in subpath
order, so a multi-stroke mark draws like a pen.

```
keyframes draw
  from
    stroke-end=0
  to
    stroke-end=1

path d="M…" viewBox="0 0 1307 840" width=252 height=162
  stroke=textTitle() stroke-width=110 stroke-linecap="round" stroke-linejoin="round" fill="none"
  stroke-end=(drawn ? 1 : 0) transition="stroke-end 1600ms cubic-bezier(.45,0,.55,1)"
// or, from the moment it exists:
path d="M…" viewBox="0 0 1307 840" … animation="draw 1600ms ease-in-out both"
```

## Design

**D1 — One node type, a leaf.** `path` is node type 7, renamed from the
never-implemented `Svg` (its prop 8, `svgSource`, became `pathData`). It
holds no children (`lower-leaf-children`) and has no intrinsic size: a bare
one is a zero-height block, on every host, so an author sizes it (`width`,
`height`, or `aspect-ratio`). *Rejected:* a general `<svg>` subtree — groups,
several shapes, gradients — the product needs one path, and a document model
is a second layout engine.

**D2 — SVG's attribute names.** `d` (prop `pathData`), `viewBox` (prop
122) and `preserveAspectRatio` (prop 123) belong to `path` alone, as do
`stroke-start`/`stroke-end` (`lower-attr-tag`). They keep SVG's camel
case, not `view-box`: the web's name, as every attribute here. A literal
`d`, `viewBox` or `preserveAspectRatio` is parsed at compile time and any
error refused (`lower-attr-value`, naming the byte); data from state is
drawn up to its first error, as SVG does. `preserveAspectRatio` is SVG's
grammar (`none`, or `x{Min,Mid,Max}Y{Min,Mid,Max}` then `meet` or
`slice`; `defer` accepted and ignored), initially `xMidYMid meet`;
`vector::fit` returns a scale per axis, so `none` stretches, and a slice
overflows into the viewport's clip.

**D3 — SVG's painting properties, inherited.** Rows `fill` (106), `stroke`
(107), `stroke_width` (108), `stroke_linecap` (109), `stroke_linejoin` (110),
`fill_rule` (113), `stroke_miterlimit` (114), `stroke_dasharray` (115) and
`stroke_dashoffset` (116) are marked `inherited`, as CSS has them, so a
`column stroke="#000"` paints the paths under it on every host (native hosts
read the path's computed style; the web inherits). `vector_effect` (117) is
not inherited, as in CSS. `fill`/`stroke` use the codec `paint`, the kernel's
`Paint`: `none`, `currentcolor` (any case; it stays a keyword through
inheritance, so each path paints its own `color`), or a colour — a
`light-dark()` pair included; `NodeRef::paint` resolves it. SVG's initials —
`fill` black, `stroke` none, width 1, `nonzero`, `butt`, `miter`, miter
limit 4, no dashes, offset 0, `vector-effect: none` — are the table's
defaults. `stroke-width` and the dashes are in path units, so they scale
with the view box; under `vector-effect: non-scaling-stroke` the stroke is
drawn in the box's pixels (width and dashes unscaled, as Chrome dashes it).
`stroke-dasharray` (codec `dash-array`, `vector::DashArray`) is `none` or
nonnegative numbers separated by spaces or commas; a negative one refuses
the value, an odd list repeats, and a list summing to zero is solid. A
literal `stroke-miterlimit` below 1 is refused at compile time.

**D4 — `stroke-start`/`stroke-end`: fractions of the whole path.** Rows 111
and 112 (f32, 0 and 1). They are not CSS — SVG spells the effect with dashes
that restart at every subpath — so the fraction is along the whole path's
length, subpaths in order: subpath 2 begins only when subpath 1 is complete.
Motion adds `Property::StrokeStart = 15`, `StrokeEnd = 16` (scalar,
identities 0 and 1), after `Layout = 14`, which no row carries; `from_wire`
maps a discriminant, not an index into `Property::ALL`. On the wire:
transition property codes 16 and 17 (15 stays `border-color`), keyframe
property codes 15 and 16. `transition` and `keyframes` take them like the compositor rows,
and a `spring()` drives them as it drives `opacity` (`Property::springs`);
the engine's first-seen, interruption and restart rules are unchanged. Only
a `path` hands the engine its strokes (`motion::node_targets`, used by
`motion_sync` and every host's boot adoption), so no other node spends
engine slots on them.

**D4a — `fill` and `stroke` are paint motion's.** `Property::Fill = 17`
and `Stroke = 18` join LLP 1062's `PAINT` (transition codes 18 and 19,
keyframe codes 17 and 18), colours premultiplied like the others, so a
`transition` and `keyframes` (with `light-dark()` pairs, through palette
functions) move them on every host, and a moving value reaches the paths
that inherit it, as an animating `color` reaches its inheritors. SVG's
`<paint>` interpolates only colour to colour, so `none` is no target
(`Kernel::paint_targets`): owning ends there, the change is discrete, and
the next colour is taken as it is. A keyframe's paint must be a colour.
`currentcolor` targets the computed `color`, and a path whose `fill` or
`stroke` is `currentcolor` paints a moving `color` it inherits.

**D5 — The web: one `<path>` per subpath.** The node is a `div`
(`position: relative` unless positioned) whose content the host builds from
the kernel's numbers — `pathMarkup`, set as `innerHTML` (the one `glue.js`
line; `document.rs` writes it into a built page). Inside: an `<svg>` covering
the content box (`viewBox`, SVG's default `xMidYMid meet`, `overflow:
hidden`), one fill `<path>` of the whole data under the strokes (so a
compound shape's holes fill by nonzero), and one stroke `<path>` per
subpath with `pathLength` set to the kernel's length of it and its place in
the whole as `--a` (start) and `--l` (length), `--L` the total. The rows
become registered numbers, `--exact-stroke-start`/`--exact-stroke-end`
(`@property` in the markup's own `<style>`), so the browser transitions and
keyframes them; each piece's dash is `calc()`/`clamp()` of those. Because
`pathLength` rescales the browser's own measure to the kernel's, pen order is
exact whatever Chrome measures. An empty piece fades (`stroke-opacity`)
rather than leave a zero-length dash's round cap. Two traps, recorded in
`vector.rs`: inside `<svg>`, `<style>` is not raw text to the HTML parser
(so `<number>` is written `\3c number>`), and attribute selectors match SVG
attribute names case-sensitively (so pieces are `[data-stroke]`). The
`<svg>` carries `preserveAspectRatio`; the fill `<path>` its `fill-rule`
(inherited CSS). A zero-length subpath is a `[data-dot]` `<path>` (`M p L
p`, which Chrome caps) at its place `--a`, shown by D9's rule in CSS. A
dashed stroke (computed `stroke-dasharray` that dashes) moves the pieces
into a `<mask>` (white, `maskUnits="userSpaceOnUse"` over the viewport many
times over, since a thin path's bounding box would cut its stroke off) and
strokes the whole data dashed through it (D10). Under `vector-effect:
non-scaling-stroke` Chrome dashes in the box's pixels, where `pathLength`
no longer rescales (measured: a quarter-length dash repeats along the
line), so the pieces drop `pathLength` and scale their numbers by `--k`,
the view box's pixels per unit — `min()` (or `max()` for a slice) of
`100cqw / width` and `100cqh / height` divided by `1px`, the node's `div`
becoming a size container. The markup depends on those two rows, which the
kernel re-sends when they change. A `spring()` on a stroke fraction is
lowered to frames of its registered number (`--exact-stroke-end`), as
LLP 1002 D2 lowers one on `opacity`.

**D6 — Apple: shape layers, UIKit and AppKit alike.** Rust sends the
kernel's normalized data (absolute `M L C Z`), the parsed view box and the
canonical `preserveAspectRatio` as props; Swift builds the `CGPath` once per
`d`. `VectorLayers` (`VectorPath.swift`) holds three `CAShapeLayer`s that
never animate on their own (`StillShapeLayer`: the engine is the only
clock) in the path view's layer, each transformed by the view box's fit
about its own origin (a host's `sublayerTransform` turns about its centre on
UIKit) — so the stroke scales with it, unevenly under `none`, as SVG's
does; a non-scaling stroke's path is mapped into the box instead and its
width and dashes stay points. `shape` fills (its `fillRule`) and strokes;
`present` ops set `strokeStart`/`strokeEnd`, which Core Animation already
measures across subpaths in order. A dashed stroke is `dashes` — the whole
path, `lineDashPattern` and `lineDashPhase` — masked by `reveal`, the
trimmed undashed stroke, since Core Animation dashes a trimmed path from
its trimmed start (measured). Paint motion's `fill`/`stroke` (and a moving
`text_color` for `currentcolor`) restyle the layers. The view clips to the
content box (SVG's viewport). Under a node `scale` of 3 the stroke stays
sharp with no extra work: measured pixel-identical to a build that raised
`contentsScale` with the scale, so none is kept. AppKit's twin backs a
flipped view the same way, with implicit actions disabled around each
change. A path is decorative unless it has `aria-label`, then an image
(web, iOS, macOS).

**D7 — Linux: the kernel trims.** `PathData::trimmed(start, end)` returns
the visible stroke (cubics split by arc length, a whole subpath keeps its
closepath, a zero-length subpath a dot by D9); tiny-skia and Vello fill the
whole path by its `fill-rule` and stroke the trim with SVG's caps, joins and
miter limit, clipped to the content box. A dashed stroke is
`PathData::dashed` (SVG's dashing, restarting at each subpath, zero-length
dashes as dots, at most 65,536 dashes) masked by the trim's stroked outline
(D10). A non-scaling stroke is drawn from `PathData::transformed` — mapped
into the box, then trimmed and dashed there, as Core Animation trims the
path it is handed. A presented `fill`/`stroke` paints over the row.

**D8 — Path data is parsed once, by the kernel.** `vector.rs` implements SVG
2 path data in full (every command, relative forms, implicit repeats,
`S`/`T` reflection, arcs by SVG's implementation notes, split at ≤ 90°) and
normalizes to `M L C Z` (a quadratic is its exact cubic). It measures each
subpath (adaptive subdivision to ~1e-6 units). No host parses SVG.
`clip-path` (LLP 1043.000) parses its `path()` the same way — CSS's
`path([nonzero | evenodd,] "<data>")`, refused whole on any error or when it
draws nothing — and carries the normalized commands and the rule to every
host (Apple: `{"rule","commands"}`; the mask's `fillRule` and hit testing
take the rule).

**D9 — A zero-length subpath is a dot.** SVG 2 §13.4.7: `M p Z` or `M p L
p` with round or square caps draws the cap; a lone moveto draws nothing.
Under the trim, a dot at length `a` shows while `start·L ≤ a ≤ end·L` and
`start < end` — Core Animation's `strokeStart`/`strokeEnd` exactly
(measured: a dot at the path's end shows at `strokeEnd = 1`, not at 0.999,
and not at `strokeStart = 1`; with no length at all, any non-empty window
shows it). The web and Linux follow that rule.

**D10 — The trim reveals the dashes.** SVG lays dashes from each
subpath's start (§13.5.5); `stroke-start`/`stroke-end` are not SVG, so a
dashed, trimmed stroke is defined as the dashed stroke shown where the
trimmed, undashed stroke (same width, caps and joins) would paint — a
mask, on every host. The dashes never move as the trim animates; a cut dash
ends in the stroke's cap. *Rejected:* dashing the trimmed path (Core
Animation's own behaviour): the dashes crawl as `stroke-start` moves.

## Verified

- Unit and integration tests: parser, view box and fit (every alignment,
  `slice`, `none`), lengths, trimming and dots, dashes, dash arrays,
  transformed paths (`kernel/src/vector/tests.rs`); `clip-path`'s full
  grammar and fill rule (`kernel/src/clip.rs`); rows through both ingress
  paths, the motion seam, a transition, keyframes and a spring under the
  clock, `fill`/`stroke` paint targets (`kernel/tests/it/vector_path.rs`);
  compile and refusals, paint keyframes
  (`contract/cli/tests/it/vector_path.rs`, fixture
  `contract/corpus/path.contract`); the web's markup, CSS and spring
  lowering (`host/web/tests/it/vector_path.rs`); Linux pixels in pen order
  through a running transition, dots, revealed dashes, `fill-rule`,
  `preserveAspectRatio="none"`, a `fill` transition, a non-scaling stroke
  and an even-odd relative `clip-path` (`host/linux/tests/it/vector_path.rs`);
  Apple's batch for moving `fill`/`stroke` and their inheritors
  (`host/apple/tests/it/paint.rs`); the UIKit and AppKit path views — fit,
  trim, aspect, non-scaling, paint rows and moving paint, dashes and their
  mask, dots, an even-odd clip (`PathViewIOSTests`, `PathViewMacTests`).
- Driven with `scripts/agent.mjs --plan` on the corpus fixture (three
  subpaths: loop, wave, underline), frames at 500/800/1100 ms and settled:
  headless Chrome, the iOS simulator, macOS and the Linux host each draw the
  loop, then the wave, then the underline, at matching fractions. A node
  `scale` of 3 on the simulator stays sharp.

## Not done

- Markers (`marker-start`/`-mid`/`-end`) need `<marker>` content — a shape
  with its own view box, `refX`/`refY`, `markerWidth`/`Height`,
  `markerUnits` and `orient` — which a leaf `path` cannot hold (D1). An
  estimate: a file-level `marker` declaration like `keyframes` (syntax,
  lowering, a row codec), kernel vertex and tangent placement, and three
  hosts drawing instances — about 1,000 lines. Awaiting a decision.
- More than one path per node, each with its own paint, in one view box
  (SVG's several `<path>`s in one `<svg>`): D1 rejected a subtree. Several
  `path`s stacked by CSS (`position: absolute; inset: 0`, the same
  `viewBox`) draw today, each with its own trim. A real group — children in
  the parent's coordinate system, no CSS boxes, perhaps one pen across them
  — touches layout exemption, the web's `innerHTML` markup (which would
  replace child elements) and Apple's subview order: about 600 lines.
  Awaiting a decision.
- On the web, a non-scaling stroke under `preserveAspectRatio="none"` that
  stretches the axes unevenly trims by the smaller scale; Apple and Linux
  trim the path as mapped into the box.
- Linux's Vello painter draws dots, dashes and their reveal, but only the
  tiny-skia painter's pixels are tested.
- On Apple, an ancestor's clip or the node's own `clip-path` applies as for
  any view.
