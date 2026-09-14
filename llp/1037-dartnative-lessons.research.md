# LLP 1037: DartNative lessons — what a borrow-the-platform peer's failures predict for exact2

**Type:** Research
**Status:** Draft
**Systems:** Apple host (navigation ownership, keyboard, native view lifetime, diagnostics), Kernel + Runner (list windowing, the scroll seam), Image (decoded-bitmap bounds), GPU module (the optional artifact), Update store (launch-failure trace)
**Author:** Claude (Opus 5) for Charlie Cheever
**Date:** 2026-09-14
**Related:** LLP 0559 (the Flutter comparable — the predecessor's research, opposite bet on pixels); LLP 1035.001 D1–D5/D9/D10 (native interaction ownership; the header-shaped route); LLP 1008 §9 (iOS keyboard, interactive dismissal); LLP 1010 §"not in v1" (windowing seam); LLP 1011 §6 (image loading gaps); LLP 1009 D2 (GPU as a second artifact); LLP 1012 (the journal ring and `logs`); LLP 1026 crash recovery; LLP 1015 (Linux statically links vello/wgpu); `rules/NOT-DOING.md` §Components (windowed list); `apps/messages`. External: DartNative (dartnative.com, github.com/DartNative/dartnative — not the unrelated `dart-native/dart_native`).

## Summary

DartNative is Flutter-style Dart widgets compiled AOT and driving UIKit/Android
views through synchronous FFI on the main thread, with Yoga for layout. It
shipped a first public preview on 2026-07-31 and a second on 2026-09-08. It
borrows the platform where exact2 does. Its changelog is therefore a free
list of failures that exact2's Apple host could also hit.

Of the five lessons drawn from it, exact2 is **aligned on two**: GPU as an
optional artifact, and keyboard show/hide in the system animation.

It is **at risk on three**:

- **The navigation bar.** The iOS host hides the system bar and every route
  paints its own header. That is exactly the state DartNative's 2026-09-08
  release fixed. LLP 1035.001 D9's proposed remedy shows the bar per route,
  which is exactly the toggle that caused their blur flash.
- **Bounded list memory.** Windowing is inert. Every node is a live native
  view, decoded images have no bound, and nothing measures memory.
- **Diagnosing a failed launch.** Nothing persists a log or breadcrumb, panics
  abort with stripped symbols, and the update store records *that* a bundle
  failed but not *why*.

There is one compounding risk. Interactive keyboard dismissal re-lays out the
whole session on every drag frame. Without windowing, that cost scales with the
length of the transcript.

## 1. Provenance

**DartNative-side claims** come from a 2026-09-14 reading of the sources below,
re-fetched the same day to confirm them. The fetch tool summarizes pages, so
quoted strings are close paraphrases, not guaranteed verbatim. Nobody ran its
demos; this is architecture, not a performance verdict.

- `dartnative.com` (home, `/changelog/`, `/docs/debugging/logging/`, `/license/`)
- `github.com/DartNative/dartnative`: `README.md`, `docs/architecture.md`,
  `docs/widgets.md`, `CHANGELOG.md`

**exact2-side claims** come from reading the tree at `39fa4cb` plus the staged
index, on the same day. Each claim is marked *[LLP]* (specified), *[code]*
(implemented) or *[measured]* (verified by a run). File and line references
were spot-checked.

## 2. The comparable

- **What it is.** "All Dart code runs on the platform's main thread. setState →
  diff → UIKit in one synchronous call stack" (README). Layout is Yoga.
- **"No engine, no bridge" is loose.** The homepage says "No Impeller. No Skia.
  No bridge." The README says what remains: "the Flutter engine with its rendering
  stack removed … the Dart VM in AOT mode, `dart:ffi`, and the Flutter
  toolchain." So there is no *rendering* engine, but there is an engine.
- **Scope.** iOS and Android only. Nothing mentions web or desktop.
- **Source.** The framework and its first-party plugins are closed source. The
  license commits to publishing source under BSD-3 within 90 days of end-of-life,
  or after 12 months without updates.

**Differences that matter here:**

- Its authoring contract is Flutter compatibility; exact2's is CSS semantics
  with the web as oracle.
- Its app code runs on the main thread; exact2's Contract runner is data plus
  bounded modules.
- Its layout is Yoga, which has the RN defaults `CLAUDE.md` rejects.

None of these is a lesson to copy. Everything below is about *native
interaction behaviour*, where the two projects face the same UIKit.

## 3. Findings

### F1 — One system navigation bar for the whole stack. Verdict: at risk, same shape as their bug

**Theirs.** The 2026-09-08 changelog entry "iOS 26 Fixes: Navigation Bars…"
records three fixes:

- *The back button stays put.* "On iOS 26 the system navigation bar is now one
  bar for the whole stack, as it is in a UIKit or SwiftUI app." A screen that
  draws its own bar leaves the system bar empty. Push and pop use the platform
  transition, so Back "fades in place and the title slides" instead of riding
  along with the page.
- *No flash after a pop.* Content "un-blurred under the collapsed bar for one
  frame" going back to a large-title screen. The fix: "The bar is no longer
  hidden after a pop, so the blur is continuous."
- *Taps.* A custom-bar screen could stop answering taps after navigation. The
  system bar now passes those touches through.

The general lesson: a native controller alone is not native behaviour. The
controller's *containment* also has to be native — one bar owned by the
stack, never toggled per screen.

**Ours: aligned on the controller.**

- *[code]* Routes are `RouteController`s in a real `UINavigationController`
  (`host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:125-134`).
- *[code]* UIKit owns the interactive pop recognizers, including iOS 26's
  `interactiveContentPopGestureRecognizer` (`:137-138`). A completed pop
  dispatches Back exactly once.
- *[code]* Pure rules are tested (`ExactKitTests/NavigationRulesTests.swift`).
- *[measured]* Real-touch cancel and complete were driven by XCTest (LLP 1035.001).

**Ours: at risk on the bar.**

- *[code]* The bar is hidden on the primary stack and on sheets:
  `nav.setNavigationBarHidden(true, animated: false)` (`NavigationIOS.swift:126`,
  `:192`).
- *[code]* Messages authors a per-route header row with a glass Back button
  (`apps/messages/app.contract:601-602`). Glass is a `UIVisualEffectView` per
  button (`NodeViewIOS.swift:871-910`), not bar material.
- *Consequence.* Back, title and material all slide with the page. UIKit's
  back-button choreography never runs, and there is no continuous scroll-edge
  blur. This is DartNative's pre-2026-09-08 state.
- *[code]* **Button Back pops a frozen snapshot.** A `when`-branch route is
  destroyed before UIKit's pop begins, so `freeze()` (`NavigationIOS.swift:27-34`)
  overlays `snapshotView(afterScreenUpdates: false)`. The outgoing page,
  glass included, animates as a still picture. An interactive swipe uses live
  views.

**The trap in the proposed fix.**

- *[LLP]* LLP 1035.001 D9 (`:799-822`) already reached the right diagnosis for
  large titles: "the behaviour belongs to a real navigation bar with a real
  title."
- *[LLP]* Its shape is a *header-shaped route*: "On iOS the host shows the
  navigation bar" for routes whose first child is a `header`. In a stack that
  mixes header-shaped and bare routes, that means showing and hiding the bar
  across push and pop. That is precisely the toggle DartNative removed to stop
  the blur flash.
- *[LLP]* D9 frames the bar as per-route title intent. Whether header buttons
  become bar items is an open question (`:820-822`). No LLP states the
  stack-level invariant.
- D9's prototype stopped after three rounds on scroll-offset mapping
  (`:900-911`), so nothing has shipped yet.

**What avoids re-learning it.** Before D9's next round, make this the invariant:
*the stack owns one visible system bar for its whole life; a route fills it
(title, Back, bar items by role) or leaves it empty; the bar is never hidden or
shown across a transition.* Add to D9's acceptance:

- a pop from a bare route to a large-title route and the reverse, checking
  scroll-edge blur continuity frame by frame;
- Back identity across push and pop;
- taps in the bar region after navigation (their third fix).

Button Back should pop live views. D1 already resolves the Back control by id,
so one option is for the host to retain the outgoing route's node until
`didShow`.

### F2 — Keyboard motion in the system animation transaction. Verdict: aligned for show/hide; at risk during interactive dismissal

**Theirs.** Views join "the same Core Animation transaction as the keyboard
itself … with zero per-frame CPU overhead and the exact system curve"
(architecture.md). Avoidance happens without resizing layout. On iOS 26 rotation
the system dismisses and re-presents the keyboard. "Anything driven from
keyboard notifications visibly drops and rises in that gap," so they follow the
keyboard's geometry, not its notifications.

**Ours: aligned on show/hide, focus switches and rotation.**

- *[code]* Show/hide reads the notification's duration and curve and runs one
  layout inside `UIView.animate(… curve << 16 …)` (`PresenterIOS.swift:115-116`,
  `:178`). LLP 1008 says "one layout, nothing per frame."
- *[code]* Zero-duration bursts from focus moving between fields are debounced
  for 80 ms (`PresenterIOS.swift:117-135`).
- *[code]* Rotation with an editor reads `keyboardLayoutGuide` geometry, not the
  notification (`ExactViewIOS.swift:124-127`). That is the same answer they
  reached, and it is checked by an XCTest drive (LLP 1035.001 D5).
- *[code]* During an interactive back-swipe the viewport is frozen
  (`preservesKeyboardViewport`).

**Ours: at risk on the drag.**

- *[code]* Under `interactive-widget="resizes-content"` (Messages,
  `app.contract:494`), a zero-size probe pinned to `keyboardLayoutGuide.topAnchor`
  forces `layoutSubviews` on every drag frame (`ExactViewIOS.swift:31-41`).
- *[code]* Each drag frame then calls `session.resize` (`:150-153`): a kernel
  relayout and op batch per frame.
- *[measured]* LLP 1008 records only that the composer "remains adjacent".
  Nobody measured frame cost.
- *Why it compounds with F3.* The transcript is not windowed, so that
  per-frame relayout covers every message ever loaded.

**What avoids re-learning it.**

- Measure relayout cost per drag frame at 25, 1,000 and 10,000 transcript
  messages on a device before trusting the path.
- If it misses, keep `resizes-content` semantics at gesture end. During the drag,
  move the composer and the transcript's bottom inset (a transform or
  `contentInset`) with no kernel resize: one layout when the drag settles or
  cancels.
