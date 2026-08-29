# LLP 1008: Apple host v1 — what `host/apple` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Apple host, Kernel (layout, text measurement), Runner (seam), Motion (native executor), C ABI, Boot
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Implementer:** Claude (Fable 5), landing 2026-08-29 (this document transcribes the landing)
**Related:** LLP 1007 (the web host whose shape this repeats), LLP 1001 §5–6 (layout is a host call; text measurement is an injected trait object) and §9 (the C ABI waited for its consumer — this is it), LLP 1002 D2/§4 (every host but the web runs `exact-motion`; Core Animation delegation is a measured question), LLP 1003 §4 (the seam), LLP 1000 (the map: web first, then Apple, then Linux), `rules/RULES.md` §Time budgets, `rules/NOT-DOING.md` §Motion (no CA executor yet). Research, never authority, whose lessons this applies: exact1's LLP 0113/0116/0169 (SwiftUI's delivery hop measured), 0223 (the AppKit/UIKit cutover), 0418/0430/0432 (CoreText as the one text engine), 0323 (measurement caching — shelved there, adopted here at its cheap end).

## Summary

`host/apple/` is the second host and the first native one: the runner and
kernel as a static library with a C ABI, driving AppKit. **The view tree
mirrors the kernel tree** (the web host's rule). This is where three things
run for real for the first time: the kernel's own layout (Taffy), text
measurement injected from the platform (CoreText, through a callback the app
registers at boot), and `exact-motion` as the executor. After every commit
the host lays the roots out, emits every parent-relative frame that changed,
seeks the engine to the app's clock, and emits every presentation value that
changed; the presenter (`host/apple/macos`, SwiftPM, AppKit — no SwiftUI)
applies typed batches to one `NSView` per node. Measured on the Caltrain app
in smoke mode, warm: **process start → first frame 56–70 ms**, of which
**45–58 ms is NSApplication and the window** and **8–9 ms is the runner,
layout, and 746 text measurements (399 from cache)**; apply is ~3 ms. Where
this document and the code disagree, the code and its tests are the authority.

## 1. The seams and the batch (`host/apple/src/host.rs`, `batch.rs`)

`Host::boot(plan_bytes, data, measurer, width, height)` decodes the plan,
boots the runner against `Kernel::new(measurer)`, walks the live tree, lays
it out, tells the engine about it (values, no transitions), and emits the
first batch. `dispatch_at(view, event, now_ms)`, `advance(now_ms)`,
`resize(width, height)`, and `tick(now_ms)` emit a batch each. Ops:
`create` (kind, props by their own names, the style dictionary, handler
kinds), `props` (set/clear), `style` (whole dictionary, when changed),
`children` (when the order changed), `destroy`, `roots`, then **`frame`** —
the node's frame in its parent's space, only when it changed (the mirror
compares every node after every layout; a parent that moves under a child
that did not is caught) — **`content`** (a scroll container's content extent,
when changed), and **`present`** (a motion property's presentation value,
each frame the engine changes it). The batch ends with `timers` (the runner
has any) and `motion` (the engine is not quiescent): the presenter runs its
250 ms clock only for the first and its display link only for the second.

The root lays out under `Offer::definite(viewport)`, and a root that is a
block is as tall as its content — so, as in a browser, **the window is a
viewport over a document**: the presenter's window content is an
`NSScrollView` whose document holds the roots, sized to them and never
smaller than the viewport. The first run of the Caltrain app on this host
found `width=100%` + `padding=20` under CSS's `box-sizing: content-box`
overflowing its parent by 40 pt — true on the web too, quietly; the app now
says `boxSizing="border-box"` (a tag attribute added for it) and the host's
`content` op holds it at the viewport's width.

## 2. The style dictionary (`host/apple/src/style.rs`)

Every set row, read through the kernel's generated `StyleProps::get`, keyed
by the row's name: dimensions as points, `"auto"`, or `{"pct":n}`; colors as
`[r,g,b,a]`; enums as their CSS spelling; `vec2` as `[x,y]`; numbers as
numbers. The four motion targets are never in it — a presenter applies their
presentation values from `present` ops — and `transition` is the engine's.
Gradient and grid rows are named as skipped. The presenter carries CSS's
defaults for the rows it paints (`font_size` 16, `font_weight` 400, black
text, no background, no border): only set rows cross, and the kernel's
defaults are CSS's.

## 3. Text: one engine (`host/apple/src/measure.rs`, `macos/…/Text.swift`)

The kernel hands its measurer a paragraph as ordered runs with an offer
(LLP 1001 §6). Here the measurer is `CallbackMeasurer`: the app's C function,
registered at boot, called with the runs flattened into C structs (UTF-8
bytes with lengths, points, `EXACT_MAX_CONTENT`/`EXACT_MIN_CONTENT` for
unconstrained offers). No `unsafe` on the Rust side: calling a safe
`extern "C"` fn pointer is safe Rust, and the structs live for the call.

The Swift side is CoreText and nothing else. A **`Paragraph`** is a
width-specific snapshot — the wrapped `CTLine`s from a `CTTypesetter`, each
line's baseline from the top, the size with the same `ceil` the kernel
receives — cached by `(spec, width)`. The measure callback answers from it,
and `NodeView.draw` paints from it: one `CTLineDraw` per line, baselines
snapped to the point grid, flush by alignment. What was measured is what is
painted, by construction. `line-height: normal` is the font's ascent +
descent + leading; a set line height centers the glyphs in the box; the
first baseline is reported so Taffy's baseline alignment works; `line_clamp`
truncates the last line with `…`; min-content is the widest unbreakable
word. Fonts are cached by (size, weight, italic).

This is exact1's conclusion applied without re-deriving it: measuring with
CoreText for layout while painting with TextKit was a "dual-engine
correctness tax" (0418), CoreText wraps ~5× faster than TextKit per
paragraph, `CATextLayer` and TextKit-only were rejected there for body text,
and a size cache hit ~97% under live resize (0322/0323). Here the numbers
came out the same shape on the first measurement: 746 requests for 128 text
nodes at boot (Taffy asks several times per node across its passes), 399
answered from cache, ~17 µs per miss.

## 4. The C ABI (`host/apple/src/abi.rs`, `include/exact.h`)

LLP 1001 §9 left the C ABI "waiting for the consumer that would make its spec
transcription rather than speculation"; this is that consumer, and the ABI
is the web host's buffer discipline over `extern "C"`: `exact_in(len)`
resizes a host-owned input buffer and returns its address; `exact_out()`
returns the output buffer's; `exact_boot(measure, ctx, width, height)`,
`exact_boot_plan(len, …)`, `exact_dispatch(view, kind, len, now_ms)`,
`exact_advance(now_ms)`, `exact_resize(width, height)`, and `exact_tick(now_ms)`
each return the output's length, a UTF-8 JSON batch. The app never hands the
host a pointer the host did not give out; the one call the other way is the
measure function. All calls on one thread; the bridge is thread-local.
`exact_apple::host!(DataType, PLAN)` instantiates the exports for one app;
`apps/caltrain/apple` is that one line plus the same `build.rs` as the web
crate, producing `libcaltrain_apple.a`. The header is written by hand (nine
functions, three structs); a header generated from `schema.json` — enum
values for rows, node types, props — waits for a consumer that reads
binary batches instead of names, as §9 of LLP 1001 said it should.

## 5. The presenter (`host/apple/macos`)

A SwiftPM package (`Package.swift`, tools 5.9; a `CExact` system-library
target over `exact.h`; `EXACT_LIB_DIR`/`EXACT_LIB` name the archive), AppKit
only. `NodeView` is one flipped, layer-backed `NSView` per node with
`layerContentsRedrawPolicy = .duringViewResize`: it draws its background,
per-side borders, and radius, and for a text node its `Paragraph`; an
`input` node carries an `NSTextField`; a `scroll`/`list` node an
`NSScrollView` whose flipped document view holds the children and takes the
`content` size. Frames are set from `frame` ops; `present` ops set an
affine transform about the bounds' center (translate · rotate · scale) and
`alphaValue`. A press is a mouse-down and -up inside the bounds on a node
with a `press` handler; an input's `controlTextDidChange` is a `change`.
Motion frames come from `NSView.displayLink` while `motion` is true and from
nothing otherwise; the runner's clock is a 250 ms timer while `timers` is
true.

**Scrolling** is LLP 1010's: the window is a viewport over the document,
a scroll container is a `ChainingScrollView` made from the node's
effective `overflow` rows, a wheel it can take it applies itself and one it
cannot chains to the next responder (the web's `overscroll-behavior:
auto`). The AppKit-first fallback this paragraph once described is gone
(a nested `NSScrollView` may move the enclosing view or animate later, so
"did it move?" can double a delta); see LLP 1010 §3–§4 for the rule and
the smoke that holds it.

**The dev loop** landed here too (LLP 1007 §6's shape): `EXACT_DEV_PLAN`
names the plan `host/web/dev.mjs` writes on every save; the app restarts
from it with state carried (`exact_boot_plan`, `Runner::carry`) in ~7 ms.
`node host/apple/build.mjs --run` sets it. `build.mjs` also forces the
Swift relink, since `swift build` does not see the Rust archive change —
a stale link that hid two lanes' changes before it was found. `EXACT_SMOKE=1` prints the boot phases and a summary and exits;
`EXACT_SHOT=<path>` writes a PNG of the window — the run and the picture
`host/apple/smoke.mjs` and a reviewer read.

## 6. Building and measuring (`build.mjs`, `smoke.mjs`, `scripts/metrics.mjs --long`)

`node host/apple/build.mjs [--run]` — `cargo build --release -p
caltrain-apple`, then `swift build -c release` against it. `node
host/apple/smoke.mjs` — launches the app in smoke mode and asserts the
landmarks. `node scripts/metrics.mjs --long` — the short run plus the macOS
host: a warm build, the budget row "touch one line, rebuild that crate"
(touching `host/apple/src/host.rs`), and the boot phases from a warmed-up
smoke run (the first launch of a fresh binary is a cold outlier, exact1's
harness lesson). The long run exists because a macOS build cannot honestly
be promised under 30 s. On 2026-08-29, warm: **build 1.7 s** (cargo 1.2 s,
swift 0.4 s; budget 5 min), **touch one line in `host.rs`, rebuild 1.8 s**
(budget 30 s — a static archive is `ar`, not a link; the link is the Swift
step), **process → first frame 56–76 ms** across runs (budget 100 ms p50):
before boot 40–62 ms, runner + layout 8–11 ms, apply 3 ms. 

**Startup, understood (2026-08-29).** With stamps from exec (`sysctl`
`p_starttime`) to the first `draw`, the honest end to end is **main → first
paint ≈ 190–235 ms** on this machine, of which **ours is ~11 ms** (runner +
layout + 746 measurements + 202 views). The rest was measured against
`host/apple/macos/floor.swift` — an empty AppKit app with the same stamps,
which `scripts/metrics.mjs` builds and runs beside ours every time — and the
empty app has the same profile: `NSApplication.shared` 60–90 ms (an
Instruments trace shows almost no CPU there: it waits on the window server
and LaunchServices, and this machine's WindowServer was at 45 % CPU),
`NSWindow` init 35–50 ms (`NSThemeFrame` building the titlebar, an
`NSAnimationContext` group, and a `dlopen` of a framework during window
creation), and ~65 ms between `activate` and `applicationDidFinishLaunching`
that disappears under `.accessory` activation policy — Dock registration.
A `.app` bundle and an ad-hoc signature change nothing; the first launch of a
fresh binary pays 160–260 ms before `main` (page-in and the launch
assessment), which the warm-up hides. So a "production build" would not
move this: it is already one, and the platform floor for an empty titled
window here is ~135 ms (accessory) to ~200 ms (Dock). A claim like "boots
in 20 ms" is a claim about the part after AppKit is up — our 11 ms — or a
different machine.

What scales with the app, and is therefore what will get slower: text
measurement (~17 µs per uncached paragraph, ~3 misses per text node at
boot, so ~50 µs per text node — 1,000 text nodes would be 50 ms, and the
answers are a persisted size cache and exact1's shelved 0323 segment
table), view creation and the JSON batch (linear, ~14 µs per node today),
and the per-commit whole-tree diff (exact1 measured ~13 ms per tick at
1,706 nodes and cut it by deferring content apply outside the viewport ±1
height). Each is a number `metrics.mjs` prints; none is a mystery.

## 7. Not in v1 (and where each is declared)

Core Animation delegation for transitions (LLP 1002 §4's measured question;
`rules/NOT-DOING.md` §Motion — the engine presents every frame through the
display link today); UIKit/iOS (the same package shape, the same batch; the
presenter's `NSView` becomes `UIView`); images (`image` nodes draw as boxes);
toggles; keyboard and pointer events beyond click and typing; scroll
position and focus across a reload; accessibility beyond `testId` as the
identifier and `accessibilityLabel`; justified text; per-corner radii
(the first set radius rounds all four); text selection; scroll position
across a reload; a generated header (§4).

## 8. Checks that hold this

`host/apple/tests/host.rs`: the first batch creates, places, and sizes the
whole tree (a `frame` per node, a `content` for the scroll container, typed
style rows, no motion at boot, the root as wide as the viewport and as tall
as its content, nothing overflowing sideways); later batches carry only what
changed and frames follow a resize; a spring arrives as presentation values
frame by frame and settles exactly; text is measured through the registered
callback (a Rust `extern "C"` fn) and the bridge's other calls answer.
`node host/apple/smoke.mjs` green on macOS 26.6 / Swift 6.3 on 2026-08-29,
with the PNG reviewed. Under the five checks the same day.
