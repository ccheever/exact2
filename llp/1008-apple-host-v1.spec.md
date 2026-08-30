# LLP 1008: Apple host v1 — what `host/apple` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Apple host (AppKit and UIKit presenters), Kernel (layout, text measurement), Runner (seam), Motion (native executor), C ABI, Boot
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-29
**Revised:** 2026-08-29 (§9: iOS — the UIKit presenter over the same archive, the Swift the two presenters share, the simulator as the run; §7 and the summary follow); 2026-08-30 (§9: `viewport-fit=cover` with the insets to the kernel, and the keyboard's inset on the viewport; §4: `exact_insets`); 2026-08-30 (§§3–6, §9–11: one dedicated runtime thread prepares native frames off-main; copied CoreText glyph snapshots cross to main; complete batches alone are published)
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

**Runtime ownership, added 2026-08-30.** AppKit/UIKit no longer run the
runner, kernel, Taffy, motion evaluation, CoreText shaping, or Keychain work
on main. One persistent `exact.runtime` thread owns the thread-local C bridge
from boot until exit and prepares every batch in FIFO order. Main admits
platform input and applies a complete batch; it never waits for preparation.
This is the native-frame half of the architecture discussed in the prompt:
non-main work moves as early as the lifecycle permits, leaving one
main-thread frame publication. §11 is the exact contract.

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
width-specific, immutable snapshot: a `CTTypesetter` and its `CTLine`/`CTRun`
objects exist only for one layout operation on `exact.runtime`; before that
operation returns, every glyph id and position is copied into Swift arrays,
with its `CTFont`, and the queue-confined layout objects are discarded. Each
line's baseline from the top and the size use the same `ceil` the kernel
receives. The snapshot is cached by `(spec, width)` behind a lock because the
measure callback reads it on the runtime thread and `NodeView.draw` reads it
on main. Drawing uses `CTFontDrawGlyphs` over those copies, applying the
current text color on main; color is deliberately not part of shaping or the
cache key. What was measured is what is painted, by construction.
`line-height: normal` is the font's ascent +
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
carrying nothing; the host hops to main only to capture the app clock, then
enqueues `exact_pump(now_ms)` on `exact.runtime`, which delivers every queued outcome to the runner
(`parse`, the resource's value or the mutation's slot, one settlement each)
and returns one batch of their commits. A forced request (`refresh`) goes
with `cache-control: no-cache`. `ibex2`'s transport is Objective-C++, so the
Swift packages link `c++` beside the archive. The agent's `clock settle`
polls asynchronously while main and the executor continue to run and reports
`settled: false` after twenty seconds of a request still out; it never spins
or blocks the main run loop.

LLP 1001 §9 left the C ABI "waiting for the consumer that would make its spec
transcription rather than speculation"; this is that consumer, and the ABI
is the web host's buffer discipline over `extern "C"`: `exact_in(len)`
resizes a host-owned input buffer and returns its address; `exact_out()`
returns the output buffer's; `exact_boot(measure, ctx, width, height)`,
`exact_prepare(measure, ctx)` followed by `exact_present(width, height)`,
`exact_boot_plan(len, …)`, `exact_dispatch(view, kind, len, now_ms)`,
`exact_advance(now_ms)`, `exact_resize(width, height)`, `exact_insets(top,
right, bottom, left)` (the safe-area insets under `viewport-fit=cover`, §9;
2026-08-30), and `exact_tick(now_ms)` each return the output's length, a
UTF-8 JSON batch. The app never hands the
host a pointer the host did not give out; the one call the other way is the
measure function. All calls run on the same persistent `exact.runtime`
`Thread`; the bridge is thread-local. A serial GCD queue is insufficient here:
it preserves FIFO order but may move successive blocks between physical
threads, which would select a fresh thread-local bridge. The first run of the
implementation caught exactly that as later calls returning `not booted`;
the dedicated thread is therefore correctness, not an optimization detail.
`exact_apple::host!(DataType, PLAN)` instantiates the exports for one app;
`apps/caltrain/apple` is that one line plus the same `build.rs` as the web
crate, producing `libcaltrain_apple.a`. The header is written by hand (fourteen
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
no meaning on a Mac keyboard. **Declared deviation:** inside a text field, `key` sees only the editing commands the
field editor reports (`insertNewline` → `Enter`, `cancelOperation` →
`Escape`, `insertTab`, the arrows, `deleteBackward`); a typed character is
the field's `change`, where the web's `keydown` fires per character. A view
the presenter no longer has sends nothing (AppKit ends editing as a destroyed
field leaves the window; the browser fires no blur on removal, so neither
does this host). Platform events capture their id/value and clock on main,
then enter `exact.runtime`; its result returns as an immutable `Batch` and is
applied in one main turn. FIFO ownership means a batch apply can enqueue more
work but can never re-enter the runner. Motion frames come from
`NSView.displayLink` while `motion` is true: at most one runtime tick may be
pending, so a slow preparation cannot accumulate stale display frames. The
runner's clock is a 250 ms main timer that likewise enqueues its advance.

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

**The main-thread split (2026-08-30).** Before §11, the current Caltrain app
(275 views, 145 text nodes) measured **10.3 ms runner + layout**, including
**6.7 ms CoreText**, followed by **4.9 ms apply**: about **15.2 ms of main
thread** for one prepared frame. The new ownership moves the first number in
its entirety to `exact.runtime`; main retains the ~5 ms publication. A smoke
of the dedicated-thread build reported 35.3 ms end-to-end runtime preparation
on that fresh launch (10.8 ms CoreText) and 4.7 ms apply; the value of the
change is isolation from main, not a claim that cold work became faster.

**UIKit follow-through (2026-08-30).** An asynchronous publication submitted
from UIKit's first layout missed the initial scene turn and waited another
~30 ms for the main queue. Baked-plan boot is now split: `exact_prepare`
decodes and validates the plan, boots the runner, constructs the initial
mirror/batch, and synchronizes motion before `UIApplicationMain`;
`exact_present` performs only the sized Taffy/CoreText layout. `UIWindow`
already reports its scene bounds and safe-area insets before it is visible,
so UIKit submits the sized phase there, performs the window's required initial
layout, then non-blockingly publishes any ready batch in the same scene turn.
On the iPhone 17 Pro simulator, five launches measured **16.2 ms p50** from
sized submission through publication (11.8 ms CoreText), **14.1 ms apply**,
and **359.2 ms `main` → first paint**. The isolated pre-change `HEAD` measured
380.4 ms p50 on the same booted simulator; the first dedicated-thread form,
before split boot, measured 413.8 ms p50.

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
with the PNG reviewed. Under the five checks the same day. `node
scripts/smoke.mjs ios` green the same day on the iPhone 17 Pro simulator (§9).
The macOS smoke wants the display on: a window on a display that has turned
off is occluded — no canvas renders or is read back, by LLP 1009 D4, so the
deck's placements stand still until a timer redraws text — and
`screencapture -l` refuses it (`screencapture exited 1`); a run while the
display slept, 2026-08-29, showed both, and HEAD's own presenter the same.

The §11 revision was driven end to end on 2026-08-30 after rebuilding both
Swift packages: `node scripts/smoke.mjs macos` green in 5.3 s and
`node scripts/smoke.mjs ios` green in 23.1 s. Both held nested scrolling,
canvas readback, deck placement/input, the motion fixed point and one-seek
parity, safe-area reporting, and the three app tests; iOS additionally held
the atomic keyboard content resize (viewport 874 → 539, bottom bar 840 →
539 under the 335-point keyboard).

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
copied**: `host/apple/swift/` holds `Bridge.swift` (the ABI plus the one
persistent runtime thread), `Text.swift` (CoreText for both — copied glyph
snapshots, a `PlatformFont`/`PlatformColor` alias, the italic trait per
platform, the context passed to `draw`), `Agent.swift` (the
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
the Dynamic Island's 62 and the home indicator's 34). Size-independent plan
boot starts before `UIApplicationMain`; the first sized layout uses
`UIWindow.bounds.inset(by: safeAreaInsets)` before the window is visible,
and every later size follows it (`runtime.resize`). `NodeView.draw(_:)` paints with `UIBezierPath` and the
same copied-glyph draw into the UIKit context; an `input` is a `UITextField`
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
for `email`/`url`/`tel`. The field is framed to the content box (padding and
border, the web's rule); `inputMode=email` (and `url`/`tel`) takes
`autocapitalizationType = .none` the way `type=email` does — a username
field is not a sentence. The agent's `tap … hover` and `type … key`
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
bar. `Controller.fit` frames the viewport from the prop after each layout
pass: the plan boots at the safe area's size (the prop arrives in the first
batch), the viewport is reframed immediately, and one runtime publication
prepares the new insets and size in that order. Agent readiness waits for
that publication; ordinary UIKit never blocks for it. A rotation changes size and insets and sends both; the
dev loop's restart hands the new runner the insets again (`rebooted`). The
style dictionary carries an `env()` length as its resolved points (§2),
re-sent by the batch that changes the insets. `layout` reports the insets
given as `env` (LLP 1012 §1) — zero when the viewport is the safe area, as
`env()` is zero on a page without the meta.

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
step). The notification normally arrives inside `becomeFirstResponder`; the
agent polls until the complete publication makes the inset observable.
`layout.env["keyboard-inset-height"]` is the overlap (335 on the
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
records the keyboard's top and has `Controller.fit` calculate the viewport
ending there with the bottom safe-area inset zeroed (the keyboard's edge has
none — the web's reading). One runtime job prepares `exact_insets` then
`exact_resize`; when both complete, main applies both batches and the
viewport frame in one `UIView.animate` using the keyboard's curve and the
duration remaining after preparation. The inset reported to the agent is
changed in that same publication, never ahead of its kernel frame. Thus the
bar, the form above it, and the shrunken column are one layout and one
observable frame, with nothing computed per animation frame. A container whose height animates stretches its own bitmap for the
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
focus (`NodeView.touchesEnded`), the agent's `tap` the same way —
except a control whose box sits on the field being edited (a password
reveal: the web keeps focus with `mousedown` `preventDefault`); the macOS
presenter does the same for a click (`PageScrollView.mouseDown`, a pressed
node's `mouseDown`: `makeFirstResponder(nil)`), since a browser blurs on a
click anywhere else. The smoke's step 13 taps the fixture's title to send
the keyboard away on iOS, the web, and macOS. `contract/corpus/keyboard-bar.contract` and
the smoke's step 13: the iPhone 17 Pro simulator's viewport 874 → 539 under
a 335 keyboard, the bar's bottom 840 → 539, the bottom inset 34 → 0, all
back on dismiss; Weird Castle's root uses it, with a yellow bar under its
screens. **`overlays-content` and `env(keyboard-inset-height)` (2026-08-30).**
The layout viewport stays; the overlap is published as `Env.keyboard`
(`exact_keyboard`, wire kind 7) and authors pad with
`env(keyboard-inset-height)` (or `keyboard-inset-bottom`). The controller
prepares that layout off-main and publishes the frames in the keyboard's
animation, the same path as `resizes-content`, without shrinking the viewport
or scrolling it — a full-bleed canvas (Weird Castle's night) stays put, the
form above it moves. Because that form is painted *through* the canvas, the
display link captures **presentation** frames for the keyboard's duration
(the model is already at the end; capturing it snapped). The web host emits
the `env()` in CSS and the viewport meta; Safari still has only `resizes-visual`.

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
runner's `StoreWrite`s through `Secrets::set`/`forget` on `exact.runtime` —
the Keychain (ibex LLP 0069): the login keychain on macOS,
`AfterFirstUnlockThisDeviceOnly` on iOS. `build.mjs` signs the macOS binary
with the first Apple Development identity in the keychain (`EXACT_IDENTITY`
names one) so the item's ACL survives a rebuild; ad-hoc otherwise, and the
keychain asks on every rebuild, before the first frame (LLP 1018 D7).

## 11. One runtime owner; one main publication (built 2026-08-30)

### 11.1 Ownership and lifecycle

`host/apple/swift/Bridge.swift` constructs one long-lived `ExactRuntime` at
the first runtime submission. It starts a named, user-interactive `Thread` and
feeds it closures through an `NSCondition`-protected FIFO. That physical
thread — not merely a dispatch queue — is the only caller of every `exact_*`
entry. It consequently owns the Rust thread-local `Bridge`, runner, kernel,
Taffy tree, motion engine, platform text measurement, and host store for the
process lifetime. The executor of §4 remains a separate I/O thread and owns
no runner state.

The earliest size a native app can honestly know is used: AppKit submits boot
as soon as its content view exists. UIKit submits size-independent baked-plan
preparation before `UIApplicationMain`, then submits the first layout when its
`UIWindow` has scene bounds and safe-area insets, before visibility. The
runtime retains the prepared host between `exact_prepare` and `exact_present`.
Its complete `Batch` is posted to main, applied, and only then is the macOS
window ordered or the agent declared ready. UIKit follow-up insets and size
for a cover root are another ordered runtime publication; agent readiness
includes that publication through `runtime.barrier`.

### 11.2 The publication protocol

All platform entrances have the same shape:

1. Main captures the platform fact: view id and input value, viewport or
   insets, or the current app clock.
2. `ExactRuntime` increments a main-owned pending-publication count and
   appends one job to its FIFO.
3. The dedicated thread calls the synchronous C ABI, including every layout
   and CoreText measurement, and parses its JSON into a `Batch` that is not
   mutated afterwards.
4. Main applies that complete batch through `Presenter.apply`, then decrements
   the count. No view is touched by the runtime thread and main never waits.

Prepared results wait in a lock-protected publication queue as well as posting
an ordinary main-queue delivery. UIKit drains that queue without waiting after
its required initial window layout and again at foreground/active lifecycle
boundaries. This lets a batch that completed during `willConnect` publish in
that same main turn instead of waiting for UIKit to service the dispatch queue;
an empty drain is an immediate no-op. Other publications use the ordinary
delivery path.

This covers boot/reload; press, change, hover, focus, blur, key, and submit;
image intrinsic size; timers; viewport/inset changes; network pumps; motion
ticks; and agent reads. FIFO order is the serialization rule. A callback
during apply may submit another job, but cannot call the runner synchronously,
so runner re-entry is impossible. A display link allows only one outstanding
motion job; later display ticks are coalesced until its batch is applied.

`runtime.barrier` is an agent/testing primitive, not application flow. Its
counter includes prepared results already posted to main, so the barrier runs
only after every earlier batch has actually been applied. The JSON-line reader
waits on its own background thread for each asynchronous answer; it never
blocks main. `clock settle` polls pending requests through asynchronous agent
calls, allowing executor wakes and main publication to continue.

The iOS `resizes-content` case is a compound publication. One runtime job
prepares insets first and resize second; main applies both batches and the
viewport frame in one animation with the keyboard curve and its remaining
duration. `keyboard-inset-height`, the safe-area environment, the root frame,
and the bottom bar become observable together. The agent's no-animation path
uses the same batches and delays the reported environment until they apply.

### 11.3 CoreText boundary and what remains on main

Apple documents Core Text's font objects as shareable while recommending that
layout objects such as typesetters, runs, lines, and frames stay within one
operation or work queue. `Text.layout` follows the stronger rule: it copies
glyph ids and positions out and publishes only value arrays plus `CTFont`.
The paragraph and font caches are locked; the platform-font cache remains
main-owned. See [Core Text](https://developer.apple.com/documentation/CoreText)
and Apple's [thread-safety summary](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html).

Main still does the work only the UI process can do: create/destroy native
views, assign properties and frames, draw copied glyphs, run responder/input
paths, and capture or present GPU canvases. This change does not claim a
zero-cost main turn: Caltrain's publication is about 5 ms today (§6). It
removes runner/layout/text preparation from that turn and gives later work a
single place to optimize without changing the native view contract.