- Unmodelled so far: hardware-keyboard attach and detach, and the macOS and
  Linux equivalents (both report no keyboard).

### F3 — Native recycling and bounded memory are different achievements. Verdict: at risk, with neither achieved

**Theirs.** The homepage says memory "stays flat … 25 rows or 25,000."
`docs/widgets.md` qualifies that:

- `ListView.builder` keeps every item as a live `UIView`.
- "Without `keepAliveCount`, `FastList` builds and holds all `itemCount` items
  (O(N) memory)." The default is `null`.

UITableView windowing bounds *native views*. It does not bound *built state*.
A framework can recycle cells and still grow without limit.

**Ours: no windowing, no bound, no measurement.**

- *[LLP]* `List` windowing is inert. "The current seam does not support windowing
  without extension … `List` and `ScrollView` are today the same thing on every
  host" (LLP 1010, `:207-216`). It lists three missing pieces: logical extent,
  origin compensation, and a scroll-offset event.
- *[code]* Apple presenters create a `UIView`/`NSView` for every `create` op
  (`PresenterIOS.swift:432-433`, `PresenterMac.swift:259-264`). Nothing culls
  by visibility, and nothing handles memory warnings.
- *[LLP/code]* Images decode eagerly with `kCGImageSourceShouldCacheImmediately`,
  "no size cap", one bitmap per view until `destroy` (LLP 1011 §6). The only
  bounded host cache is `TextCache` (`Text.swift:69`).
