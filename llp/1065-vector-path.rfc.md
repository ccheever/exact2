# LLP 1065: Vector paths — a `path` node drawn along its pen path

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Kernel (`schema.json` node type 7 `Path`, props 8 `pathData` and 122 `viewBox`, style bits 103–109, codec `paint`; `kernel/src/vector.rs`; `motion::node_targets`), Motion (`Property::StrokeStart`/`StrokeEnd`), Contract (`path`, `d`, `viewBox`, SVG's painting attributes, `stroke-start`/`stroke-end`, keyframes), Web host (`vector.rs`, `css.rs`, one `glue.js` line), Apple host (`vector.rs`, `VectorPath.swift`, `PathViewIOS.swift`, `PathViewMac.swift`), Linux host (`paint/vector.rs`, both painters)
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

**D2 — SVG's attribute names.** `d` (prop `pathData`) and `viewBox` (prop
122) belong to `path` alone, as do `stroke-start`/`stroke-end`
(`lower-attr-tag`). `viewBox` keeps SVG's camel case, not `view-box`: the
web's name, as every attribute here. A literal `d` or `viewBox` is parsed at
compile time and any error refused (`lower-attr-value`, naming the byte);
data from state is drawn up to its first error, as SVG does.

**D3 — SVG's painting properties, inherited.** Rows `fill` (103), `stroke`
(104), `stroke_width` (105), `stroke_linecap` (106), `stroke_linejoin` (107)
are marked `inherited`, as CSS has them, so a `column stroke="#000"` paints
the paths under it on every host (native hosts read the path's computed
style; the web inherits). `fill`/`stroke` use a new codec `paint`: the
keyword-colour codec with keyword `none` (`none`, a colour, or a
`light-dark()` pair); SVG's initials — `fill` black, `stroke` none, width 1,
`butt`, `miter`, miter limit 4 — are the table's defaults. `stroke-width` is
in path units, so it scales with the view box. `currentColor` is refused
(gap).

**D4 — `stroke-start`/`stroke-end`: fractions of the whole path.** Rows 108
and 109 (f32, 0 and 1). They are not CSS — SVG spells the effect with dashes
that restart at every subpath — so the fraction is along the whole path's
length, subpaths in order: subpath 2 begins only when subpath 1 is complete.
Motion adds `Property::StrokeStart = 5`, `StrokeEnd = 6` (scalar, identities 0
and 1). The existing wire grammars grow by the enum alone: transition
property codes 6 and 7, keyframe property codes 5 and 6 (4, height, is still
refused). `transition` and `keyframes` take them like the compositor rows;
the engine's first-seen, interruption and restart rules are unchanged. Only
a `path` hands the engine its strokes (`motion::node_targets`, used by
`motion_sync` and every host's boot adoption), so no other node spends
engine slots on them.

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
attribute names case-sensitively (so pieces are `[data-stroke]`).

**D6 — Apple: a `CAShapeLayer`, UIKit and AppKit alike.** Rust sends the
kernel's normalized data (absolute `M L C Z`) and the parsed view box as
props; Swift builds the `CGPath` once per `d` and fits it into the content
box on every layout, line width scaled with it; `present` ops set
`strokeStart`/`strokeEnd`, which Core Animation already measures across
subpaths in order. The layer backs its view, so UIKit adds no implicit
animation; the engine is the only clock. It clips to the content box (SVG's
viewport). Under a node `scale` of 3 the stroke stays sharp with no extra
work: measured pixel-identical to a build that raised `contentsScale` with
the scale, so none is kept. AppKit's twin backs a flipped view the same
way, with implicit actions disabled around each change. A path is
decorative unless it has `aria-label`, then an image (web, iOS, macOS).

**D7 — Linux: the kernel trims.** `PathData::trimmed(start, end)` returns
the visible stroke (cubics split by arc length, a whole subpath keeps its
closepath); tiny-skia and Vello fill the whole path and stroke the trim with
SVG's caps, joins and miter limit, clipped to the content box.

**D8 — Path data is parsed once, by the kernel.** `vector.rs` implements SVG
2 path data in full (every command, relative forms, implicit repeats,
`S`/`T` reflection, arcs by SVG's implementation notes, split at ≤ 90°) and
normalizes to `M L C Z` (a quadratic is its exact cubic). It measures each
subpath (adaptive subdivision to ~1e-6 units). No host parses SVG.

## Verified

- Unit and integration tests: parser, view box and fit, lengths, trimming
  (`kernel/src/vector/tests.rs`); rows through both ingress paths, the
  motion seam, a transition and keyframes under the clock
  (`kernel/tests/it/vector_path.rs`); compile and refusals
  (`contract/cli/tests/it/vector_path.rs`, fixture
  `contract/corpus/path.contract`); the web's markup and CSS
  (`host/web/tests/it/vector_path.rs`); Linux pixels in pen order through a
  running transition (`host/linux/tests/it/vector_path.rs`).
- Driven with `scripts/agent.mjs --plan` on the corpus fixture (three
  subpaths: loop, wave, underline), frames at 500/800/1100 ms and settled:
  headless Chrome, the iOS simulator, macOS and the Linux host each draw the
  loop, then the wave, then the underline, at matching fractions. A node
  `scale` of 3 on the simulator stays sharp.

## Not done

- On the web a `spring()` on a stroke row is not lowered (the web engine's
  spring lowering animates the four compositor properties); the value jumps.
  Native hosts run it.
- `currentColor`, `fill-rule`, `stroke-miterlimit`, `stroke-dasharray`,
  `preserveAspectRatio`, `vector-effect`, markers, and more than one path per
  node.
- A zero-length subpath draws no dot on the web; Core Animation may.
- On Apple, an ancestor's clip or the node's own `clip-path` applies as for
  any view; the UIKit and AppKit XCTests have no path case yet.
- `clip-path` keeps its own, smaller parser (absolute `M L Q C Z`); it could
  use `vector.rs`.
