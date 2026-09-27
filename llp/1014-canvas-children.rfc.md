# LLP 1014: Canvas children — laid out by the kernel, over the surface everywhere, through it natively

**Type:** RFC
**Status:** Accepted (Charlie Cheever, 2026-08-29, unreviewed; written that day from the probe of the same day and built the same day — §2 steps 1 and 2 landed, LLP 1014.000 transcribes them and names what step 3 still owes; §3's numbers are the probe's, confirmed in the tree. The §5 doing-list take, delegated by Charlie the same day: the three gradient style rows leave the schema — 1009's proposed take for this door, applied here.)
**Systems:** Kernel (`Canvas` holds children), Contract (`canvas` children; corpus), Web host (a wrapper element), Apple host (the overlay, capture, the hook list), GPU module (one export, two trait methods), Linux host (the painter decision this informs)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Related:** LLP 1009 (the canvas — D3 "a leaf with a kernel-owned box" is what this amends; D4 the host owns the frame; "a GPU paint stage for ordinary subtrees" was deliberately not decided there and is decided here in its smallest form), LLP 1001 §1 (the web is the standard; deviations declared), LLP 1002 (one representation, two executors — the shape D2 repeats), LLP 1007 §1 (the web host's elements), LLP 1008 §1 (one `NSView` per node) and §7 (accessibility beyond `testId`), LLP 1010 (scroll nodes — D4's third source), LLP 1012 (`screenshot`, `type`, `tap` — the fixtures' instruments), `rules/DEFERRED.md` §Components and §Runtime. The design this maps, never a dependency: WICG HTML-in-Canvas — `layoutsubtree`, `drawable`, the `paint` event, `drawElementImageToTexture`, `updateElementGeometry` (Chrome origin trial 148–150; Mozilla Negative; WebKit no signal).

## Summary

A **`canvas` may hold children.** The kernel lays them out in the canvas's box
exactly as the browser lays out a `layoutsubtree` canvas's children; every host
composites them **over** the surface; a host that can paint a subtree into a
texture hands it to a surface that asks for it, and the surface may distort the
**pixels but not the boxes**, so hit-testing and accessibility stay the
kernel's. That is the browser's HTML-in-Canvas, natively — probed 2026-08-29:
capture 0.4–0.7 ms and upload 0.2–0.5 ms for a 380×220 pt canvas at 2×, ~5 ms
for the whole document, a hit over the title landing on its text node. The
kernel changed by one predicate; the module's ABI by one call; the presenter did
the work. Five decisions; everything else is the spec's.

## 1. Decisions

**D1 — A `Canvas` holds children, laid out in its box; they never size it.**
`NodeType::can_hold_children` gains `Canvas`; the lowering's refusal lifts with
it (no corpus entry names `canvas`; the corpus gains an accepting case). The
canvas is still sized by its rows — 300×150 by tag default, `width`/`height`
from the author — and its content never influences that (LLP 1009 D3 holds).
The children get the web's `layoutsubtree` semantics — blockified, static
positioning, the canvas as containing block — which the kernel gives for free
because a `Canvas` is `display: block` like any node and Taffy lays out what it
holds. No deviation to declare: a `<div>` inside a `<canvas layoutsubtree>` lays
out the same, content-box and all (the probe's first Contract put
`height="100%"` and `padding=20` on the column and got 260 pt in a 220 pt box —
the kernel agreeing with a bare `<div>`).

**D2 — Over the surface everywhere; through it where the host can paint a
subtree.** Children composite above the surface on every host. That is the
representation, and it is what the web host does by wrapping: a canvas node
becomes a `<div>` holding an absolutely positioned `<canvas>` and the children
in flow (children of a bare `<canvas>` are fallback content — never rendered).
A surface that wants the children says so — `Surface::wants_children(&self) ->
bool`, default `false` — and on a host that can capture a subtree (macOS today;
a Linux painter by construction) receives them as a texture through
`Surface::children(&mut self, Option<&wgpu::TextureView>)`, default no-op,
called when the texture is created or resized (its contents update in place);
the host then stops compositing the overlay itself. On a host that cannot (the
web, until a browser ships HTML-in-Canvas), `children` is never called and the
children stay over the surface — the aurora's title reads the same on the web
minus the refraction. Not chosen: a Contract attribute choosing the mode (the
author would be writing host knowledge into the app); Chrome's origin trial as a
web executor (a flag, a Negative from Mozilla, three spellings of the method in
a year) — it is the **readback oracle** for a fixture when the flag is on (LLP
1009 D1's band), never a path the web host ships.

**D3 — Capture is the presenter's, on its paint cycle, by copy.** On macOS a
canvas's children live in a flipped overlay view above its `CAMetalLayer`;
`cacheDisplay` paints the overlay's subtree into premultiplied RGBA at the
window's scale; the module's ABI gains one export, `gpu_texture(id, width,
height, bytes)`, which writes an `Rgba8Unorm` texture (created or replaced at a
new size) and calls `children(Some(&view))` when the texture is new; the
overlay then composites at alpha 0 — laid out, hit-testable, in the
accessibility hierarchy. The copy path is the decision: its cost is linear in
pixels and 0.3 ms for a card (§3). Zero-copy — an `IOSurface` imported through
`wgpu-hal`, a third audited `unsafe`, double-buffered against the GPU's reads —
and `CARenderer` (the layer tree composited into the texture on the GPU,
reusing AppKit's backing stores) are later, measured trades. They pay only for a
full-screen every-frame subtree (~7 ms of copies at 2× on a 16″ display), where
the paint itself also misses the budget and the answer is a painter host, not a
faster copy.

**D4 — Invalidation is the batch plus a declared list of host repaint
sources.** The browser's `paint` event fires when a drawable's display list
changes; on macOS there is no display list to watch. A canvas captures again:
(a) after any batch whose ops touched a node under its overlay — kernel ops and
motion presentation values; (b) when a node view under the overlay draws itself
(`draw(_:)` marks its canvas, behind a reentrancy guard so the capture's own
drawing does not retrigger it); (c) when a scroll node's clip view under the
overlay changes bounds (LLP 1010); (d) every frame while the first responder is
under the overlay — carets, typing, focus rings — at 0.7 ms/frame (§3).
Captures coalesce per run-loop turn; a canvas whose overlay has no subviews
never captures. Everything not on this list is a declared gap, and the list
grows with every native piece a canvas may hold. A host that paints has the
exact answer because it *is* the display list — one more argument for the wgpu
painter in the Linux decision (`QUEUE.md`). The lesson from Mozilla's objection
to the browser's version — content through a canvas is timed by the JS loop,
losing compositor-thread scrolling and animation — holds here in this form:
scrolling and motion inside a canvas are recaptured, not composited, and a
scroll view through a shader is the case this RFC does not recommend.

**D5 — Geometry is the kernel's: a surface may distort pixels, not boxes.** No
transform in v1. *(Built 2026-08-30 as the later decision described below —
LLP 1014.000 §1b: per-child textures, placements with depth, hit-testing and
accessibility through them, the card stack as the consumer.)* Hit-testing and accessibility come from the kernel's frames
through the alpha-0 overlay — AppKit hit-tests through alpha 0 (verified, §3)
and does not through `isHidden`, which is why alpha. Transforms are a later
decision in the browser's shape (`updateElementGeometry`): the surface returns
a placement per direct child; the canvas view's `hitTest` inverts it and routes
straight to the child, skipping intervening clips and transforms;
`accessibilityFrame` reports it. Arbitrary warps end where the browser's do, in
a hit-test callback into the surface. Per-child textures — the browser's
`drawable` — are that later decision's other half: one texture per direct child
with its kernel frame, when a surface asks; nothing in v1 forecloses it.

**Not decided here, deliberately:** nested canvases (the inner surface as a
module-owned texture composed by the outer — the browser's reverse-tree order;
*built 2026-08-30 by readback, LLP 1014.000 §1a*);
text selection inside a canvas (none on macOS yet, LLP 1008 §7); the web
executor if a browser ships HTML-in-Canvas; the accessibility role of children
under a canvas (§4).

## 2. Sequencing (web first, as the map says)

1. **Kernel, Contract, web:** the predicate; the corpus case; a kernel test that
   a canvas's children are laid out in its box and never size it; the web host's
   wrapper element; the aurora gains its three texts — over the surface on the
   web, the app's first use.
2. **The Apple host:** the overlay and `Capture.bitmap`; `gpu_texture`;
   `wants_children`/`children`; D4's four sources with the guard, the coalescer,
   and the empty-overlay skip; the aurora samples the texture (the probe's
   shader). The smoke (`scripts/smoke.mjs macos`) asserts the hit test through
   the overlay and that a `type` into an `input` inside a canvas repaints
   (D4 d).
3. **Fixtures:** `scripts/agent.mjs macos screenshot` of the canvas region
   against a recorded reference; with Chrome's flag on, the same subtree through
   `drawElementImageToTexture` as the readback oracle.
4. **The Linux painter decision** takes this RFC as input.

The spec (1014.000) transcribes the landing.

## 3. Costs, measured 2026-08-29 (the probe: this machine, Retina 2×, release, smoke mode)

- **Capture** of the aurora's children (760×440 px — three texts, one column):
  **0.37–0.69 ms** across five runs; recapture with paragraphs cached
  0.15–0.48 ms; an empty overlay (the line map's, 760×320) 0.22–0.42 ms — the
  floor for `cacheDisplay`.
- **Upload** of 1,306 KiB: **0.18–0.49 ms** (`write_texture` plus the call
  across the dylib); the first upload ever 8.6 ms (texture creation, the
  staging allocation).
- **The whole document** — 840×6,492 px, 209 views, 131 texts, 758 measured
  runs — **4.1–6.9 ms** in one capture: the upper bound for "everything through
  a shader," and it fits 120 Hz.
- **Every frame** (61 captures at the display link): capture median 0.46 ms,
  upload median 0.24 ms — **~0.7 ms/frame** for the animated case.
- **`gpu_render`** with the texture sample: first frames 2–3 ms (pipelines;
  29 ms once on a cold Metal shader cache), then median 0.5–1.4 ms. A tail of
  3–4 frames at 15–84 ms coincides with the smoke's own `screencapture` and
  200-event wheel storm and is the same with and without every-frame capture;
  **not measured against the unpatched build** (§4).
- **Hit test** at the title's centre through the alpha-0 overlay lands on
  `text aurora-title "Mountain View"`; the node keeps its identifier; its role
  is `AXUnknown` (LLP 1008 §7's gap, not the probe's).
- **Module load** 3–39 ms warm, 389 ms cold on the first run — LLP 1009's
  number.
- **Size:** +373 −26 over nine files (kernel 4, module 98, aurora 162, presenter
  47, GPU bridge 55, smoke 26, app 4). Not measured: a canvas resize, a reload
  with children, a transition on a child (only the every-frame cost), device
  loss, the five checks on the probe tree.

## 4. Open questions

1. ~~The `gpu_render` tail~~ — answered in LLP 1014.000 §3: no tail in steady
   state (p50 0.5 ms, max < 1 ms at 120 Hz); ~1.5 s of frame-long acquires
   after the window appears; and a window that cannot be seen, which the
   presenter now skips.
2. The accessibility role of children under a canvas — `AXUnknown` today for
   every text node, not only these; whether this lane is where LLP 1008 §7's
   gap closes for text.
3. Whether `wants_children` stays on the trait or joins the registry tuple the
   compiler reads — on the trait until a second consumer wants it at compile
   time.

## 5. The DEFERRED trades this RFC owes (Charlie's, before Acceptance)

The rule: name what it unblocks, and take something off the doing-list in the
same PR.

- **§Components `canvas`, widened** (LLP 1009 walked through the door; this
  lets children through it). Unblocks text and controls through a shader
  natively — the aurora's title first. The take proposed in r1 — `Svg` and
  `svgSource` leave `schema.json` — is **withdrawn**: LLP 1009's round-2
  reviews found the same swap frees no v1 capacity (LLP 1001 §9 already puts
  SVG rasterization outside v1) and it was withdrawn there in r3; deleting an
  unused node type is cleanup, not a trade. **The take, named by delegation
  (Charlie, 2026-08-29) and applied: the three gradient style rows —
  `gradient_type`, `gradient_angle`, `gradient_colors` — leave
  `schema.json`**, with their `GradientType` vocabulary. LLP 1009 §5 proposed
  exactly this for the door it opened; it was declared in v1, lowered by no
  host (both skipped the rows by name) and no Contract attribute, and a
  gradient with anything on it is now a canvas surface with children. They
  return when a host earns them.
- Nothing else moves: D3 and D4 are the presenter's, D5 declines a feature.

## Ratification note

Accepted by Charlie Cheever 2026-08-29 on this text, unreviewed, the same
day it was written from the probe and built in the main tree (LLP 1014.000);
the probe's worktree is gone, its diff kept in the session's scratchpad and
its report in the artifact "Text Through the Aurora". §5's take applied the
same day by delegation: the gradient rows are out.