- *[code]* Messages builds every inbox row, each with its own nested swipe
  `scroll` (`app.contract:504-505`), and every transcript message (`:630`). The
  fixture has about 25 records, so none of this has been felt.
- *[measured]* `metrics.mjs --scaling` measures *runner* time at 300, 3,000 and
  10,000 rows. LLP 1005 says: "peak allocation is unmeasured … not a browser or
  phone frame budget."
- *[LLP]* `rules/NOT-DOING.md` §Components frames the windowed list against
  60 fps: "if it misses 60fps, that is a kernel bug." That is the frame-rate half
  only. Their docs show the memory half is a separate bar.

**What avoids re-learning it.** When windowing is designed, count separately and
per host, at N = 25 / 1,000 / 25,000:

- construction time to first frame;
- runner and plan state retained;
- live native views;
- decoded image bytes;
- process footprint.

Materialized views are bounded by the window. Plan and runner state for off-window
rows needs its *own* stated bound (or an explicit "O(N) data, O(window) views"
contract). Decoded images need a cache with eviction independent of view
lifetime. Naming these five numbers before building keeps "we recycle" from
standing in for "memory is flat", the gap DartNative's homepage fell into.

### F4 — Platform drawing by default, GPU as an optional surface. Verdict: aligned on web and Apple

**Theirs.** `CustomPaint` uses Core Graphics or `android.graphics.Canvas`; the
separate `dartnative_skia` package adds a Graphite surface.

**Ours.**

- *[LLP]* LLP 1009 D2 makes the GPU module a second artifact.
- *[code]* On Apple, the dylib is copied only for apps with a GPU crate
  (`host/apple/build.mjs:421-489`) and loaded with `dlopen`
  (`GpuModule.swift:75`).
- *[code]* On the web, `gpu-glue.js` is injected only when a surface is pending
  (`host/web/glue.js:1418-1426`).
- *[code]* No host crate depends on `exact-gpu`.
- *[measured]* Web module 142 KiB (70 KiB gzip); native about 1.7 MiB, loaded
  after first paint (LLP 1009).

**Caveats.**

