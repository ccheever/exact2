# LLP 1008: Apple host v1 — what `host/apple` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Apple host (AppKit and UIKit presenters), Kernel (layout, text measurement), Runner (seam), Motion (native executor), C ABI, Boot
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-08-29 (§9: iOS — the UIKit presenter over the same archive, the Swift the two presenters share, the simulator as the run; §7 and the summary follow); 2026-08-30 (§9: `viewport-fit=cover` with the insets to the kernel, and the keyboard's inset on the viewport; §4: `exact_insets`); 2026-09-04 (§4: the update store's entries — `exact_update_*`, per process, no handle but `exact_update_sync`; the ABI stays v2; `exact_boot` boots the store's selection); 2026-08-31 (§9: macOS `viewport-fit=cover` is a full-size-content window, titlebar height as `safe-area-inset-top`; §5: Edit menu so the field editor's command keys work); **2026-09-03 (LLP 1031 D2/D1 landed: the C ABI is v2 — every export takes a runtime handle from `exact_create`, callbacks are set per runtime, a destroyed or busy handle is refused by name, never a trap — and the Swift is one package, `host/apple/Package.swift`: the `ExactKit` library (`ExactApp` / `ExactSession` / `ExactView`, the presenters, text per session, canvases and web views per session over modules loaded once, the agent, the dev connection) with `ExactMac` and `ExactIOS` as adapters over it and `ExactHostMac` as the sample host; §4 and §5 describe the shape before that landing where they name `exact_boot(measure, …)`, the static `enum Exact`, `host/apple/macos/Package.swift`, or `host/apple/swift/`; the header comment in `include/exact.h` and LLP 1031 are current)**
**Implementer:** Claude (Fable 5), landing 2026-08-29 (this document transcribes the landing; iOS the same day, §9)
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
layout, and 746 text measurements (399 from cache)**; apply is ~3 ms. **iOS is
the same host** (§9): the same archive built for the simulator's target, a
UIKit presenter of the same shape applying the same batches, and the Swift
that is not about a window — the bridge, CoreText, the agent's clock, the GPU
module's ABI — shared between the two presenters rather than copied. Where
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
when changed), **`present`** (a motion property's presentation value,
each frame the engine changes it), and **`command`** (a capability an
action called, after its commit — LLP 1005 §3; `setScheme` is the app's
appearance on macOS, the window's interface style on iOS). The batch ends with `timers` (the runner
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

Against the browser, measured 2026-08-30 with `layout` on all three hosts
at 420 wide: the same departure rows wrap on the web and on macOS (the
first fits, the next three wrap on both); a two-line row is 32 px in Chrome
and 31 here — `line-height: normal` for the 13 pt system font is 16 there
and 15.5 in CoreText, half a pixel a line and nothing else. (On the Linux
host the font is another — DejaVu Sans, pinned — so its wrapping is its
own.)

This is exact1's conclusion applied without re-deriving it: measuring with
CoreText for layout while painting with TextKit was a "dual-engine
correctness tax" (0418), CoreText wraps ~5× faster than TextKit per
paragraph, `CATextLayer` and TextKit-only were rejected there for body text,
and a size cache hit ~97% under live resize (0322/0323). Here the numbers
came out the same shape on the first measurement: 746 requests for 128 text
nodes at boot (Taffy asks several times per node across its passes), 399
answered from cache, ~17 µs per miss.

## 4. The C ABI (`host/apple/src/abi.rs`, `include/exact.h`)

**Requests (LLP 1016 D2, built 2026-08-30).** A request never reaches the
presenter. `exact_boot`/`exact_boot_plan` take a wake callback
(`ExactWakeFn`, with its context) beside the measure callback; the library
runs an **executor thread** (`host/apple/src/executor.rs`) that owns one
`ibex2::host::Host` — the platform transport, `NSURLSession` — and the app's
`Bindings`, endowed from the data crate's grants (ibex LLP 0067/0068;
`ibex2` with `default-features = false`, taken from the sibling checkout
`../ibex`, the way Weird Castle takes exact2). After every call that
produced a batch, the bridge hands the runner's new requests to that thread;
each outcome is queued and the wake is called *from the executor's thread*,
carrying nothing; the presenter hops to its main thread and calls
`exact_pump(now_ms)`, which delivers every queued outcome to the runner
(`parse`, the resource's value or the mutation's slot, one settlement each)
and returns one batch of their commits. A forced request (`refresh`) goes
with `cache-control: no-cache`. `ibex2`'s transport is Objective-C++, so the
Swift packages link `c++` beside the archive. The agent's `clock settle`
pumps the queue itself while the run loop turns (its handler runs inside a
main-queue block, so the wake's own main-queue pump cannot run until it
returns) and reports `settled: false` after twenty seconds of a request
still out.

