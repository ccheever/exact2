# LLP 1050: Drop frames or drop views — what each platform lets a list choose

**Type:** Research
**Status:** Draft
**Systems:** Scrolling (LLP 1010 §6.2 the windowed list, §6.5 the collection), Runner (`ListWindow`, `collection/index.rs`, `CollectionFeedback`), Apple host (`ScrollPumpIOS`, `Collection.swift`, `ChainingScrollView`, responsive scrolling), Web host (`collection-glue.js`, `list-selection.js`), Linux host (`presenter.rs` wheel, `presenter/collection.rs`), future Windows and Android hosts (`rules/DEFERRED.md`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24 (r1); r2 the same day, citations corrected after three reviews (`llp/reviews/1050.000-choosing-the-fill-tradeoff.{claude,astra,grok}.md`)
**Related:** LLP 1010 (scrolling; "never reset native momentum"), LLP 1044 and 1044.000 §11 (the deferred ruling: *"Never-blank and never-stall conflict there; which wins is a product choice"*), LLP 1044.001 §5.3/§6.2 (no visible placeholder in ordinary use; an explicit pending landing area under overload), LLP 1041 (graceful overload), `rules/DEFERRED.md` ("Scroll always wins"; no virtualList v2; Windows and Android hosts deferred). External: expo/expo PR 49975 (merged 2026-09-23, `List.ForEach` recycling). Every external source is cited inline; claims marked *inference* are reasoning, not a quoted source.
**Plans:** [LLP 1050.000](1050.000-choosing-the-fill-tradeoff.rfc.md) proposes how an app picks the trade-off.

## Summary

Charlie asked, 2026-09-24, after comparing our collection to Expo's recycled
SwiftUI `List` (PR 49975): *"is it possible to implement our collection thing in
a way that lets you make the choice of whether you want to drop views or drop
frames? those seem like they might each be desirable in certain situations"* —
and for this record: each platform, including Linux, Windows and Android.

The choice exists only at one moment. A scroll has moved the viewport onto rows
that are not built, and a frame is due. Either the frame waits for the rows (a
**hitch**: a late frame, never a hole) or it is presented without them (a
**hole**: the list's background or a placeholder where rows will be). Rows
built ahead of the viewport are invisible either way and can always run under a
budget; the choice is about the residue that prefetch did not cover.

**Findings:**

| | |
|---|---|
| F1 Who scrolls decides | Where the platform moves content off the main thread (browsers, WinUI's scrollers), a *user* scroll cannot wait for rows. Where the scroll is committed from the main thread (UIKit, Android Views/Compose, GTK4, Qt Quick, our Linux host), it waits by default. AppKit's responsive scrolling is between the two: it moves prepared content off the main thread and pauses at the edge of what was prepared |
| F2 A hole is always available | A single-threaded host can still present a hole: build under a budget, mount a cheap placeholder for the rest, present. Paging 3 nulls, Flutter's deferred image loading and WinUI's phases all do this. Only *never a hole* is platform-limited |
| F3 App-initiated moves can always wait | A jump, a restore, an insertion, a first mount — we move the offset ourselves, so we can build the target rows first on every platform, the web included |
| F4 Platform defaults split | Block: UIKit, RecyclerView, Compose, Flutter, GTK4, Qt Quick, WPF. Hole or placeholder: browsers, WinUI ListView (placeholders on by default), React Native FlatList. AppKit responsive scrolling *stalls* at the edge of what it prepared |
| F5 Shipped knobs exist | WinUI `ShowsScrollingPlaceholders` + `SetDesiredContainerUpdateDuration`; WPF `IsDeferredScrollingEnabled` (thumb drags only); React Native `windowSize`/`maxToRenderPerBatch`, documented as blank area vs. JS blocking. Among the systems examined, none lets an app retune the choice per gesture (WPF's split is fixed to thumb drags) |
| F6 The landing point is often known | At fling release UIKit (`targetContentOffset`) and InteractionTracker (`NaturalRestingPosition`) say where the scroll will stop; on Android `OverScroller.getFinalY` does when the host owns the scroller (RecyclerView's internal target is not public). The rows passed over and the rows landed on need not get the same policy |
| F7 We are split by path, not by choice | At `65d2a585` exact2 does both, by accident of path. The legacy list's runner always builds the visible rows in a report, so native blanks are rare (1044.000's scoreboard still records 3 blank frames in two 12,000 pt/s flings). The collection builds its whole window with no row limit on Apple and Linux. On the web both paths can show the background, because the compositor paints before the animation-frame report runs |

## 1. The trade-off, stated once

Four outcomes are possible when demand exceeds a frame:

1. **Block** — build every visible row, then present. Correct content, late frame.
2. **Defer** — present within budget; unbuilt visible rows show a hole or a
   placeholder; fill on later frames.
3. **Budget** — build synchronously up to N ms, then defer the rest. Block and
   defer are its two ends (N = ∞, N = 0).
4. **Slow the scroll** — shorten or cap the fling so fill keeps up (a fling
   target changed at release). Neither a hitch nor a hole, but the platform's
   momentum is altered.

Two adjacent levers change how often the choice arises without being the
choice: **cheaper rows** (reuse, fewer nodes, faster text) and **earlier rows**
(velocity-projected prefetch, landing-first prefetch, off-thread preparation).

## 2. Each platform

### 2.1 Web (Blink, WebKit, Gecko)

- **Scrolling is off the main thread.** Chromium's compositor thread can
  "scroll and redraw the tree without ever consulting the main thread"
  ([compositor thread architecture](https://www.chromium.org/developers/design-documents/compositor-thread-architecture/));
  "if the main thread is slow to respond, then the Scheduler may draw without
  waiting" ([How cc Works](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/how_cc_works.md)).
  Missing tiles draw as checkerboard and the frame still goes out, except when an
  impl-thread animation is moving the checkerboarded layer
  ([layer_tree_host_impl.cc](https://github.com/chromium/chromium/blob/main/cc/trees/layer_tree_host_impl.cc) ~1289).
  Firefox's APZ calls scrolling past its painted region checkerboarding
  ([APZ](https://firefox-source-docs.mozilla.org/gfx/AsyncPanZoom.html)).
- **`scroll` events are late by design**: the new position "is visible to the
  user before the `scroll` event is updated in the DOM"
  ([scroll-linked effects](https://firefox-source-docs.mozilla.org/performance/scroll-linked_effects.html)).
  A virtual list driven from `scroll` shows the background where rows are missing
  (*inference*).
- **The escape hatches are bad.** Non-passive listeners delay a gesture's start,
  not its frames; Chrome made root `touch*` (56) and `wheel` (73) listeners
  passive by default ([1](https://developer.chrome.com/blog/scrolling-intervention),
  [2](https://developer.chrome.com/blog/scrolling-intervention-2/)). Safari on
  macOS waits for the main thread's frame and then times out and commits alone
  ([WebKit](https://trac.webkit.org/wiki/WhatsChangingwithscrollingonmacOS)).
  Owning the scroll (`overflow: hidden`, own momentum) buys *block* at the cost
  of platform physics and accessibility — excluded here by "Scroll always wins".
- **`content-visibility: auto` is not never-blank.** Only the *initial*
  proximity determination is synchronous; later ones "take effect at the next
  rendering opportunity", and the spec's reason for special-casing the first is
  the risk of "producing blank content"
  ([CSS Containment 2](https://drafts.csswg.org/css-contain-2/)). Its
  `contain-intrinsic-size` sizes a skipped element: a cousin of a list-row
  estimate, not the same property.
- **Can choose:** defer or budget for user scrolls; block for app-initiated
  moves (build, then set `scrollTop` in the same task). No landing prediction:
  the web exposes no fling target (`scrollend` fires only at the end).

### 2.2 macOS (AppKit)

- **Responsive scrolling** moves already-drawn *overdraw* on a background thread;
  when it runs out, "concurrently moving the content is paused while waiting for
  the main thread to catch up"
  ([AppKit 10.9 notes](https://developer.apple.com/library/archive/releasenotes/AppKit/RN-AppKitOlderNotes/index.html)).
  AppKit itself stalls; it does not show holes. The hook to grow the prepared
  area is `prepareContent(in:)`
  ([docs](https://developer.apple.com/documentation/appkit/nsview/preparecontent(in:))).
  Overriding `scrollWheel:` disables it (same notes) — LLP 1044 F3's finding.
- **For us** the overdraw is the document view as drawn, so a row the runner has
  not mounted is overdrawn as background and responsive scrolling will carry
  that hole on screen (*inference*). Reporting an honest `preparedContentRect`
  (the mounted window) would turn AppKit's stall into our *block*; reporting a
  larger one would be *defer* (*inference*, untested).
- **At `65d2a585`:** `isCompatibleWithResponsiveScrolling` is true
  (`NodeViewMac.swift:57`) but applies only on the `.appKit` route — contained
  axes with a phased gesture (`:139–142`); an `auto` axis, and any unphased mouse
  wheel, is scrolled by hand on main (`:176–186`). Collections flush
  synchronously from the `boundsDidChange` observer (`NodeViewMac.swift:1110`,
  handled at `:710–713`, into `Collection.swift:215–220`), two reports per
  main-queue turn (`Collection.swift:45–55`), the rest next turn — so a
  collection can present a hole when the turn budget is spent. A scrollbar knob
  drag is already told apart (`KnobDrag`, `CollectionMac.swift:34–38`). The legacy list reports visible-only synchronously when uncovered and
  budgets the rest (`PresenterMac.swift:430–457`, `:543–555`).
- **Can choose:** all three for user scrolls on the single-threaded route; with
  responsive scrolling, block and defer through `preparedContentRect`. Landing
  prediction: none public for wheel momentum (*inference*).

### 2.3 iOS (UIKit)

- **The scroll is committed from the main thread.** A late commit repeats a
  frame ([tech talk 10855](https://developer.apple.com/videos/play/tech-talks/10855/);
  [hitches](https://developer.apple.com/documentation/xcode/understanding-hitches-in-your-app)),
  so slow construction hitches and never holes (*inference from the hitch
  model*).
- **Prefetch is adaptive and gives up under pressure**: when "there's no quiet
  times … we will not do pre-fetching"
  ([WWDC16 219](https://asciiwwdc.com/2016/sessions/219);
  [`isPrefetchingEnabled`](https://developer.apple.com/documentation/uikit/uicollectionview/isprefetchingenabled)).
  Placeholders in UIKit apps are data placeholders inside synchronously built
  cells.
- **Landing is known**: `scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`
  gives, and can change, where deceleration stops
  ([docs](https://developer.apple.com/documentation/uikit/uiscrollviewdelegate/scrollviewwillenddragging(_:withvelocity:targetcontentoffset:))).
- **Expo's PR 49975** sits here at pure *defer*: SwiftUI `List` holds every key
  with an estimated-height `Color.clear`; visibility (`onAppear`) is batched
  through `DispatchQueue.main.async` to JS, which renders a pool of
  `visible + 2 × overscanCount` slots (default 10 rows a side) with slot
  `index % capacity`. Its test plan: "Can see some blanks on ultra fast
  scrolls." Every data change bumps a revision that clears all measured heights.
- **At `65d2a585`:** collections flush synchronously inside
  `scrollViewDidScroll` (`NodeViewIOS.swift:649`), each report building its
  whole window with no row limit, under the same two-reports-per-turn cap as
  macOS (`Collection.swift:45–55`). The legacy list's `ScrollPump` builds
  visible rows synchronously only when uncovered (`ScrollPumpIOS.swift:78–84`)
  and fills the rest in 1–4 ms slices (`:28`, `:133–161`). Anchor corrections
  are dropped, not queued, while the user is tracking, dragging or decelerating
  (`CollectionIOS.swift:66–69`). `scrollViewWillEndDragging` exists
  (`NodeViewIOS.swift:686–707`) but only serves horizontal snapping.
- **Can choose:** everything — block is native, defer is a cheap placeholder
  mount, budget is the pump, landing-first from `targetContentOffset`.

### 2.4 Linux

- **Toolkits block.** GTK4's kinetic scroll is a frame-clock tick callback on
  the main thread
  ([gtkscrolledwindow.c](https://gitlab.gnome.org/GNOME/gtk/-/raw/main/gtk/gtkscrolledwindow.c) ~3351–3507);
  `GtkListView` recycles at most 200 widgets with 2 extra items a side
  ([gtklistview.c](https://gitlab.gnome.org/GNOME/gtk/-/raw/main/gtk/gtklistview.c)).
  Qt Quick's Flickable moves on the GUI thread
  ([scene graph](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html));
  a buffered delegate that becomes visible mid-incubation is `forceCompletion()`ed
  ([qqmldelegatemodel.cpp](https://code.qt.io/cgit/qt/qtdeclarative.git/plain/src/qmlmodels/qqmldelegatemodel.cpp) ~1302).
- **Wayland would allow off-main scrolling**: a desynchronized subsurface
  commits alone and `wp_viewport.set_source` crops per commit
  ([viewporter.xml](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/stable/viewporter/viewporter.xml))
  — AppKit-style overdraw in principle (*inference*; no toolkit found doing it).
- **Our host at `65d2a585`** draws everything itself on one thread (paint,
  poll, pump, input: `display.rs:380–470`). It has no fling: `wheel_at` adds and
  clamps deltas (`presenter.rs:1266–1330`), then refines collections
  synchronously, two passes (`presenter/collection.rs:8`, `:546–727`). The legacy
  list is not wired on Linux at all (no `list_viewport` call anywhere under
  `host/linux`; `QUEUE.md:89–94` lists Linux geometry as owed).
- **Can choose:** everything, including real holes, because it owns physics
  and presentation; a budgeted build followed by a painted placeholder band needs
  no second thread.

### 2.5 Windows (no host yet; the old repo's Direct2D host is deferred)

- **WinUI scrolls off the UI thread.** DirectManipulation processing "occurs on
  a separate, independent thread"
  ([DManip](https://learn.microsoft.com/en-us/windows/win32/directmanipulation/direct-manipulation-portal)),
  which is what `ListView`'s `ScrollViewer` uses; the newer `ScrollView` is
  built on InteractionTracker
  ([ScrollPresenter.cpp](https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/ScrollPresenter/ScrollPresenter.cpp)),
  which "is running in a different process than the application"
  ([InteractionTracker](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.composition.interactions.interactiontracker)).
- **Microsoft chose placeholders**: `ShowsScrollingPlaceholders` defaults to
  true ([API](https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.controls.listviewbase.showsscrollingplaceholders)),
  justified as "preserving smooth panning"
  ([ListView perf](https://learn.microsoft.com/en-us/windows/uwp/debug-test-perf/optimize-gridview-and-listview)).
  Phased filling (`x:Phase`, `ContainerContentChanging`) runs under a budget,
  40 ms by default (`BUDGET_MANAGER_DEFAULT_LIMIT`,
  [BudgetManager_Partial.h](https://github.com/microsoft/microsoft-ui-xaml/blob/main/dxaml/xcp/dxaml/lib/BudgetManager_Partial.h)),
  settable with `SetDesiredContainerUpdateDuration`. Thumb drags are the
  exception: "all scroll movement is going to happen synchronously with ticks"
  ([WindowManagement_Partial.cpp](https://github.com/microsoft/microsoft-ui-xaml/blob/main/dxaml/xcp/dxaml/lib/ModernCollectionBasePanel_WindowManagement_Partial.cpp) ~175).
- **WPF blocks** (offset change → `InvalidateMeasure` → build next layout pass
  on the UI thread,
  [VirtualizingStackPanel.cs](https://github.com/dotnet/wpf/blob/main/src/Microsoft.DotNet.Wpf/src/PresentationFramework/System/Windows/Controls/VirtualizingStackPanel.cs) ~476)
  and offers `IsDeferredScrollingEnabled`: content updates "only when the user
  releases the thumb"
  ([docs](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-controls)).
  Classic GDI `ScrollWindowEx` is blank-then-paint.
- **A self-rendering host** can put its own GPU content in a composition visual
  and let InteractionTracker move it
  ([Visual layer in Win32](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/using-the-visual-layer-with-win32)) —
  the browser model: smooth, holes past the over-render margin, no block.
  `NaturalRestingPosition` and inertia modifiers give landing prediction and
  fling shaping.
- **Can choose:** with InteractionTracker, defer/budget (block only for thumb
  drags and app-initiated moves); with its own scroller, everything.

### 2.6 Android (no host yet; deferred)

- **One thread does it all.** Choreographer callbacks run on the UI Looper
  ([Choreographer](https://developer.android.com/reference/android/view/Choreographer));
  RecyclerView's fling steps `scrollVerticallyBy` from a `postOnAnimation`
  runnable ([RecyclerView.java](https://github.com/androidx/androidx/blob/androidx-main/recyclerview/recyclerview/src/main/java/androidx/recyclerview/widget/RecyclerView.java)).
  RenderThread replays recorded frames and animates a few hidden properties; it
  does not scroll (*inference*, no API found).
- **Prefetch runs in the gap between frames** with per-view-type cost averages
  (`GapWorker`, `willCreateInTime`,
  [GapWorker.java](https://github.com/androidx/androidx/blob/androidx-main/recyclerview/recyclerview/src/main/java/androidx/recyclerview/widget/GapWorker.java)),
  but work "needed next frame" is forced with `FOREVER_NS` — block. Compose
  composes visible items in measure, and its pausable (split-across-frames)
  composition applies to prefetch only
  ([release notes](https://developer.android.com/jetpack/androidx/releases/compose-foundation));
  enabled in 1.10 alpha, disabled in 1.10.6 for stability.
- **Holes appear only by design or by another thread.** Paging 3 placeholders
  are null *data* bound to cheap views
  ([PagingConfig](https://developer.android.com/reference/kotlin/androidx/paging/PagingConfig));
  React Native shows blank area because JS renders behind a native scroller
  ([FlatList](https://reactnative.dev/docs/optimizing-flatlist-configuration)).
  A self-rendering host can own input (`Window.takeInputQueue`) and move
  pre-rendered layers with `ASurfaceTransaction` (API 29/31) — Chrome's shape
  (*inference*).
- **Can choose:** with platform scrolling, block (default) or defer via cheap
  placeholder items, budget via frame-deadline prefetch, landing-first via
  `OnFlingListener`/`OverScroller`; with its own scroller, everything.

## 3. The matrix

For a **user** scroll (an app-initiated move can block everywhere, F3):

| Host | Scroller | Block | Defer | Budget | Landing-first | Default expectation |
|---|---|---|---|---|---|---|
| Web | browser compositor | no | yes | yes | best-effort projection only | holes (checkerboard) |
| macOS | AppKit, main or responsive | yes on main; to the prepared edge on responsive (*untested for us*) | yes | yes | projection only (*inference*) | stall |
| iOS | UIKit, main | yes | yes (placeholder) | yes | yes | hitch |
| Linux (ours) | ours, main | yes | yes | yes | yes, once it has a fling | — |
| Windows, WinUI scroller | InteractionTracker | thumb only | yes | yes | yes | placeholders |
| Windows, own scroller | ours | yes | yes | yes | yes | — |
| Android, platform scrolling | UI thread | yes | yes (placeholder) | yes | yes | hitch |
| Android, own scroller | ours | yes | yes | yes | yes | — |

**Slow the scroll** is possible on iOS, Android and Windows and trivially on
Linux; LLP 1010 §6.2 forbids it today ("never reset native momentum").

## 4. Where exact2 stands, `65d2a585`

| | Collection (`virtualized=true`) | Legacy windowed |
|---|---|---|
| Web | scroll → animation-frame flush, 2 reports per list (replenished for new wrappers and epochs), 4 per frame, plus a pre-paint flush on resize (`collection-glue.js:36,72,92,246,264–270`); holes | animation-frame flush, `createLimit = 2` (`list-selection.js:8,34`); velocity 0 (`..Default::default()`, `abi.rs:406`, `lists.rs:9–10`); holes |
| macOS | sync from `boundsDidChange`, whole window per report, 2 reports per turn, rest next turn; no coverage check | visible-only sync when uncovered; budgeted rest; coverage rescue ≤8 tries (`PresenterMac.swift:799`, `:859`) |
| iOS | sync in `scrollViewDidScroll`, whole window per report, the same 2-per-turn cap | `ScrollPump`: same as macOS (`ScrollPumpIOS.swift:214`, `:247–258`) |
| Linux | sync after each wheel delta, 2 passes over a queue of views | not wired |
| Window | three viewports (one either side), no velocity, immediate retirement (`index.rs:259–262`, `mod.rs:476–478`) | the same three viewports, shifted by `velocity × 0.1 s` clamped to ±0.75 viewport (`window.rs:472–474`); retirement bounded, deferred only while a report creates nothing and rows remain to create (`:696–701`) |
| Budget in the ABI | none (`CollectionFeedback`, `collection/api.rs:17–39`) | `list_viewport_within(…, create_limit)` (`runner/src/runner/lists.rs:88–93`) and `list_status().pending` (`:32`, `:52–54`) |

No host has a placeholder. No agent `state` field reports list holes; the
macOS state's content-region diagnostics report retained coverage for the
reader's regions only (`Agent.swift:154`). LLP 1044.001 §8 asks for coverage
and request-to-accepted-viewport timestamps.
LLP 1044.000 shows what the current native policy buys and costs: on its
final scoreboard, jumps at 25/29 ms and never blank against Legend's 14 blank
frames; in its progress notes (`1044.000:472–479`), a 256-column table at 3–5
fps where Legend holds 30 by leaving rows blank — the case Charlie deferred on
2026-09-21. Which input produced the 3–5 fps is not recorded there.

## 5. Confidence

- **High:** the threading facts for web, AppKit, UIKit, RecyclerView, Compose,
  GTK4, Qt Quick, WinUI and WPF (primary docs or source cited); exact2's paths
  (read at `65d2a585`, not run).
- **Medium (*inference*):** that responsive scrolling carries our unmounted rows
  as holes; that `preparedContentRect` can steer it; that a self-rendering
  Windows or Android host can get Chrome-style off-thread scrolling through
  InteractionTracker or SurfaceControl.
- **Not established:** any measurement. No host here was run for this record;
  the Expo comparison benchmark is proposed, not built.