- Linux does not load the GPU module at all (LLP 1015). It statically links
  vello and wgpu for its base painter, so on that host GPU code is core, not
  optional.
- On Apple, the module ships inside the bundle and is loaded lazily, never
  downloaded separately.

Neither caveat is a DartNative-shaped risk. Both are already declared.

### F5 — Production diagnostics as a product requirement. Verdict: at risk

**Theirs.** The logging docs describe "one stream" merging Dart, Swift and Kotlin
logs. It persists "every session to a file on the device", rotating at 1 MiB
(about 2 MB on disk), with boot breadcrumbs as "cheap one-liners between the
awaits in main()." A failed ordinary launch is diagnosable without a debugger.

**Ours.**

- *[code]* Host logging is plain stderr (`ExactHostIOS/main.swift:22`,
  `ExactUpdates/Updates.swift`). On a device launched from the home screen,
  stderr is lost.
- *[code]* No `os_log`/`Logger`, panic hook, breadcrumb or log file exists under
  `host`, `runner`, `update` or `apps`.
- *[code]* Release sets `panic = "abort"` and `strip = true` (`Cargo.toml:15-20`),
  with no hook to record the panic first. A Rust panic leaves an OS crash report
  with stripped symbols and nothing else.
- *[LLP/code]* The journal is a 4,096-line in-memory ring (LLP 1012,
  `runner/src/runner.rs:263-268`). It is readable only through a live agent
  `logs` connection.
- *[code]* The update store persists a failure *counter* and a `bad` digest list
  (`update/src/store.rs`). The refusal *reason* (`launch_refusal`) and the boot
  note live only in memory and the ring.
- *[LLP]* LLP 1026 promises the failure goes "in the next envelope request's
  headers so the server can see it". *[code]* `update/src/client.rs` sends no
  such header.

**Consequence.** A TestFlight user whose app dies before first pixel, or whose
bundle is demoted, leaves a digest and a count. There is no timestamp, no
reason, no last breadcrumb, and no Rust backtrace. The continuous release loop
(LLP 1030.003) makes that case routine, not rare.

**What avoids re-learning it.**

- Persist the journal ring to a capped file in the owned filesystem
  (LLP 1030.002) on the release path, flushed at boot milestones. Their numbers
  (1 MiB rotation, about 2 MB) are a reasonable starting budget.
- Write a panic hook that appends the message and location before `abort`.
- Record the refusal reason and time alongside `failures` in `record.json`.
- Build the LLP 1026 failure header, or delete the promise.

Each piece is small, and must not add boot-path work beyond an append.

## 4. What is not a lesson

- *Main-thread synchronous execution.* It removes coordination cost, but
  expensive app work can still block the thread. exact2's runner is not app code
  on the main thread, and nothing here argues for moving it there.
- *Yoga, Flutter-compatible authoring, iOS/Android scope.* These are a different
  portability contract. exact2 keeps CSS semantics and the web oracle.
- *Closed source with a sunset clause.* This is relevant only to adopting
  DartNative, which nobody proposes.
- *Maturity.* Two previews six weeks apart. The 2026-09-08 fixes are good
  maintenance evidence, not proof of breadth. Its changelog is still worth
  following as a feed of concrete UIKit failure cases.

## 5. Confidence

- **High.** The exact2-side code facts in F1, F3, F4 and F5 were read and
  spot-checked at the cited lines. The DartNative changelog, list and logging
  claims appear in its own docs.
- **Medium.** The F1 prediction that D9's per-route bar toggle reproduces the
  blur flash. The mechanism matches their changelog text, but D9 has not been
  prototyped against a mixed stack.
- **Medium.** The F2 claim that per-frame relayout during dismissal will cost
  frames at transcript scale. The mechanism is certain; the magnitude is
  unmeasured.
- **Low.** DartNative's interactive-dismissal behaviour. Only a summarized
  mention was found.

## 6. Follow-ups this document suggests (none decided)

1. **LLP 1035.001 D9.** Adopt the stack-owned-bar invariant (F1) before the next
   prototype round, and add the mixed-stack blur, Back-identity and bar-tap
   checks to acceptance.
2. **Keyboard.** Measure dismissal-drag frame cost at transcript scale (F2) on a
   device.
3. **Windowing.** When `List` windowing is designed, define the five-number
   memory measurement (F3) first. Consider amending `NOT-DOING.md`'s windowed-list
   line to name memory beside 60 fps.
4. **Diagnostics.** A capped persisted journal, a panic hook, and refusal reasons
   in `record.json` (F5), and the LLP 1026 header built or struck.
5. **Comparison run.** Drive DartNative's navigation and keyboard demos next to
   the Messages fixture on the same iOS 26 simulator, to confirm F1 and F2
   empirically rather than from docs.