**The update store (LLP 1026 D9/D11; LLP 1030 D7; built 2026-09-04).**
`host/apple/src/update.rs` holds one `exact_update::Client` per process
behind a lock — the app's container holds the store and every runtime
boots from its selection, and the runtime registry is thread-local — so
the entries take no handle except `exact_update_sync(rt)`, and the
version stays 2. `exact_update_open(len)` reads
`{"base":…,"assets":…}` from the store's own input buffer
(`exact_update_in`; `exact_update_out` answers) and puts the store at
`<base>/exact/<app id>/update` from `compat.json`'s facts;
`exact_update_select` reports the entry and its assets directory (the
host's overrides by name); **`exact_boot` boots the selection** — the
entry's plan, else the baked bytes — and counts the boot, falling back
to entry zero in the same launch when the entry's plan is refused;
`exact_update_boot_succeeded` at first pixel (`ExactSession.firstDrawn`).
`exact_update_check(done, ctx)` runs the check on a thread of the
library's own over ibex2's transport (the executor's `NSURLSession`) and
calls `done` there with one line; `ExactApp` hops to the main thread,
logs it, and calls `exact_update_sync` per session so the `delivery`
resource follows. `exact_update_activate` hands the staged plan's bytes
over and `ExactApp.apply` restarts every session with carry. Swift holds
no networking for updates; `Updates.swift` is the whole face. The dev
policy fold (1026 D12) is owed: `EXACT_DEV_PLAN` and `PlanURL` are as
they were.

LLP 1001 §9 left the C ABI "waiting for the consumer that would make its spec
transcription rather than speculation"; this is that consumer, and the ABI
is the web host's buffer discipline over `extern "C"`: `exact_in(len)`
resizes a host-owned input buffer and returns its address; `exact_out()`
returns the output buffer's; `exact_boot(measure, ctx, width, height)`,
`exact_boot_plan(len, …)`, `exact_dispatch(view, kind, len, now_ms)`,
`exact_advance(now_ms)`, `exact_resize(width, height)`, `exact_insets(top,
right, bottom, left)` (the safe-area insets under `viewport-fit=cover`, §9;
2026-08-30), and `exact_tick(now_ms)` each return the output's length, a
UTF-8 JSON batch. The app never hands the
host a pointer the host did not give out; the one call the other way is the
measure function. All calls on one thread; the bridge is thread-local.
`exact_apple::host!(DataType, PLAN)` instantiates the exports for one app;
`apps/caltrain/apple` is that one line plus the same `build.rs` as the web
crate, producing `libcaltrain_apple.a`. The header is written by hand (ten
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
with a `press` handler; an input's `controlTextDidChange` is a `change`. The
events beyond those (LLP 1005 §3; 2026-08-30): a `hover` handler is an
`NSTrackingArea` — `mouseEntered`/`Exited`, the previously hovered node's
leave sent before the new one's enter; `focus`/`blur` are first-responder
changes (a field's begin/end editing; a node with such a handler
`acceptsFirstResponder` and takes it on mouse-down); a `key` handler gets
`keyDown`'s name in the web's vocabulary (`Enter`, `Escape`, `Tab`,
`Backspace`, `Delete`, the arrows, else the characters); a `submit` handler
hears the field editor's `insertNewline` (Enter — the web's implicit
submission). An input's `type="password"` is an `NSSecureTextField`, remade
in place if the type changes (a different class on AppKit); `inputMode` has
no meaning on a Mac keyboard. The menu bar always carries a standard Edit
menu so the field editor hears ⌘A/X/C/V/Z — AppKit does not bind those keys
itself (`StandardKeyBinding.dict` has no `selectAll`), and a bar of only app
and Develop items left them unmatched (Weird Castle's login, 2026-08-31).
**Declared deviation:** inside a text field, `key` sees only the editing commands the
field editor reports (`insertNewline` → `Enter`, `cancelOperation` →
`Escape`, `insertTab`, the arrows, `deleteBackward`); a typed character is
the field's `change`, where the web's `keydown` fires per character. A view
the presenter no longer has sends nothing (AppKit ends editing as a destroyed
field leaves the window; the browser fires no blur on removal, so neither
does this host), and an event arriving while a batch is being applied waits
for the batch to finish — the runner is never re-entered.
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

**Images** (LLP 1011 §4 is the spec). An `image` node's `NodeView` loads
and decodes its source off the main thread — an `http(s)` URL as is, a
relative path contained under `EXACT_ASSETS` (the app's directory;
`build.mjs --run` and the smoke set it), the way a page resolves `src`
against its URL; nothing else loads — then, if it is still the current
load of a live view, reports the bitmap's pixel counts through
`exact_intrinsic(view, w, h)`: the kernel lays the image out as a replaced
element (LLP 1001 §1) and the batch carries every frame that moved.
`draw` paints it with CSS `object-fit` (`fill`, `contain`, `cover`,
`none`, `scale-down`; unknown = `fill`) centered in the content box,
clipped to it and to the border radius. Held by the smoke (not blocking):
the Caltrain header's `assets/caltrain.png` (a generated 320×120 PNG)
loads at 320×120 and lays out 96×36 from `width=96`; and by
`host/apple/tests/host.rs` (the batches after `set_intrinsic`).

**The dev loop** landed here too (LLP 1007 §6's shape): `EXACT_DEV_PLAN`
names the plan `host/web/dev.mjs` writes on every save; the app restarts
from it with state carried (`exact_boot_plan`, `Runner::carry`) in ~7 ms.
`node host/apple/build.mjs --run` sets it. `build.mjs` also forces the
Swift relink, since `swift build` does not see the Rust archive change —
a stale link that hid two lanes' changes before it was found. `EXACT_SMOKE=1` prints the boot phases and a summary and exits;
`EXACT_SHOT=<path>` writes a PNG of the window — the run and the picture
`host/apple/smoke.mjs` and a reviewer read. Under a script (`EXACT_AGENT=1`)
the app is an **accessory** — no Dock tile, never activated, its window
ordered front regardless so it is seen and its canvases render — and takes
the focus from no one; a `type` makes the window key itself, which does not
activate the app. The agent's `screenshot` (`cacheDisplay` of the viewport)
paints every canvas's picture, read back from the module, where a Metal
layer would otherwise be blank — the same path a capture takes — so the
sky is in it without asking for `window` (2026-08-30).

## 6. Building and measuring (`build.mjs`, `smoke.mjs`, `scripts/metrics.mjs --long`)

The crate argument names the app; `scripts/app.mjs` (`resolveApp`, 2026-08-30)
turns it into a directory, a cargo workspace, and a target directory — `apps/<name>`
in this repo, or `EXACT_APP_DIR` for an app outside it (weird-castle) — and cargo
runs there while `EXACT_LIB_DIR` points the Swift packages at that target. The
bundle id is `com.exact.<name>`; `ExactMac`/`ExactIOS.app` are the one output slot
per host. LLP 1007 §7 has the shape.

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
display link today); toggles; pointer coordinates and moves (a drag),
`keyup`, double-click, wheel offsets reaching the runner, and a `key` inside a
text field beyond its editing commands (§5); scroll
position and focus across a reload; accessibility beyond `testId` as the
identifier and `accessibilityLabel`; justified text; per-corner radii
(the first set radius rounds all four); iOS text selection; scroll position
across a reload; a generated header (§4).

**2026-09-05, LLP 1033:** Apple paints nested text runs with their own fonts,
colors, and decoration, using the same CoreText paragraph as measurement.
macOS supports drag selection across paragraphs, select-all and copy; inline
link activation goes to the embedding session's `openURL` command delegate.
Selection is presenter state. `ExactSession.change(testId:value:)` lets a native
embedder deliver file data to an existing change handler without compiling UI.
Changing an embedded `ExactApp.assetRoot` now updates its resolver; a complete
signed generation retains its pinned resolver. The macOS Markdown viewer drives
these paths; the iOS document interaction work remains in LLP 1033.

## 8. Checks that hold this

`host/apple/tests/host.rs`: the first batch creates, places, and sizes the
whole tree (a `frame` per node, a `content` for the scroll container, typed
style rows, no motion at boot, the root as wide as the viewport and as tall
as its content, nothing overflowing sideways); later batches carry only what
changed and frames follow a resize; a spring arrives as presentation values
frame by frame and settles exactly; text is measured through the registered
callback (a Rust `extern "C"` fn) and the bridge's other calls answer.
`node host/apple/smoke.mjs` green on macOS 26.6 / Swift 6.3 on 2026-08-29,
with the PNG reviewed. Under the five checks the same day. `node
scripts/smoke.mjs ios` green the same day on the iPhone 17 Pro simulator (§9).
The macOS smoke wants the display on: a window on a display that has turned
off is occluded — no canvas renders or is read back, by LLP 1009 D4, so the
deck's placements stand still until a timer redraws text — and
`screencapture -l` refuses it (`screencapture exited 1`); a run while the
display slept, 2026-08-29, showed both, and HEAD's own presenter the same.

## 9. iOS: the UIKit presenter (`host/apple/ios`, `host/apple/swift`; 2026-08-29)

iOS is this host on its fourth surface, not a fifth host. **The archive is the
same**: `cargo build --release -p caltrain-apple --target aarch64-apple-ios-sim`
(x86_64-apple-ios on an Intel Mac) builds `libcaltrain_apple.a` — the runner,
the kernel, the data crate, the baked plan, the C ABI of §4 — with nothing in
`host/apple/src` or the header changed; `caltrain-gpu` builds for the same
target as the dylib the bundle carries in `Frameworks/`. **The presenter is
the same shape**: `host/apple/ios` is a SwiftPM package (tools 5.9, iOS 17, a
`CExact` system-library target over `exact.h`) whose executable target holds
`main.swift`, `Presenter.swift`, `Gpu.swift`, and `AgentIOS.swift`, and links
the archive the way `macos` does. **What is not about a window is shared, not
copied**: `host/apple/swift/` holds `Bridge.swift` (verbatim), `Text.swift`
(CoreText for both — a `PlatformFont`/`PlatformColor` alias, the italic
trait per platform, the context passed to `draw`), `Agent.swift` (the
request loop, `reply`, `settle`, the `clock` fixed point — everything of
LLP 1012's presenter half that is not `layout`/`tap`/`type`/`screenshot`),
and `GpuModule.swift` (the dylib's ABI, `dlopen`); both packages symlink
them into their target (SwiftPM follows the link). `AgentMac.swift` is what
remained of the macOS agent file. The macOS smoke is the regression check
for the split and stayed green through it.

**UIKit, where it differs from AppKit.** Nothing flips (UIKit's origin is
the top-left). The viewport is a `UIScrollView` over a content-sized
document, framed to the **safe area** — where a browser lays a page out on
a phone without `viewport-fit=cover`: 402×778 on the iPhone 17 Pro (874 less
the Dynamic Island's 62 and the home indicator's 34), and the plan boots at
the first layout pass that has a size, following every later size
(`Exact.resize`). `NodeView.draw(_:)` paints with `UIBezierPath` and the
same `CTLineDraw` into the UIKit context; an `input` is a `UITextField`
reporting `.editingChanged`; a scroll container is a `UIScrollView` whose
content size is held to the box on an axis that does not scroll (UIKit would
pan it otherwise); presentation values go on `transform` about the center,
the frame set untransformed first (UIKit's `frame` is undefined under a
transform). **A press is a touch down and up inside the bounds**; a node
without a handler forwards the touch up the responder chain, so a touch on a
button's text reaches the button as a DOM click bubbles; a pan cancels it
(`canCancelContentTouches`) — scroll always wins. The events beyond press
and change (§5's list): `hover` is a `UIHoverGestureRecognizer`, so a pointer
hovers and a finger never does; `focus`/`blur` are first-responder changes (a
field's begin/end editing; a node with such a handler `canBecomeFirstResponder`
and takes it on touch-up); `key` is `pressesBegan`'s `UIKey` by web name, or
inside a text field `textFieldShouldReturn` → `Enter` only (typed characters
are `change`, §5's deviation), which is also a `submit` handler's event and
sets the return key to *Go*. `type="password"` is `isSecureTextEntry` (with
the password content type); `inputMode` (`email`, `numeric`, `decimal`,
`tel`, `url`, `search`) picks the keyboard, and `type` alone does the same
for `email`/`url`/`tel`. The agent's `tap … hover` and `type … key`
deliver directly by the responder-chain rule, as its press does. The canvas machinery of
LLP 1014 is ported whole (the overlay, placements, `hitTest` through them,
`accessibilityFrame`), with two UIKit facts folded in: the overlay is a
`PlainView` whose `hitTest` ignores its own alpha (UIKit refuses hits below
0.01; a canvas's children painted through its surface composite at 0), and
`hitTest` takes the point in the receiver's own coordinates. `Canvases` is
the macOS one on a `CAMetalLayer` (`layerClass`), capturing with a
premultiplied-RGBA `CGContext` flipped to UIKit's geometry and
`layer.render(in:)`; a nested canvas's cached layer contents are dropped
before each capture so `draw` runs and its readback — where its placements
are read — happens on every capture (the deck in the sky moves on every
seek, as it does on macOS through `settle`, LLP 1014.000 §1c; the smoke pins
it at `+100` on both). No starvation guard: iOS has no window occlusion, a
backgrounded app is `visible == false`, and on the simulator a surface's
first render — its pipelines compiling — honestly takes over 200 ms. The GPU module's device request now takes
wgpu's default limits where the adapter meets them and its downlevel
defaults (with the adapter's resolution) where it does not
(`gpu/src/lib.rs` `load_gpu`): the simulator's Metal device is below the
Apple4 family and offers 15 inter-stage variables to the default's 16; an
iPhone since the A11 offers 31, and every other host meets the defaults.

**The safe areas and `viewport-fit` (revised 2026-08-30).** The layout
viewport is the safe area (above) — a browser's rule for a page without
`viewport-fit=cover` — and what a phone paints behind the status bar and
the home indicator is **the first root's `background`**, as Safari paints
the root element's background under both; white where the root sets none
(the macOS presenter paints the same colour beyond a document shorter than
its viewport). The Caltrain app sets its `main` to the sky's dark when the
sky is on and white when it is off. When the first root's `viewportFit`
prop is `cover` (Contract's `viewport-fit="cover"`, LLP 1006 §2), the
viewport is the whole screen and the safe-area insets go to the kernel
(`exact_insets`, §4; `Host::set_insets`; `Kernel::set_env`, LLP 1001 §2),
where the app's `env(safe-area-inset-*)` lengths resolve to them — Weird
Castle's root pads itself by the four and its dark runs under the status
bar. On macOS the same prop makes the window `fullSizeContentView` with a
transparent titlebar: the viewport is the whole window, the titlebar's
height is `safe-area-inset-top`, and the traffic lights overlay the
content the way a phone's status bar does. `Controller.fit` frames the viewport from the prop after each layout
pass: the plan boots at the safe area's size (the prop arrives in the first
batch) and a cover root is reframed and re-inset in the same turn, before
anything is drawn; a rotation changes size and insets and sends both; the
dev loop's restart hands the new runner the insets again (`rebooted`). The
style dictionary carries an `env()` length as its resolved points (§2),
re-sent by the batch that changes the insets. `layout` reports the insets
given as `env` (LLP 1012 §1) — zero when the viewport is the safe area, as
`env()` is zero on a page without the meta.

The standalone macOS adapter also applies the current `viewport-fit` when it
installs its window callback: mounting the view can already have booted the
embedded plan. Every successful session boot re-sends the view's insets,
as a dev reload does, even if the new root keeps the same viewport mode
(2026-09-04: the external cover-root app exposed both initialization gaps).

**The keyboard (2026-08-30).** A software keyboard does not change the
layout viewport — the web's default (`interactive-widget=resizes-visual`,
Safari's only mode): the visual viewport shrinks and the focused field is
scrolled into it. `Presenter.keyboardChanged` hears
`keyboardWillChangeFrame`/`WillHide`, takes the keyboard's overlap with the
viewport, and inside `UIView.animate` with the keyboard's own duration and
curve sets the viewport's `contentInset.bottom` (and the indicators') to it
and reveals the field being edited (`reveal`: through every scroll container
above it, each moving only as far as it must, within its edges, with 8 pt of
air). The keyboard and the content are then one Core Animation transaction —
the content moves in lockstep, never a frame behind — and nothing is laid
out again. A field focused while the keyboard is already up is revealed on
`textFieldDidBeginEditing`. Under the agent (LLP 1012) the inset applies without the animation, as the
agent's wheel scrolls without one: its world is settled between calls, and
UIKit hit-tests a scroll view at its *presentation* offset while the
keyboard's spring is still settling — a `tap` computed from the model
offset missed the button for half a second (found by the smoke's dismiss
step). The notification arrives inside `becomeFirstResponder`, so the
agent's `type` sees the inset in its next `layout`; `layout.env["keyboard-inset-height"]` is the overlap (335 on the
iPhone 17 Pro simulator). The agent's `tap` now also does what a touch up
does first — the nearest node that takes the focus takes it — so a tap on a
node with a `focus` handler resigns the field and the keyboard goes.
`contract/corpus/insets.contract` and the smoke's step 12 hold all of this
on every host (the keyboard on iOS only; a simulator device shows one only
with *Connect Hardware Keyboard* off in Simulator's I/O › Keyboard menu —
`DevicePreferences.<udid>.ConnectHardwareKeyboard` in
`com.apple.iphonesimulator`, which the smoke does not set).

**`interactive-widget="resizes-content"` (2026-08-30, the same day).** The
web's opt-in for what the default cannot do — a bar pinned to the bottom
that rides on the keyboard (Chrome Android's mode; Safari has none): the
layout viewport ends at the keyboard's top. When the first root's
`interactiveWidget` prop says so, `keyboardChanged` does not inset; it
records the keyboard's top and, *inside the keyboard's animation block*,
has `Controller.fit` frame the viewport to end there with the bottom
safe-area inset zeroed (the keyboard's edge has none — the web's reading)
and send `exact_insets` and `exact_resize`; the batch's frame ops are set
inside that block, so every frame that moves is a Core Animation move with
the keyboard's own duration and curve, in the keyboard's transaction — the
bar, the form above it, the shrunken column — one layout, nothing per
frame. A container whose height animates stretches its own bitmap for the
duration (`contentMode = .redraw` repaints once, at the new size); a solid
background does not show it, text nodes keep their size and translate. The
field being edited is revealed after, through the scroll containers above
it. `layout` then reports the shrunken viewport (as `innerHeight` shrinks
under this mode in Chrome) and, as `keyboard-inset-height` still, the
keyboard's overlap with the viewport the controller would frame without
one (measured against the shrunken frame it read 0 — the first bug the
Weird Castle bar found). **A focus moving from one field to another** comes
as a burst of `keyboardWillChangeFrame`s with no duration, over a few
turns — the height jittering between the two keyboards (335, 308, 335 on
the simulator; the email keyboard and the default) — and laying out for
each flashed the page (the second bug it found). No-duration changes now
wait 80 ms for the last of them (`keyboardDebounce`), which usually
changes nothing; an animated change — the show, the hide — is applied at
once, in the keyboard's own transaction. What remains of a hand-off is the
keyboard's own one-frame blink — its accessory bar torn down for the
outgoing responder and rebuilt for the incoming one — and it is UIKit's,
not this host's: a from-scratch UIKit app with two bare `UITextField`s
produces the same 308 → 335 burst on every switch, programmatic or
touched, with any traits (identical plain fields included) and even when
one field is re-traited in place with `reloadInputViews`; a phone shows it
faintly too (Charlie, 2026-08-30: "bearable"). Safari masks it below the
responder, in WebKit's own input-assistant handling — how is an open
question, in the queue. **Dismissing it** is the web's
rule: a tap that lands on nothing that takes the focus blurs the field
and the keyboard goes — a touch nothing consumed reaching the viewport
(`ScrollView.touchesEnded`), a press on a node that does not take the
focus (`NodeView.touchesEnded`), the agent's `tap` the same way; the macOS
presenter does the same for a click (`PageScrollView.mouseDown`, a pressed
node's `mouseDown`: `makeFirstResponder(nil)`), since a browser blurs on a
click anywhere else. The smoke's step 13 taps the fixture's title to send
the keyboard away on iOS, the web, and macOS. `contract/corpus/keyboard-bar.contract` and
the smoke's step 13: the iPhone 17 Pro simulator's viewport 874 → 539 under
a 335 keyboard, the bar's bottom 840 → 539, the bottom inset 34 → 0, all
back on dismiss; Weird Castle's root uses it, with a yellow bar under its
screens. Not built: `env(keyboard-inset-*)`, the `overlays-content` mode.

**The agent (LLP 1012) on iOS.** A simulator app has no stdin, so
`EXACT_AGENT=1` with `EXACT_AGENT_SOCKET=<path>` listens on a Unix socket
(under 104 bytes; the driver makes it in the temp dir) and speaks the same
JSON lines; `ready` carries the app's pid. `layout` is the viewport's
content space less its offset, transforms carried by UIKit's `convert`
(the motion fixture's 75 and 86.55 hold). **`tap` is the one declared
deviation from §1's contract**: UIKit offers no public touch synthesis, so
a tap hit-tests through the window (UIKit's own, placements included) and
delivers the press by the responder-chain rule a touch gets
(`NodeView.activate`, which VoiceOver's `accessibilityActivate` also uses);
a wheel applies LLP 1010's chaining rule from the hit view up. `type` is
`becomeFirstResponder`, `selectAll`, `insertText` — one `editingChanged`
with the whole value. `screenshot` is `drawHierarchy(afterScreenUpdates:)`
at the screen's scale in the standard (8-bit sRGB) range — a wide-color
screen would otherwise yield a 16-bit PNG — and sees Metal, so `window:
true` is the same picture. The driver's `ios` carrier (`scripts/agent.mjs`
`openIOS`) installs the bundle, launches it with `simctl launch --console
--terminate-running-process` and keeps that attached for the app's stdout
and stderr (simctl's `--stdout=`/`--stderr=` files stay empty on Xcode
26.6), connects to the socket as it appears, and on close hangs up (the app
exits at EOF) and kills the reported pid if it lingers. One app per bundle
id per device: a session replaces a running copy.

**Building and running.** `node host/apple/build.mjs --ios [--run] [--sim
<udid|name>]`: cargo for the simulator target, `swift build --triple
arm64-apple-ios17.0-simulator --sdk …`, the `.app` assembled from scratch
(its `Info.plist` written — a scene manifest, `UILaunchScreen`,
`MinimumOSVersion` 17 — never committed; new inodes, ad-hoc signed),
`simctl install` on the simulator `--sim`/`EXACT_SIM` names, else a booted
iPhone, else the iPhone Pro on the newest iOS (booted and waited for);
`--run` shows it in Simulator.app with `EXACT_DEV_PLAN` watched, so the web
dev loop's saves restart it too. Warm, 2026-08-29: cargo 0.2 s, swift 3.5 s
(a relink 0.8 s), install 0.4 s; the first install boots the simulator
(~10 s). The simulator helpers are exported to the driver.

**Measured, 2026-08-29** (iPhone 17 Pro simulator, iOS 26.5, Xcode 26.6,
Swift 6.3, this Mac; printed, not asserted): smoke mode exec → `main`
144 ms; `main` → `didFinishLaunching` 18 ms, → window 86 ms; runner + layout
22 ms of which 883 text measurements (484 cached) 13.8 ms in CoreText; apply
12.5 ms; **`main` → first paint 127 ms**, 273 views. Under the agent, boot
59–78 ms (`main` → first frame applied). The sky's capture is 1206×2334 in
21–33 ms, uploaded (11 MiB) in ~1 ms; the deck's 48 children in 60–80 ms.
`node scripts/smoke.mjs ios`: **green in 11.7 s** — the landmarks, the
logo 96×36, sixty timers from one seek, the station change through the
real hit-test, a wheel of 300 taken by exactly one container, the scroll
fixture stopping at **652** (the same number as macOS and the web: the same
CoreText, the same font), the canvas readback matching its recorded
reference `scripts/fixtures/canvas-sky.ios.png` **to the pixel** (0.00%),
seven captures, the deck's placements and focus, the motion fixture's 75 /
86.55 / 1500. The `--shot` pictures were reviewed: the aurora behind the
station, the boards refracted through the glass, the line map's stations
laid out by the kernel.

**A phone** (`--device`, 2026-08-30): the same script builds the archive
and the GPU dylib for `aarch64-apple-ios`, the presenter for
`arm64-apple-ios17.0` on the `iphoneos` SDK, and assembles a second bundle
(`.build/device/ExactIOS.app`, `iPhoneOS` in its plist, the app's `assets/`
inside — a phone reads no other machine's paths, so the presenter's asset
root defaults to the bundle) signed for real: the phone `devicectl` knows
(`--phone`/`EXACT_PHONE`, else the reachable one, else the only one), a
development profile on this Mac that covers it and the bundle id (the
team's wildcard or the id itself; unexpired; `EXACT_PROFILE`), the
keychain's Apple Development identity for that team (`EXACT_IDENTITY`),
entitlements written from them (`application-identifier`, the team,
`get-task-allow`), the profile embedded — then `devicectl device install
app` and, with `--run`, `process launch`. No Xcode project: the profile is
one Xcode once put on the Mac for any app of the team. Verified
2026-08-30: `codesign --verify --deep --strict` passes and the entitlements
read back; the bundle installed on the iPhone 17 Pro Max this Mac has
paired (iOS 26.6) **over Wi-Fi** — `devicectl`'s `localNetwork` transport,
no cable — in 8.1 s, launched, and stayed running (`devicectl device info
processes` shows it). A phone that is asleep, on another network, or with
Wi-Fi off is `unavailable` to `devicectl`; the script says so and stops
after signing.

**The frame rate on the phone, measured (2026-08-30).** `EXACT_FPS=1` makes
the display link run always and report once a second — frames delivered,
the longest gap, the canvases' renders and captures with their times — to
stderr (which `devicectl device process launch --console` relays), to
`Documents/fps.log` in the app's container, and to a readout across the
top of the screen; the app grew a "Sky off / Sky on" button for the
comparison (`state sky`, `component Content`, `when sky` choosing the
canvas or a plain scroll — `apps/caltrain/app.contract`). On the iPhone 17
Pro Max (iOS 26.6, a 120 Hz display; the bundle's plist opts in with
`CADisableMinimumFrameDurationOnPhone`, and in fps mode the link asks for
80–120, or a display link there is held to 60 and measures nothing above
it — the first trace read a flat "60" with the sky off for that reason):
**with the sky off, scrolling, 120 fps, the longest gap 8.3 ms — every
frame**. **With the sky on, scrolling, 42–64 fps: 30–40 captures a second
at 20–25 ms each** — the whole app rasterized into a 1206×2334 bitmap and
uploaded (11 MiB) on every scroll frame, because the scroll container lives
inside the canvas and each scroll repaints through it (LLP 1014 D4 c) —
while the aurora's renders cost **0.1–0.2 ms** a frame. Idle with the sky
on, 111–114 fps with a 55–75 ms gap once a second: the countdown's tick
changes text, and one text change re-captures the whole sky. So the cost
is the children capture, not the shader. Two ways down were tried the same
day and are declined: **capturing at 2× on the 3× phone is slower** — 28–39
ms a capture, 34–45 fps — because `layer.render(in:)` then resamples every
view's 3× backing store instead of blitting it (the cost is per-layer work,
not pixels), so the scale stays the screen's; and **`CARenderer`** cannot
render the overlay — assigning it a layer takes the layer out of the
window's tree, and UIKit's next `superview` walk faults (a layer is in one
tree only); and **the render server's snapshot** — the overlay composited
under the Metal layer at alpha 1 and `drawHierarchy(afterScreenUpdates:
false)` into the bitmap — costs the same 26–27 ms and is refused
intermittently. Three rounds, so the loop stopped there (`rules/RULES.md`),
and the answer was a design, built the same day:

**The shadow-layer capture** (`host/apple/ios/…/Shadow.swift`). The
overlay stays in the window's tree, where UIKit needs it — touches, the
keyboard, accessibility — and a **shadow tree of plain `CALayer`s mirrors
it** on every capture: geometry, opacity, clipping, corner radius, and the
same `contents` objects (the views' backing stores, shared, never copied;
a shape, text, or gradient layer's own state besides), each property set
only where it differs, the sublayer lists re-attached only when they
changed, layers kept by the source layer's identity. **`CARenderer`
renders that tree on the GPU** into a Metal texture (`rgba8Unorm`,
private) on a command queue the presenter owns; the texture is cleared by
a render pass on that queue first (the renderer composites over what the
texture holds — the previous frame appeared as ghosts of every text), a
blit on the same queue reads it back into the capture bitmap, and the rows
go in bottom-up (with a flipped root the renderer's picture is upside down
as a whole). A view the batch just invalidated is displayed before it is
mirrored (`displayIfNeeded`), so the capture is not one frame behind it —
the logo, as it loaded, was the tell. A nested canvas contributes its
readback picture as its layer's contents, as it does through `draw` in the
CPU capture, which remains the fallback where there is no Metal device
(`EXACT_CAPTURE=cpu` forces it, the fixture's oracle;
`EXACT_CAPTURE_DUMP=<dir>` writes both captures of a frame as PNGs, the
way the orientation and the ghosts were found). The smoke's readback
fixture, recorded from the CPU capture, holds the GPU one at 0.37% beyond
the band, mean 0.11. With the readback still in the loop, scrolling with
the sky on measured 110 fps on average (86–120), a capture 10.7 ms — the
mirror 4.5, the GPU 1.8, the readback 2.1 — against 20–25 ms and 42–64 fps
before.

**Two more steps took it to the display's rate.** A nested canvas's
readback is cached (`Canvases.picture(of:)`): the line map, once drawn,
is rendered again and read back only when the module reports it dirty or
its surface wants a frame at a new clock — it had been re-rendered on every
capture of the sky. And **the texture is handed to the module as it is**:
`gpu_texture_metal(id, w, h, texture)` (Apple targets; `gpu/src/lib.rs`
`texture_from_metal`) retains the `MTLTexture` and imports it through
wgpu-hal's Metal backend (`Device::texture_from_raw`,
`create_texture_from_hal`) as the canvas's children — no readback, no
upload; the previous children are copied out of it at each hand-over for a
surface that crossfades. Two things about that hand-over were found on the
phone, not the simulator: the renderer's queue option must be the real
constant (`kCARendererMetalCommandQueue`, read by `dlsym` — its Swift name
is not exported on iOS; passed as a guessed string the renderer used its
own queue and the module sampled half-drawn textures), and **it must be one
texture**, imported once: two in turn made the module re-import and
re-bind every capture, and a surface takes a new children view as a fresh
set — the glass crossfaded on every capture, a flicker with the alpha in
between. The module's reads of the one texture are complete before it is
drawn into again: `gpu_sync()` (`device.poll(Wait)`), which the capture
calls first. **On the phone, scrolling with the sky on: 119 fps on
average (101–120), 18 of 19 seconds at 110 or more, the longest gap 13
ms on average; a capture 7.1 ms** — the mirror 1.9, the GPU 1.6, the rest
the wait for the module's last frame — against 20–25 ms and 42–64 fps
where this began. The deck's per-child textures still take the byte path
(a texture each; not on the scroll path).

**Not in v1 (iOS):** a synthesized touch for `tap`; a pan chaining out of a nested scroll view at its edge
(UIKit's own behavior stands; the agent's wheel chains); rotation is handled
but untested; `scripts/metrics.mjs` has no iOS row; the agent API on a
phone (the socket is a simulator's; a phone would want the same lines over
`devicectl`'s tunnel or USB).

## 10. The store (LLP 1018, as built 2026-08-30)

Nothing crosses the ABI: `host/apple/src/store.rs` endows the app's
`ibex2::host::Bindings` once at boot (`EXACT_AGENT=1` selects a memory store
unless `EXACT_STORE=real`), reads each granted name through `Secrets::get`
into the runner's snapshot (`Host::boot_stored`), and hands the same bindings
to the executor thread; after every commit `Host::persist` writes the
runner's `StoreWrite`s through `Secrets::set`/`forget` on the main thread —
the Keychain (ibex LLP 0069): the login keychain on macOS,
`AfterFirstUnlockThisDeviceOnly` on iOS. `build.mjs` signs the macOS binary
with the first Apple Development identity in the keychain (`EXACT_IDENTITY`
names one) so the item's ACL survives a rebuild; ad-hoc otherwise, and the
keychain asks on every rebuild, before the first frame (LLP 1018 D7).
