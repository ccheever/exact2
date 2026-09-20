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

The `content` op carries natural scrollable overflow, including the end padding
correction, without flooring it to the node's frame (2026-09-10). The platform
presenter applies the ordinary client-size minimum: UIKit already does this;
AppKit retains the extent and recomputes the document size on both content and
frame changes. This preserves sub-client content changes for native containers
with their own insets. A Rust regression holds the below-client updates; live
iOS/macOS fixtures verify short content, growth, overflow, shrinking and client
resizing (`/tmp/messages-content-extent/`). Native title projection remains
unshipped under LLP 1035.001 D9.

A clock advance can collect several receipts against the final kernel tree.
The host creates all surviving views before emitting their final `children`
lists, so an early timer cannot attach a child that a later timer has not yet
created in the presenter. Motion still consumes each receipt at its own time.

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
text, no background, no border). Effective overflow and CSS inheritance
also cross even without an own row (LLP 1035.000, landed 2026-09-09): a
`text` node or an editor gets every inherited row's computed value — the
font rows, alignment and colour it measures and paints with — and any other
node its computed colour, through `NodeRef::computed_style` (LLP 1001 §6); a
row resolved to its initial value stays out of the dictionary (the presenter
carries CSS's defaults), colour excepted. A light/dark pair stays a pair. The
kernel touches the descendants an inherited change reaches, so the ordinary
update path re-sends them and the host never re-derives per frame.
UIKit/AppKit containment therefore cannot break CSS inheritance;
`host/apple/tests/host.rs` holds colour and the text rows (a paragraph's size
reaching its runs and a field, an override never re-sent, a resize sending
no style).

Border widths cross as the kernel's effective widths (LLP 1001 §1), so
`none`/`hidden` consume no editor/image inset and paint nothing even if a width
was authored. Visible borders supply their default 3-point width and resolve
`currentcolor` against the node's computed text colour, preserving appearance
pairs. Swift consumes those values through its existing layout and paint paths;
it does not calculate border-style semantics independently. Live style and
inherited-colour cases are held by `tests/inherited.rs` and the native fixture
in `/tmp/messages-border-semantics/`.

## 3. Text: one engine (`host/apple/src/measure.rs`, `macos/…/Text.swift`)

The kernel hands its measurer a paragraph as ordered runs with an offer
(LLP 1001 §6). Here the measurer is `CallbackMeasurer`: the app's C function,
registered at boot, called with the runs flattened into C structs (UTF-8
bytes with lengths, points, `EXACT_MAX_CONTENT`/`EXACT_MIN_CONTENT` for
unconstrained offers). No `unsafe` on the Rust side: calling a safe
`extern "C"` fn pointer is safe Rust, and the structs live for the call.

The Swift side is CoreText and nothing else. A **`Paragraph`** is a
width-specific snapshot — the wrapped `CTLine`s from a `CTTypesetter`, each
line's baseline from the top, and its layout dimensions — cached by `(spec,
width)`. An authored line height retains the fractional sum of its line boxes,
including empty and clamped paragraphs. Intrinsic width and `normal` paragraph
height retain their existing whole-point ceiling: changing those measurements
requires a separate wrapping/host-parity comparison. The measure callback answers
from the paragraph, and `NodeView.draw` paints those same lines: one `CTLineDraw`
per line, flush by alignment inside the CSS content box. Borders and padding
are excluded from the wrapping width and added to the painted origin; the
paragraph cache keys that content width, even when its outer frame is unchanged.
AppKit selection and link hits use the same origin and alignment. The padded
paragraph/container comparison, padding mutation and recreation, and Messages
quoted replies and empty results are recorded in `/tmp/messages-text-padding/`.
Painting still snaps baselines to the logical point
grid, independently of the reported fractional baseline; that remaining raster
placement difference is separate from an authored line box's height.
`line-height: normal` is the font's ascent +
descent + leading; a set line height centers the glyphs in the box; the
first baseline is reported so Taffy's baseline alignment works; `line_clamp`
adds `…` when later text is hidden, then truncates the final visible line to
make room. The candidate retains paragraph-global UTF-16 indices for selection;
trailing breaks/spaces are removed before the token. A box narrower than the
token keeps the clipped first character. Previously CoreText received an
already-fitting line and omitted the token. The Swift regressions and iOS/web
long-reply drive are in `/tmp/messages-line-clamp/verification.json`. Mixed-run
token styling still differs from the browser's paragraph-styled ellipsis, and
the browser fixture does not show the token on a right-aligned line; full text raster
parity remains open. `overflow-wrap` travels through `ExactMeasureRequest`
and the paragraph cache. Normal uses public Unicode line-break boundaries
so CoreText's emergency word split becomes overflow; `break-word` retains
that split. `anywhere` additionally measures min-content by composed-character
clusters; the other modes retain the widest word. A forward cursor consumes
line boundaries once rather than searching the whole list for every line.
The normal/break-word/anywhere/restored fixture agrees with the browser's
finite-width and flex minimum behavior (`/tmp/messages-overflow-wrap/`).
Native editing controls keep their existing UIKit/AppKit wrapping policy;
explicit overrides on those controls remain unverified. Fonts are cached by
(size, weight, italic).

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
`textarea` carries a plain-text `NSTextView` in an `NSScrollView` (a
`UITextView` on iOS); its `change` preserves newlines, Enter inserts a
newline without submitting, and its font, color, padding and border use
the same rows as `input`. Contract lowers it to `TextInput` with the existing
`semanticTag="textarea"`, emitted as a real `<textarea>` on the web. The
HTML `readonly` lowers to the inverse of the existing `editable` prop,
keeping the text selectable while refusing edits. The app gives this bounded
editor its width and height; rich text is outside this control.
UIKit textarea line spacing uses the authored `line-height` through paragraph
minimum/maximum heights; clearing it restores TextKit's natural spacing. Storage
and typing attributes update in place, preserving text and selection. TextKit's
baseline within each box still differs from CoreText's centered CSS baseline.
`field-sizing="content"` (2026-09-09, Messages) instead measures the current
value/placeholder, constrained by CSS min/max dimensions. Fixed controls use a
preferred size independent of value; explicit dimensions still win. The browser
receives the CSS property itself. A trailing textarea Return now measures its
empty caret line without inserting content into the value (2026-09-10, kernel
text measurement). Browser/iOS growth and shrink drives and public XCTest typing
cover both Messages composers; physical typing and internal scrolling keep the
last line visible (`/tmp/messages-composer-growth/`). The later real-Paste drive
also keeps it visible. Driver bulk replacement's clipped capture was premature:
UIKit continues revealing the caret after insertion returns. `clock settle` now
observes that editor within its existing native bound, using its actual caret
and reachable scroll range. Blur, logical removal/replacement, an inactive route
or user tracking ends the observation; it holds the native editor weakly and
never scrolls it (`/tmp/messages-editor-paste/`, LLP 1035.003 D5). A subsequent
viewport repair observes geometry animations in the session's mounted view
hierarchy, including groups, and requires an idle native turn after native work
finishes. That turn catches keyboard work queued by responder or sheet completion.
Immediate/later captures now remain stable; session isolation, the two-second
bound and physical manual-scroll retention are exercised in
`/tmp/messages-viewport-settle/`. Opacity-only animation and containing-app views
are outside this geometry observation; the driver never seeks UIKit's clock.
Changed `scrollTop` and `scrollLeft` props are DOM-style
assignments after the complete batch’s geometry/children are mounted. Each
axis clamps independently to its content extent; an unchanged axis retains its
offset. These are not permanent scroll locks: unrelated patches do not reapply
them. A pending offset that already equals the live position (including a
UIKit overscroll) is not reapplied or clamped; this preserves an active pan or
deceleration when the producer mirrors a scroll event into its binding.
A `display: none` ancestor or scroll container retains the position from before
its box disappeared. Hidden offset assignments are ignored, and collapsing its
native extent emits no scroll event. On restoration, the saved position clamps
to the new extent. Browser/iOS/macOS fixtures cover both axes, manual scrolling,
self/ancestor hiding and content shrinkage while hidden.
An `input` node carries an `NSTextField`; a `scroll`/`list` node an
`NSScrollView` whose flipped document view holds the children and takes the
`content` size. A new leading child is inserted before existing Exact-only
siblings, preserving their mounts; containers with native decorations retain
their existing ordering path. Frames are set from `frame` ops; `present` ops set an
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
App commands are declared on buttons with `aria-keyshortcuts`, for example
`"Meta+S Control+S"`. Hosts bind a space-separated list of chords with one
character key and explicit `Meta` or `Control`; `Alt` and `Shift` may also
qualify a chord. Bare `Escape` is also supported, with no modifiers; other
named unmodified keys are not bound. Matching uses the exact modifier set
and ignores character-key case.
The same mounted button and its `disabled` state govern clicks, keys, and
native menu actions; disabled matches and held-key repeats are consumed
without dispatch. macOS routes commands before text editing, scoped to the
focused Exact view, and derives menu labels and Command equivalents from
those buttons (one menu entry per button; Control alternatives do not create
duplicates). On macOS, `Meta+,` appears as Settings in the application menu;
tablist tab commands and `Meta+[` / `Meta+]` appear in Go; other declared
commands appear in File. The standalone adapter supplies About, Services,
Hide, Quit, Close Window, Full Screen, Minimize, Zoom and Bring All to Front
through AppKit. Reconciliation preserves those static commands and unchanged
declared menu items. Menus revalidate mounted, enabled, non-inert nodes in
the key window and refuse under a native sheet (2026-09-15, Interview).
An action may issue `focus("html-id")` to move focus after its batch has
mounted nodes and applied values. It resolves the existing HTML `id` within
that session/root, never `testId`; missing, disabled, hidden, or inert targets
are ignored. Native input/textarea focus uses the platform editor and the
existing focus event; web uses the element's focus. The command takes one
string, does not select text, and does not activate the target. macOS and iOS
handle it inside the session before forwarding external delegate commands.

`copyText(text)` (2026-09-10, Messages) is also handled inside the session,
after its committed batch. Exactly one string is required. iOS assigns it to
`UIPasteboard.general.string`; macOS clears the general pasteboard and writes
the string representation, reporting failure on stderr. There is no clipboard
read, editor selection, synthetic key event or focus change. The application
separately dismisses its menu and restores its prior editing state. The command
adds no agent operation or clipboard-read API.

`selectText("html-id")` (2026-09-10, Messages) focuses a mounted, enabled
text editor after its batch, then selects its whole value using UIKit's
`selectAll(editor)` or AppKit's `selectAll`. It shares `focus`'s visibility,
inert and modal deferral checks; non-editor targets are refused. A read-only
textarea permits selection without opening the keyboard, and its native
selection handles and edit menu own range adjustment and Copy. Parent reply,
long-press and double-tap recognizers yield to touches inside an editor.
Read-only UIKit selection sessions emit `blur` on first-responder resignation,
as their editing delegate otherwise omits it. No clipboard read or new agent
operation is involved. The app decides when to remove its selection surface.
The shared session dispatcher must consume this command before external delegate
delivery. A current-build Messages drive found that branch missing: the editor
appeared but selection never began (`unknown command selectText`). Restoring the
branch selects the full value after mounting. A Swift batch regression covers
creation before delivery and keeps product commands external; public XCTest covers
an outgoing long press and range adjustment, plus Select, native Copy and outside
dismissal on outgoing and incoming messages (`/tmp/messages-selection-geometry/`). The current
17-point/20-point specimen retains glyph placement across selection; this does not
establish native return-motion or general text-raster parity.

`autocapitalize` and `autocorrect` (2026-09-09, Messages) configure both
UIKit editor types. Capitalization accepts HTML's case-insensitive `none`/`off`,
`sentences`/`on`, `words` and `characters`; absent, empty or invalid values use
the host's sentence-entry default. Correction is off only for `off`; otherwise
UIKit's default respects user preferences. Email, URL and password **input**
states force both off even when explicitly enabled; textarea has no such type
state. Changed hints reload a focused editor's input views. AppKit applies
correction to the textarea or active shared field editor, restoring the setting
when a different field starts editing; it has no virtual-keyboard capitalization
projection. Web uses the actual HTML attributes. Autocorrection off does not
guarantee UIKit removes the predictive strip: on iOS 26.5 it can remain empty.

`spellcheck` independently controls spelling checking. The kernel resolves the
nearest explicit hint under [HTML’s checking algorithm](https://html.spec.whatwg.org/multipage/interaction.html#spelling-and-grammar-checking),
leaving authored props intact. The Apple host propagates ancestor changes and
reparenting to otherwise untouched editors; removing the last hint clears the
projected prop. UIKit uses `.yes`, `.no`, or `.default` for both input and textarea,
reloading input views if a focused editor’s hint changes. AppKit enables continuous
spelling checks unless explicitly disabled, including on shared field-editor
handoffs. Correction remains a separate setting. On the recorded iOS 26.5
recipient keyboard, disabling both correction and spelling removes the empty
27-point strip; this is observed platform behavior, not a promised keyboard height.

The browser handles chords it receives, including while an input or textarea
is focused; a browser-reserved chord such as Command-N may never reach the
page. This binds the declaration explicitly; ARIA alone does not install a
browser keyboard handler. Undeclared and unsupported chords keep the host's
normal handling.

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

**Clipping.** `clip_path` carries validated absolute path commands from the
kernel (LLP 1001 §1). The shared `ClipPath.swift` makes one `CGPath`; UIKit and
AppKit apply it as a `CAShapeLayer` mask, clipping the node and its descendants.
Their hit tests reject points outside that path before walking children. Clearing
the row removes the mask. Coordinates are fixed CSS pixels from the border-box
origin; the path does not scale on resize or change kernel geometry.

**Symbol images** (LLP 1035.004, 2026-09-10) resolve seven portable roles
from the kernel schema. `UIImageView` / `NSImageView` renders the configured
native glyph inside the kernel-owned image leaf; computed inherited font size
and weight configure its point size, while non-inherited `tint_color` supplies
the template colour and follows appearance. CSS sizing, padding, clipping and
`object-fit` retain their meaning. Native intrinsic reports are deferred beyond
the current batch and checked against actual view identity and load generation.
The native child is decorative and noninteractive; the containing control owns
its label and hit target. `layout` reports `native.symbol` (renderer, generated
name, intrinsic size and frame). Dynamic unknown roles paint empty and log a
refusal. The iOS renderer and button interactions have saved pixel/touch evidence;
AppKit geometry and actions pass, but its transparent captures do not prove paint
(`/tmp/messages-symbol-integration/verification.json`).

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

**2026-09-05, long-document pass:** inline text identities stay in the presenter
map as run data, but only paragraphs mount in the Apple view hierarchy. Text
changes invalidate the owning paragraph; frame-only changes retain its spec and
reuse its painted layout at the same width. CoreText typesetters are cached across
widths; metric-only paragraph keys let painting reuse measurement's line breaks
(clamped paragraphs still take the truncation path). Caches evict their coldest
eighth at capacity instead of clearing everything. macOS paints text only inside
the viewport, invalidates newly exposed regions when scrolling, and caches the
selection's paragraph order. Capture still paints its requested region. The whole
document remains laid out; this is not block-layout virtualization. LLP 1033
records the long-document benchmark and remaining reflow cost.

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

**Router projection (LLP 1038 D6/D7/D11, 2026-09-14).** The Rust host
emits the coalesced `router{top,url,removed}` op at boot and beside commands
after commits. The session retains the last op for both agents' `navigation.url`;
Native URL entry points (LLP 1038 D5/D8/D11) use `exact_location_of` through the runtime buffers, then set the launch location before boot or dispatch kind 14 to the root's `navigate` handler. iOS handles `connectionOptions.urlContexts` and browsing-web user activities before view construction, and `scene(_:openURLContexts:)` / `scene(_:continue:)` while running. macOS handles the first non-file URL in `application(_:open urls:)`; attachment and boot wait until `applicationDidFinishLaunching`, after launch URL delivery. No URL means `/`. The explicit development `open?url=` link is checked first and never reaches `navigate`; file URLs keep the document path. `type` on the root uses the same dispatch without focusing an editor. Manifest `urlSchemes` reach `CFBundleURLTypes`; `host.ios.associatedDomains` accepts explicit entitlement entries or `true` for the app origin's `applinks:` entry. UIKit animates a controller
change according to the prefix relationship of the controller lists, whatever verb produced it (`NavigationRules.isPushOrPop`):
an `open` that extends the current stack animates as a push, one that replaces it swaps.
UIKit cancellation may call `willShow` for the source a second time; its
interactive source is retained until `didShow`, so the agent reports `cancelled`
and dispatches no Back. The existing agent freeze and modal conditions still apply. AppKit uses
`NavigationRules.stack` on each navigation container's direct keyed children:
only the selected route is visible and interactive, except its immediate
underlay when selected presentation is `modal`, visible but inert. Other routes
are hidden through `NSView.isHidden`; ancestor inert gates input, focus and
accessibility. An unmatched selection leaves projection unchanged and journals
once per key (1035.001 D6).

LLP 1039 passes the session’s layout viewport to the runner before first settlement; `resize` re-answers `exactViewport` and merges that commit with the existing relayout batch. Under `interactive-widget="resizes-content"`, the keyboard-adjusted frame is therefore both the kernel’s viewport and the app’s `height`.

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

**Short vertical scroll containers** (2026-09-09, Messages): effective vertical
`overflow: scroll` enables UIKit's `alwaysBounceVertical`, so a list that fits
still responds elastically to a finger drag. Non-scrolling axes remain locked;
content size and programmatic offset clamping are unchanged. This is the host's
native boundary affordance, which [CSS leaves to the user agent](https://drafts.csswg.org/css-overscroll/#boundary-default-actions),
including containers without overflowing content. It adds no property or shared
motion executor. Messages' full and filtered inboxes exercise held dragging,
reversal and release. Exact's current drag-to-offset response still differs
from the native Messages large-title list; enabling bounce does not establish
matching native title motion.

**Native swipe rows** (2026-09-10, Messages; `SwipeActionsIOS.swift`). An
explicit `swipeContent` id on a scroll node requests a UIKit cell around that
full-size descendant. `swipeLeading` and `swipeTrailing` list descendant press
control ids, outermost first. Each action uses the control's accessible name,
background colour and authored icon; `destructive=true` maps to the native
destructive role. A completed action invokes the same live control id once.
The first-child icon snapshot applies that child’s own affine transform to its
image bounds and drawing context, preserving authored scale and rotation. Action
colors and raster icons resolve appearance when opened; an already revealed
action does not yet refresh its appearance.
Input eligibility follows weak references to the original ancestor chain captured before mounting,
including authored visibility, disabled state and inert containment. UIKit
temporarily disables the cell while its action callback runs; that cell is not
an authored ancestor and cannot suppress the action. Restoring a projection
restores the original scroll visibility; session reset releases all projections.
Disabled, hidden or inert controls do not become actions; when the first
configured control is disabled, another control does not silently acquire the
full-swipe gesture. UIKit owns recognition, progress, reversal and release.

The parent authored scroll view retains vertical scrolling. The kernel retains
row dimensions. Projection moves the content's wrapper subtree as a unit,
retaining intermediate ancestors and their styles/input restrictions; before
each batch it restores that subtree to its logical parent and frame, then
remounts it after ordinary updates and navigation. Authored fallback action
controls are hidden only during projection. Native cell chrome supplies the
content node's background while it is mounted. Other rows close when a new row
starts editing; a size change or inactive route closes its native surface.
Invalid or ambiguous references, missing accessible names and nonmatching row
dimensions retain the authored fallback with a diagnostic.

Agent inspection observes the native action's public UIKit button and descendant
image, never a private class name or a guessed offset. A revealed action reports
its actual local, viewport, window and screen frame and accessible name. An
unrevealed or ambiguous target reports unavailable native geometry. Agent `tap`
requires a uniquely resolved, visible, unobscured button and invokes UIKit's
control action; it reports `host-activation`, not finger delivery. The native
button's accessible name is attached during presentation, not by inspection.

For a named button used as the row content, the native cell exposes that
button's declared accessible name, identifier and button/disabled traits, and
forwards accessibility activation to the live authored control. Without this
projection, UIKit's cells around custom-painted content omitted all six Messages
conversation names from public accessibility snapshots. Ordinary-launch captures
now expose the names; direct public accessibility activation opens the intended
conversation and refuses a retained cell after filtering removes its control.
Eight physical swipe cases and eight ordinary Tapback entry/dismissal pairs pass
with the projection (`/tmp/messages-swipe-accessibility/verification.json`).
This verifies the named-button boundary, not a complete VoiceOver interaction.

**Horizontal scroll snap** (2026-09-09, Messages): the admitted CSS subset is
`scroll-snap-type: none | x mandatory` and `scroll-snap-align: none | start`.
The browser executes these as CSS. UIKit's `scrollViewWillEndDragging` finds
the nearest captured start position to its projected destination and adjusts
`targetContentOffset`; UIKit owns dragging and deceleration, using its fast
rate for mandatory snapping (normal otherwise). Oversized snap
areas remain freely scrollable while they cover the viewport; nested scroll
containers capture their own snap areas. This first native binding handles
touch release, not programmatic re-snapping or layout-change re-snapping.
AppKit and Linux snapping remain unimplemented. Messages' single resting
position and vertical-scroll coexistence have been driven on web and iOS;
matching Messages' exact settling curve remains open.

**Scroll events** (2026-09-09, Messages): a declared `scroll` handler receives
`scrollLeft` and `scrollTop` as two numbers. UIKit delivers tracked/decelerating
user offsets synchronously outside a presenter batch, before painting: deferring
these made authored scroll-linked positions visibly lag the scrolling surface.
A reentrant dispatch or layout-generated change queues one callback on the main
queue and reads the latest actual position after layout. AppKit's clip-bounds
notifications use that queued path. An unchanged position or
a removed view dispatches nothing; events do not bubble. Programmatic offset
changes use the same path. Browser/iOS/macOS drives cover both axes, clamping,
independent resets and unrelated updates. Messages uses this event to highlight
the revealed inbox row and reset the other rows without resetting the active drag,
and to resist timestamp travel while keeping date labels stationary.

**Following the end** (2026-09-09, Messages): `scrollFollowEnd=true` records
whether a scroll view is at its bottom (within one point) before a batch and
restores the new bottom after layout. Above it, iOS records the first visible
descendant, descending into partially visible boxes, and compensates for its
movement in content coordinates. It retains the chosen anchor across batches
until the reader scrolls, as [CSS anchor invalidation](https://drafts.csswg.org/css-scroll-anchoring/#anchor-invalidation)
requires. If that node disappears, later visible candidates captured before the
batch can preserve the reading position. Only when none survives is the old
offset clamped. The top stays at zero. A browser/iPhone fixture covers deletion,
hiding, restoration, manual scrolling and empty content; the chat drive retains
the next bubble and active draft when the clipped top message is deleted.
An inactive navigation route may gain height when the foreground route hides
the keyboard. If that clamps an unpinned reader, iOS retains the intended offset
until the returning viewport can accommodate it. This covers both a detached
route and one still attached during a push animation. A new drag or explicit
vertical scroll assignment discards that saved intent. Anchoring remembers the
offset UIKit actually stored, so its fractional quantization cannot masquerade
as a user scroll. Messages contact-details Back now preserves the reading
position with the keyboard restored, including ordinary-launch button returns
and cancelled/completed native swipes; top/end following remains intact.
macOS retains the numeric offset. Explicit `scrollTop`
assignments apply afterwards and win. Web’s end-following code lets the browser
anchor an unpinned reader. A shared fixture first demonstrated a 100-point jump
on both hosts, then verified stable content after growth and shrink above the
reader, plus explicit and automatic end following. This remains an opt-in
application policy, not the full CSS anchoring selection/suppression algorithm. The iOS transcript may also
request `keyboardDismissMode="interactive"`; UIKit owns the drag and dismissal.
Zero-duration keyboard frame changes during that drag bypass the focus-switch
debounce. Notifications omit intermediate interactive frames, so a zero-size
dependent on `UIKeyboardLayoutGuide` requests layout as the guide moves; while
dragging, the viewport uses that local guide edge. Ordinary show/hide changes
keep the keyboard notification's duration/curve. Real Simulator dragging has
verified that the composer remains adjacent to the keyboard during the gesture.

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
(`canCancelContentTouches`) — scroll always wins. `contextmenu` and `dblclick`
(2026-09-09, Messages) are a UIKit long-press recognizer and two-tap recognizer;
only nodes declaring the handler install one, and removed handlers remove their
recognizer. No app gesture arena is introduced. The browser receives the DOM
events; AppKit secondary/double clicks deliver the same payload-free actions.
The events beyond press
and change (§5's list): `hover` is a `UIHoverGestureRecognizer`, so a pointer
hovers and a finger never does; `focus`/`blur` are first-responder changes (a
field's begin/end editing; a node with such a handler `canBecomeFirstResponder`
and takes it on touch-up); `key` is `pressesBegan`'s `UIKey` by web name, or
inside a text field `textFieldShouldReturn` → `Enter` and
`deleteBackward` → `Backspace`, including an empty field. Backspace reaches
the nearest authored key handler before UIKit performs its normal deletion;
the iOS agent calls that same editor method. Typed characters are `change`
(§5's deviation). Enter is also a `submit` handler's event and
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
Weird Castle bar found). If removing a focused view announces a keyboard change
synchronously during a presenter batch, that change waits until the batch ends.
Otherwise the older batch can overwrite the newer viewport frames while the
runtime already believes them applied. The deferred change retains the keyboard's
animation duration and curve; it is independent of the removed view's lifetime.
The Messages forwarding-cancel/Back drive verifies the inbox's painted search
position returns to its original full-height position.
**Authored inertness (2026-09-11, LLP 1035.001 D3):** a node becoming inert
ends editing within its subtree and excludes that subtree from UIKit input and
accessibility. Direct activation checks the entire ancestor chain before choosing
an action; editor delegates also refuse renewed editing. Clearing the prop
restores participation without replacing the nodes or draft. Public-XCTest input
and accessibility-tree checks verify this on iOS. A modal confirmation escapes
inert ancestors and keeps its issued presentation when its invoker becomes inert;
its own action eligibility and identity checks remain. AppKit's existing focus
checks do not establish complete subtree input/accessibility support. Evidence:
`/tmp/messages-inert-ownership/verification.json`.

**A focus moving from one field to another** comes
as a burst of `keyboardWillChangeFrame`s with no duration, over a few
turns — the height jittering between the two keyboards (335, 308, 335 on
the simulator; the email keyboard and the default) — and laying out for
each flashed the page (the second bug it found). No-duration changes now
wait 80 ms for the last of them (`keyboardDebounce`), which usually
changes nothing. The agent's `clock settle` waits for this queued resize in
both timing modes before claiming a settled viewport (LLP 1035.003 D5).
An animated change — the show, the hide — is applied at
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

**A contact on the simulator** (LLP 1035.003 §3 candidate 1, 2026-09-10):
the app synthesizes no touch. The driver posts a real mouse into the
Simulator's window (`host/apple/pointer.swift`), calibrated by hovering at
two desktop points and reading where the viewport saw the pointer
(`layout.pointer`, a hover recognizer the presenter installs only under the
agent), and refuses — naming the covering app — when the Simulator's window
is not the topmost at the point. `layout` also reports `screen` (the
device's size and scale, and the viewport's origin on it).

**The agent (LLP 1012) on iOS.** A simulator app has no stdin, so
`EXACT_AGENT=1` with `EXACT_AGENT_SOCKET=<path>` listens on a Unix socket
(under 104 bytes; the driver makes it in the temp dir) and speaks the same
JSON lines; `ready` carries the app's pid. `layout` is the viewport's
content space less its offset, transforms carried by UIKit's `convert`
(the motion fixture's 75 and 86.55 hold). `layout` with an `id`
(2026-09-09, LLP 1035.002 D1) adds `node`: the runner's rows and their
sources (the library's `node` message) merged with what UIKit knows — the
box in the viewport, the window and the screen's coordinate space, the
capture scale, the scroll chain from the viewport in, the `overflow` and
`clip-path` clippers above it, `hidden`/`inert`/`inViewport`/`clipped`
from the actual superview chain (a sheet's inert source reads `inert`),
and what was mounted: the view class, an inline run's paragraph, the
editor and whether it is first responder, the material, the containing
controller and the route key (`NavigationHost.routeKey`). macOS reports
the same with AppKit's window and screen spaces flipped to y-down and
`inert` false (AppKit has none). A stale id is refused by name. `state`
(2026-09-10, LLP 1035.002 D2, `stateSections` in `AgentIOS.swift` /
`AgentMac.swift`, appended to the library's reply by `Agent.swift`) adds what
UIKit knows: `focus` — the first responder's node and its editor, the
responder's class, and a focus the sheet is still holding for its
presentation (`ModalHost.pendingFocusTarget`); `keyboard` — visible, the
overlap, the keyboard's top edge and the layout guide in the viewport's space,
the resize policy, an interactive dismissal in flight; `navigation` — the
route the root names, `UINavigationController`'s stack by key
(`NavigationHost.observation`), the presentation and its close policy, and
the transition's phase (`in-progress` while UIKit animates, `completed` after
a pop that pressed Back, `cancelled` after an interactive pop returned,
`idle` otherwise). macOS reports the first responder through a field's
editor, `keyboard.visible` false, the stack as `NavigationRules.stack`'s
prefix and `idle`. Every reply the host answers itself (`layout`, `tap`,
`type`, `clock`, `screenshot`) is tagged `epoch`/`incarnation`/`clock` through
the runner's `tags` message (D3, `Agent.tagged`). **`tap` is the one declared
deviation from §1's contract**: UIKit offers no public touch synthesis, so
a tap hit-tests through the window (UIKit's own, placements included) and
delivers the press by the responder-chain rule a touch gets
(`NodeView.activate`, which VoiceOver's `accessibilityActivate` also uses);
a wheel applies LLP 1010's chaining rule from the hit view up. `type` is
`becomeFirstResponder`, `selectAll`, `insertText` — one `editingChanged`
with the whole value. `screenshot` is `drawHierarchy(afterScreenUpdates:)`
at the screen's scale in the standard (8-bit sRGB) range — a wide-color
screen would otherwise yield a 16-bit PNG — and sees Metal. By default it
captures the session viewport. `window: true` captures the containing UIWindow,
including native confirmations and other embedded sessions; separate system
windows such as the keyboard are outside that capture. This distinction landed
with LLP 1021 D2's confirmation integration (2026-09-10). The driver's `ios` carrier (`scripts/agent.mjs`
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
(`appleArtifacts(app, { destination: 'ios' }).bundle`, `iPhoneOS` in its plist, the app's `assets/`
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

**The contract behind the projection** (LLP 1035.001 slice 1, 2026-09-10):
the rules the navigation, modal, keyboard and focus code below applies are
pure functions in `NavigationRules.swift`, held by
`ExactKitTests/NavigationRulesTests.swift`: the stack is the prefix through
the route the root's key names, and a key naming none leaves the stack alone
and journals once; a completed transition presses the Back control exactly
once, on a key change only (a cancelled swipe shows the key the root still
names; a programmatic Back already moved it); the Back control is resolved
at use, by HTML `id`, lowest view id among enabled press controls, never
captured at a gesture's start; a pop begins only with a stack to pop, no
transition in flight, no sheet, a Back control and no context preview, and a
pan past the 20-point edge yields to a `swiperight` node under its start;
`closedby="none"` refuses the sheet gesture; deferred geometry replays frames
before contents, ids ascending; a keyboard notification is a session's only
for its own editor or while it holds an inset it applied; the viewport
freeze is for an initially interactive pop, never a sheet; a `focus` that
cannot be delivered names its reason. Refusals are journal lines through
`exact_log` (LLP 1012 §3). A blur is the session's viewport's, never the
window's, so two sessions in one window keep their editors apart (the
two-session smoke's step 3b). `EXACT_AGENT_TIMING=platform` keeps the
animations natural under the agent (LLP 1035.003 D5).

**Modal confirmation** (LLP 1021 D2, 2026-09-11): MenuHost also projects
HTML `dialog` invoked through `commandfor` and `command="show-modal"` into its
existing session-owned confirmation. Only the one-action/closing-Cancel grammar
with `closedby="any"` is admitted. Open state stays native; cancellation retains
the presenting editor, and completion revalidates the source, route and action.
Reload, unmount and destruction retire that session's presentation. Messages
physical checks and the two-session dialog fixture pass on the rebuilt host
(`/tmp/messages-modal-confirmation/`). AppKit hides the declared dialog and
journals that its projection is unsupported.

**Native navigation** (2026-09-09, Messages): the first root's `navigationKey`
and `navigationBack` project its keyed direct child routes into a UIKit
navigation controller, contained by the nearest owning view controller. The
kernel still lays out the same Contract views. Generic child-list reconciliation
leaves retained keyed routes in their native controllers. Removing the root's
navigation declaration returns surviving route content before detaching its
controller, preserving the mounted editor and selection in the UIKit fixture
(`/tmp/messages-route-ownership/`). UIKit owns push/pop animation
and interactive pop recognition, progress, cancellation and scroll arbitration.
Only an interactive pop from the still-live, still-selected source route presses
its currently enabled named back control. A programmatic completion never presses
Back; a cancelled swipe leaves Contract slots, drafts and mounted views intact.
Callbacks from a retired navigation controller or a superseded shown controller
are ignored. Intent received during a native transition is projected when it ends.
An interactive sheet's completed callback returns to UIKit before restoring its
viewport and releasing its modal-navigation slot. Only the same live, selected
source may then invoke Back. Keeping that route selected or requesting another
sheet rebuilds presentation ownership; reusing the old slot previously moved
the primary view under a sheet with the wrong controller parent. The second
candidate passes retained/replacement-route and mid-gesture permission cases
(`/tmp/messages-dismissal-owner/`, LLP 1035.001 D2).
This prevents a rapid Back → Compose from cancelling the new sheet; the iOS drive
passes twice with recipient focus, keyboard, saved drafts and physical Back gestures
(`/tmp/messages-navigation-completion/`). Both gesture
recognizers refuse to begin when the named Back control is missing or disabled;
the completion callback uses that same enabled-control lookup, scoped to the
selected route. A retained inactive route's control cannot authorize a pop or
sheet dismissal; sheets check permission at sync and through UIKit's delegate
(`/tmp/messages-back-owner/`, LLP 1035.001 D1). Messages disables
its conversation Back while the forwarding sheet is open. Previously an edge
swipe could pop the source route underneath that sheet. The presenter
freezes outgoing pixels only when a button's action deletes its route before
UIKit can animate it. In agent mode, programmatic navigation settles immediately;
physical gesture verification must also run outside that mode. There is no
Exact transition-progress value, gesture arena or native route source override.

Unmounting ExactView retires its native sheet and navigation owners without Back,
preserving the runner and surviving route nodes. Offscreen updates create no
native presentation; remount installs owners from current intent. Focus requested
offscreen stays bound to its actual editor and waits from window attachment
through controller installation. Physical sheet-drag unmount/destruction and an
offscreen route replacement verify retained state, remount, focus and independent
session use (`/tmp/messages-unmounted-owner/`, LLP 1031 D3). A subsequent
same-window transfer to a different UIKit controller also passes with the current
host, including a physical sheet drag and offscreen replacement with queued focus
(`/tmp/messages-reparent-owner/`, 2026-09-11). The old parent loses its navigation
child and the new parent owns it. Unmount ends current focus; explicit queued
focus reaches the new editor. Continuous editing and cross-window moves are not
established by these checks.

**Gesture retirement (2026-09-10, LLP 1035.001 D8):** removing a route can
cancel a child's recognizer before that child's destroy operation reaches Swift.
The kernel has already removed the target. A drag's motion update and final
Reply dispatch therefore use the presenter's existing post-batch event queue,
with the originating view's identity checked at delivery. A destroyed or replaced
view receives neither. The production held-reply → Back reproduction previously
reported `drag target is gone`; it now completes without that write. Public XCTest
touches also verify a short release returning to rest and a completed Reply
dispatching once, with draft and keyboard retained (`/tmp/messages-gesture-retirement/`).

**Initial ownership and refits (2026-09-10, LLP 1035.001 D8/D9):** the presenter
installs the initial native owner after logical mounting and the root's frame,
before applying child geometry. Initial installation resolves the controller's
layout, but does not present a sheet or flush focus. Initial and later updates
share the route projection; containment is not a second route model. A refit
requested while a batch applies is coalesced onto the next main-queue turn,
avoiding a resize from an incompletely mounted tree. An initially selected modal
leaves the primary route mounted until its first draw; the existing post-draw
`dataReady` batch then presents the sheet. The production keyboard/Back drive and
two-session reload/destruction fixture pass (`/tmp/messages-owner-install/`).
Native title/header authoring and its CSS geometry mapping remain D9 work.

**Presented routes** (2026-09-09; fullscreen extended 2026-09-11, LLP 1035.001 D4):
`navigationPresentation="modal"` uses UIKit's large page sheet; `fullscreen`
uses `.overFullScreen`. Every boundary in the selected route prefix retains its
own navigation controller, including when a later push or nested sheet is active.
`navigationSource` on a fullscreen route names an HTML id in its preceding route.
On iOS 18+, UIKit's public zoom transition resolves that live source at use;
removed or unmounted sources return nil, replacements resolve by their new identity.
Without the prop, UIKit uses its ordinary fullscreen transition. The existing
Back and `closedby` rules govern dismissal. R4 native owner/gesture verification
and R5 browser verification are recorded under `/tmp/messages-fullscreen/`.
Navigation tracks route transitions, excluding enclosing presentation appearance
callbacks that have no corresponding route completion. The owning `ExactView`
stays in its embedder. The presenting navigation controller keeps its parent,
stack, bar and scroll relationship; its whole view stays in the owning `ExactView`.
A separate navigation controller owns each presented segment and enters its owner with
the session viewport, using the controller containment callbacks. Close restores
the presenting view to its original container and retires that presentation’s navigation
controller. Nested presentations retire from the top down. The source retains its pre-sheet geometry; frame/content updates caused
by the sheet's viewport are deferred and replayed before restoring normal layout.
Scroll assignments on that source wait with its geometry and apply after the
deferred frames and content extents. In particular, a scroll created while the
sheet is open must not consume its initial offset against a zero-size extent
(Messages' new inbox row, verified on iOS and the browser).
Style updates and UIKit appearance propagation still reach its actual controls,
fixing the stale light/dark background caused by the former screenshot. Source
interaction and accessibility are disabled while covered and restored on close.
Forwarding's backdrop shows selection already closed. UIKit owns presentation,
interactive dismissal and spring-back.
An outgoing sheet remains native-owned until UIKit's dismissal completion, even
if its logical route or runtime is replaced. New route projection waits for that
retirement and resumes from the current tree; an in-flight presentation finishes
before a requested dismissal starts. Reset cancels old focus targets, not the
native completion still needed to release the owner. Completion is identity-bound,
does not reconcile a destroyed session, and a retained controller holds its host
weakly. `state.navigation.transition` and `clock settle` include that retirement
and pending projection. Focus readiness is separate: an already-mounted destination
editor can focus while the outgoing sheet dismisses. Timed reopening, reload during
dismissal/presentation, two-session destruction and nine physical sheet cases pass
(`/tmp/messages-modal-retirement/`, LLP 1035.001 D3). The editor-retirement repair
below subsequently addresses first-Send keyboard continuity.
The sheet's local keyboard guide supplies its available layout height; the
horizontal-pop viewport freeze does not apply to modal transitions. Focus
commands during navigation, sheet presentation or native window mounting share the presenter's single
pending slot, bound to the actual target node. Delivery waits until controller installation and sheet presentation finish;
then mounting flushes it. Replacing the target or route cancels it. A later focus replaces the pending intent, and native
owner reset clears it. The current production Messages drive passes rapid
Back → Compose twice and a local send; the two-session host also destroys the
presenting session with a filled sheet open while the other stays editable
(`/tmp/messages-modal-owners/`). `state.focus.pending` reports its target and wait reason.
An unmounted focus target stays in that identity-bound slot even before UIKit
announces the next navigation transition. A window-mount callback and the end of
the outermost batch retry delivery; a partially applied batch cannot focus it.
The production physical New Message Send now focuses its conversation composer
automatically. Reload and destruction with focus queued preserve the other
session's editor (`/tmp/messages-send-focus/`). Selection leaving a sheet now
installs the selected primary stack before starting dismissal. The conversation
is mounted behind the departing sheet, removing the earlier inbox flash and
subsequent push. Two real-touch first-Send drives focus the composer and send once;
Back/sheet gestures, rapid Compose, reload and two-session destruction pass
(`/tmp/messages-compose-handoff/`). Those runs still lacked keyboard continuity:
three hidden samples span 247/204 ms in those runs, and recorded video shows the
keyboard dropping and returning. This does not establish native first-Send motion.

**Editor retirement (2026-09-10, LLP 1035.001 D3):** when a modal route is
destroyed, its child navigation hierarchy stays in the retiring sheet. Each
destroyed node is forgotten and removed from the live map immediately; its view
stays mounted under the frozen outgoing surface until native dismissal completes.
The retained hierarchy is hidden from accessibility. Disabling its interaction
would itself resign its editor, so event retirement uses the existing presenter
disconnect. The viewport returns to its primary owner independently. Dismissal
starts on the next main-queue turn, after the batch's focus commands transfer the
first responder to the mounted destination. Only the retired controller is held
for cleanup; a disappeared host still dismisses that controller without reaching
into another session. First/ordinary Send, physical Back/sheet gestures and
two-session interruption checks pass. No hidden-keyboard samples or vertical
keyboard movement appear in the uninstrumented first-Send check
(`/tmp/messages-editor-retirement/`); native first-Send reference motion is still
unestablished.
The modal controller and route use the system's secondary grouped surface
behind transparent authored corners. UIKit supplies the dimming outside it;
painting an app backdrop inside this surface had left a dark corner seam.

The route's `closedby="none"` prevents platform close requests through
`isModalInPresentation`; `closerequest` permits the sheet gesture. Explicit
Close still invokes the authored control in either state. A completed gesture
invokes the root's named Back control, while a reversal leaves the route and its
inputs mounted. Messages allows gesture dismissal for an empty new draft and
resists it for populated forwarding, following the native fixture captures.
The browser projects the same modal route and close-request policy (LLP 1007).
There is no Exact gesture-progress value or second layout engine.

For an initially interactive navigation transition, keyboard notifications do
not resize the viewport until UIKit finishes or cancels: its keyboard is moving
sideways with the route. A retained native first-responder editor uses its
container's keyboard guide, including after cancellation or rotation. The latter
can announce a notification edge four points above the guide; the guide owns the
occupied geometry. Keyboard visibility and top-edge inspection use that active guide,
scoped to the session's editor or retained inset; notifications supply animation
timing. The production XCTest drive retains inset 335 and viewport height 539
through a held and cancelled multiline draft, without the previous zero-inset
sample. Completion still saves the draft and hides the keyboard; populated sheets
and two-session focus/destruction also pass (`/tmp/messages-keyboard-guide/`).
The rotation drive retains the draft and first responder in both landscape
orientations: viewport 874×198 and composer y=141.667, matching native Messages'
measured keyboard edge and composer position. Return to portrait restores 402×539
and y=482.667. Initial composer and sheet keyboard captures remain geometrically
stable in platform and agent timing (`/tmp/messages-rotation/`). This does not
establish compact-header or keyboard-control parity. A competing horizontal transcript scroll
still takes rightward gestures in its content area; that arbitration remains an
open limitation, not a completed iMessage parity claim.

**Emoji selection** (2026-09-09, Messages): an `input` with `emojiPicker=true`
uses the existing text field with an enabled emoji `textInputMode` preferred on
iOS. The system owns Search Emoji and its results. The delegate and the
editing-changed path (also used by driver insertion) share `EmojiSelection`:
one accepted emoji grapheme emits `change`, ordinary text and multi-grapheme
input do not, and the field remains empty. AppKit applies the same selection
gate to field changes; it does not automatically open Character Viewer.
The policy changes reload a focused iOS input's views. UIKit still permits
switching keyboard modes: this public field retains ABC/dictation controls
that native Messages' reaction grid omits. Messages keeps its composer draft
separate and captures its entry focus state. Close restores the reaction strip
without actions; double-tap entry restores a previously active composer,
long-press entry keeps it closed until selection or final dismissal.

**Context previews** (2026-09-09, Messages): `contextTarget` on a preview node
references the original content's HTML `id`. After layout and navigation
projection, the presenter finds the preview's nearest enclosing absolute panel.
iOS magnifies the preview without reflow by 15%, capped at 26 added points on
the larger dimension. The wide public UIKit fixtures and 12/24-line Messages
captures are recorded in `apps/messages/README.md`. It
uses scale 1 instead when the preview declares `contextMagnify=false`, as
Messages does for badge and double-tap entry. Absence or true retains the
default enlargement. Both modes share placement, clamping, focus retention and
the authored transform; selecting a mode does not change kernel geometry. It
preserves the source's outside edge and vertical center. Top-aligned immediate
side siblings move horizontally by the corresponding enlarged-edge displacement,
so an authored side control keeps its gap without changing balloon measurement.
Following siblings at
each enclosing level move by half the added height, counteracting the panel's
upward shift so receipts keep their source-relative position (a direct Messages
capture confirms that the receipt stays still while its balloon enlarges).
It then clamps the complete projected panel, including the farther extent of
the enlarged preview or following controls, inside the
safe viewport above the keyboard, intersected with the panel parent's bounds.
If the entire panel is too tall, the trailing sibling group outside the preview's
branch moves together to keep its controls inside that region. It may overlap
the preview; native Messages does this for the 24-line sample. The receipt
retains its source-relative position, including the overlap observed on tall
native balloons. Kernel boxes and text measurement remain unchanged.
Messages uses that authored region to reserve the participant popover and its
24-point gap above the palette; AppKit's alignment uses the same region.
The projection composes with the authored
transform and is recomputed from unprojected geometry each batch; it never
changes kernel frames or text measurement. AppKit aligns the nested preview
without magnification. The panel's controls and preview are
ordinary Contract content. Long press and double tap continue through the
platform recognizers; outside dismissal is a declared press control. Navigation
swipes are disabled while a context preview is present. This custom presentation
does not claim UIKit's final pixel rounding/clipping, preview animation or the
complete emoji picker of Messages.

iOS captures the source's rectangle in session viewport coordinates before
entry-batch commands can dismiss the keyboard. It retains that entry rectangle
through height changes and recaptures on width changes. The source's vertical
scroll contents translate with the preview's retained position and clamp. A
scroll region that itself moved, such as the centered focused reply thread,
retains its entry clip position; the remaining displacement applies to its
contents. This keeps neighboring messages visible through keyboard dismissal.
The panel must be outside that content subtree for the source projection to
apply. Projection resets before each batch's calculation and after dismissal;
no authored scroll offset or kernel frame is written. The cache is bounded by
the mounted source identity, preview lifetime and target value. AppKit retains
its existing live-source alignment.

Context actions retain the active editor's focus. UIKit and the browser do not
blur it before reaction/reply dispatch, because keyboard dismissal would move
an anchored panel away from the same tap. The native touch path and agent's hit
path share this rule. `retainFocus` on a node or ancestor also keeps unhandled
touches from the scroll container’s blank-ground keyboard dismissal, including
the first tap of a double-tap recognizer and a swipe released on a sheet header. Actual blank-ground taps still dismiss.
An authored `focus` command can deliberately transfer focus to a menu with a
key handler; Messages uses this for long-press while double-tap retains its
editor. No recognition or scroll arbitration moves into the runner.
Visible overflow also participates in iOS hit testing:
children painted beyond a row's width can be selected after scrolling them
into view, while each non-visible overflow axis still clips hit participation.

The Messages inline reply surface now consumes `backgroundMaterial="ultra-thin"`
(LLP 1001): a non-interactive `UIVisualEffectView` underneath authored children.
The presenter restores that underlay order after child-list reconciliation;
removing the prop removes the effect. UIKit supplies its appearance changes.
A material does not change focus, navigation, or hit-testing policy. The
`glass` value supplies an iOS 26 `UIGlassEffect` with the authored uniform corner
radius, with ultra-thin blur as the pre-26 fallback. A changed material value
replaces the effect; later radius changes update its corner configuration. Glass
holds authored children in its `contentView` and enables UIKit’s interactive effect
for a node with an enabled press handler. Material changes move those children
into the replacement container. An actual held Back press in the Messages
simulator enlarges the glass, and releasing it still dispatches navigation.

Textarea placeholders use UIKit’s placeholder color and repaint when the
field style changes. A root appearance change also refreshes the host canvas
color, including the window area exposed around the keyboard’s rounded corners.

A native press resolves its target before ending the previous editing session.
Keyboard dismissal can resize the viewport synchronously; the release must not
be re-tested at the old window point against the control's new frame. A target
removed by the focus change is still refused. The iOS agent uses the same order and focuses both UITextField and UITextView
when a tap reaches their owning node. Its responder walk stops at the resolved
press target, after considering that target's own focusability, as a native
pressed node handles `touchesEnded` without forwarding it. A containing node's
key handler therefore cannot take focus from an editor retained by a reaction
button. Unhandled taps still traverse ancestors; ordinary blank-ground taps
still end editing. The Insets fixture checks retained-button activation under
a key handler and the editor's actual UIKit first-responder state (2026-09-09).

### Right-swipe actions

`swiperight` is a recognized event (dispatch kind 12), authored by Messages as
`openReplies(message.replyRoot)`. UIKit uses a one-finger `UIPanGestureRecognizer`;
vertical and leftward gestures are left to scrolling. A right drag follows the
bubble, with resistance beyond 64 points; release beyond that threshold invokes
the action, while reversing or cancelling returns without invoking it. Crossing
the threshold requests selection feedback (device feel remains unverified).
The navigation content-pop recognizer yields when the hit bubble owns this
right swipe; the first 20 window points remain available for edge navigation.

`exact_drag_x` holds translate through the existing motion engine, leaving the
kernel target and layout unchanged. Release observes the authored target with
the gesture velocity. Messages currently authors a 180ms ease-out return;
this is not a claim that its curve and threshold match Messages. Browser
pointer events hold CSS translate and return under the same CSS transition.
Its `pointercancel` ends the observation when the browser takes scrolling.
Direct children marked `swipeIndicator` also hold their authored opacity and
scale toward 1 with the offset (LLP 1001). Release observes both authored targets
through the same engine. Messages draws the reply arrow in Contract;
neither host adds a view or a separate animation executor for the indicator.
The drag remains host presentation state; only the completed action enters
Contract. No gesture arena or per-frame app code is introduced.

### Typed line-height seam (LLP 1035.000.000, 2026-09-11)

Apple C ABI v5 adds `has_line_height` to `ExactTextRun` and a paragraph
`strut` run to `ExactMeasureRequest`. A false flag means natural metrics;
a true flag with zero is an explicit zero box. The style dictionary retains
ratios as numbers, lengths as `"24px"`, and `"normal"`; Swift resolves each
ratio using the receiving node's computed font, matching kernel projection.
CoreText measurement and painting include the paragraph strut and the
ascent/descent extrema of only the runs on each line. Normal line height
includes the shaped fallback font's metrics; explicit lengths size the
authored inline box while fallback glyph ink can overflow. Native textarea
paragraph attributes receive the same resolved length; TextKit's zero
min/max sentinel is bypassed with its smallest positive line-height multiple
for an explicit zero (below driver precision).

The same DejaVu face at 20/40px with inherited fixed 20px line-height gives
26.92px from CoreText and 27px from Chrome's rounded font metrics. This is
the retained D6 mixed-baseline rounding investigation, separate from explicit
single-font boxes and the smaller-run paragraph minimum, which match within
0.02 logical units. Baseline raster snapping remains unchanged.
