# LLP 1068: Recycling heavy native views in list rows

**Type:** RFC
**Status:** Draft (r1)
**Systems:** Apple host (`NodePoolIOS.swift` and its reset contract; the video, web and native-module arms; `GpuIOS.swift`'s canvases; `NodeViewIOS.swift`'s material, scroll and field views), GPU module ABI (LLP 1009: one new entry, stage 3), native-module ABI (LLP 1024: one optional entry, stage 3), the video and web arms (one export each), Runner (none), Contract (none in stages 1–3)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** unassigned. Stage 1 belongs to whichever lane builds the Extra Heavy feed (`~/bench/xheavy`), before it measures
**Date:** 2026-09-27 (r1)
**Related:**
- `~/bench/xheavy/EXACT2-GAPS.md` gap 1 (the audit this answers) and `SPEC.md` (the 17 row kinds).
- LLP 1010 §6 (the collection; row state dies on retirement, `:334–340`, `:729–730`).
- LLP 1050.000 D3 (a costly row is never built mid-fling) and §7 (view reuse landed on iOS, `:294–302`).
- LLP 1009 D2 (a surface instance is per canvas node, `:72–74`) and D4 (the target is registered on create and unregistered on destroy, `:99–101`).
- LLP 1056 D10 (2D canvas pooling, stage 3), LLP 1024 D4 (the native table), LLP 1042 (video lifecycle, `:58–63`), LLP 1020 (iframe), LLP 1008 §9 (the iOS host).
- `QUEUE.md` "Pooling a list row that holds a canvas needs a ruling" (2026-09-26).

## Summary

`NodePoolIOS` parks a retired list row's views and hands them to the next
row of the same shape. It refuses any row that holds a video, a web view, a
GPU canvas, a 2D canvas, a native-module view, a material, a live nested
scroll view or a text field (`NodePoolIOS.swift:153–163`), and its `kinds`
leave out `native`, `iframe`, `video`, `canvas`, `canvas2d`, `input` and
`textarea` (`:60`). In the Extra Heavy feed about 9 of 17 row kinds therefore
rebuild every view they hold, plain or heavy, each time they scroll in.

This RFC decides, per heavy kind, whether the host may reuse the platform
view, what a reuse must reset so it cannot be told from a fresh element, and
what to do instead where reuse cannot be made invisible. It is built on
three findings:

1. **The row, not the heavy view, is most of what is lost.** Refusal is
   whole-row: one text field in a thread row makes its labels, images and
   boxes rebuild too. Pooling *around* a heavy leaf — reuse the row, build
   the leaf fresh — recovers the plain-view savings for every kind without
   touching any heavy view's semantics.
2. **The web oracle is "removed, then a new element inserted."** The runner
   never reuses ids and row state dies on retirement (LLP 1010). So a
   reused heavy view must look exactly like a new element: time 0, a blank
   document, an empty draft, a transparent bitmap, scroll offset 0. What may
   carry is only what no page can observe: a process, a decoder, a layer, a
   drawable pool, a tile cache.
3. **Reuse pays most where the view is slowest to create, and that is the
   heavy views** (§3, measured on the iPad simulator). A map costs about
   20 ms of main thread to create against 1.7 ms to reuse; a video player
   6.4 ms against 0.3 ms; a web view reaches its new document in 4 ms when
   reused and in about 500 ms when new (a WebContent process per web view).
   At 120 Hz a frame is 8.3 ms, so a map, a web view or a video built
   mid-fling misses frames on its own. Materials, scroll views and fields
   are cheap either way; they matter because one of them makes the whole
   row ineligible.

**Per-kind decisions (recommendations for Charlie):**

| Kind | Decision | Stage |
|---|---|---|
| Any row with a heavy leaf | Pool the row around the leaf: plain views reused, the heavy leaf destroyed and created fresh | 1 |
| Material (`UIVisualEffectView`) | Pool; stateless | 1 |
| Nested scroll view (carousel) | Pool when at rest; offset back to 0 | 1 |
| Text field / text area | Pool when not focused and not composing; value, selection, undo cleared | 2 |
| 2D canvas (`canvas2d`) | Pool, as LLP 1056 D10 already rules | LLP 1056 stage 3 |
| GPU canvas | Pool the `MetalView` and `CAMetalLayer`, never the instance (LLP 1009 D2 holds); later, the module keeps the configured wgpu surface per layer, if a profile says so | 2, then 3 |
| Video | Pool the `AVPlayer` and its layer (cap 2), never the item; hidden until the new item's first frame | 2 |
| Web view (`iframe`) | Pool (cap 2), rebound to the new node, hidden until the new document's `load`; the back-forward list is the one declared deviation | 2 |
| Native-module view (map) | Never pooled without the module's opt-in; never *created* mid-fling (built when the list slows); the opt-in `prepare_for_reuse` entry built with `native-map` as its first adopter | 2 (deferral), 3 (opt-in) |

The questions for Charlie are in §10.

## 1. What the pool is today

The collection's runner retires a row by destroying its nodes and creates
the next under new ids (`runner/src/instance/collection/api.rs`). On iOS,
`Presenter.apply`'s `destroy` case asks the pool first
(`PresenterIOS.swift:702`): a row whose whole subtree the batch destroys, in
a collection's list, of an eligible shape, parks — its views stay in the
list, the root hidden, each view released from the presenter's maps and
`recycle()`d. A row created later in a batch with the same shape (kinds and
child counts in preorder) `take`s them and applies its create ops. The file
header (`NodePoolIOS.swift:15–43`) is the reset contract. Measured on
listbench: the presenter's share of a built row fell from 1.23 to about
0.7 ms (LLP 1050.000 §7).

Three properties of today's pool matter here:

- **Refusal is whole-row.** `retire` requires every view in the tree to be
  `recyclable` (`:143`); `shape` returns nil for any kind outside `kinds`
  (`:118`). One ineligible leaf sends the whole row down the destroy path.
- **A full pool refuses; it does not evict.** `retire` returns false when
  `count ≥ 32` or the shape already has 8 (`:137`, `:141`). Parked trees of a
  shape that stops appearing stay until their list leaves the window
  (`end()`, `:85–90`). With 17 row kinds, a feed can fill the pool with
  trees of shapes that no longer scroll in, and then park nothing.
- **iOS only.** macOS has no pool (`host/apple/Sources/ExactKit/Mac/` has none;
  `QUEUE.md` "A list benchmark row is now mostly the runner and the batch").
  The web never reuses DOM nodes (`host/web/glue.js:745–749` removes the
  element; for a video it pauses, removes `src` and calls `load()`). The web
  is therefore the oracle, not a participant.

## 2. The oracle: what a page shows when an element is removed and a new one inserted

A virtualized list on the web (react-window, TanStack Virtual, the
collection's own web host) removes a row's elements and inserts new ones.
exact2's semantics are that, on every host (LLP 1010 `:334–340`: "Rows
leaving the window lose their view instances and component-local slots").
Per kind:

| Kind | Removing the old element | The new element | Observable carry-over |
|---|---|---|---|
| `video` | HTML's removal steps pause it (the media element "is removed from a Document" → internal pause steps) | `currentTime` 0, `readyState` HAVE_NOTHING, a fresh buffer; `poster` or transparent until the first frame; autoplay only once it can play | none; the HTTP cache may make the load faster |
| `iframe` | discards the nested browsing context (its document, timers, audio, scroll, focus) | a new browsing context navigating to `src`; `load` fires again | none except what the guest stored itself (cookies, storage) |
| `canvas` / `canvas2d` | the bitmap and its contexts go with the element | a transparent bitmap of the default or authored size | none |
| `input` / `textarea` | the value, selection, undo history and composition go | empty unless `value` is set; the dirty-value flag clear | none (the browser's autofill is per form, not per element) |
| scroll container | `scrollTop`/`scrollLeft` go | offset 0; browsers do not restore it on a new element | none |
| `backdrop-filter` / material | stateless | stateless | none |
| a native module | its instance is destroyed (`destroy`, LLP 1024 D4) | a new instance from `create` | whatever the module stores outside its instance |

The rule this RFC adopts: **a reused platform view must be indistinguishable
from a fresh one to the app, to the agent (LLP 1012) and to the reader.**
What may carry is only what a page cannot observe either: processes,
decoders, layers, drawable pools, caches. One deviation remains, and it is
declared: a reused web view's back-forward list (§4.7, Q3). §5.3 rejects the
one design that would need a larger one.

A consequence worth stating: *the draft, the video's time, the carousel's
offset and the map's camera are the app's to keep*, in keyed parent data, as
LLP 1010 already says of drafts. A reuse that kept them would make iOS
disagree with the web, and would make a row's state depend on which parked
view it happened to get.

## 3. Cost of creating versus reusing

Measured 2026-09-27 with a scratch UIKit probe (outside the repo) on an
iPad Pro 11-inch (M5) simulator, iOS 27.0, on a Mac with an M5 Max; the main
thread, 3 warm-ups, then N = 30 (N = 10 for new-region maps), median with
p90 in brackets, in milliseconds. "Create" is init, insertion into a view in
the window, layout and a `CATransaction.flush`. "Reuse" is what a park and
take would pay: unhide, move, and the kind's own re-point (a new HTML
document, `player` moved to another layer, a new region, a same-size frame).
Simulator numbers are not device numbers: they rank the kinds; §8's iPad
run replaces them.

| Kind | Create (sync) | Reuse (sync) | To content, new vs reused (async) | Memory per live view |
|---|---|---|---|---|
| Plain view + label (what the pool reuses today) | 0.35 (0.43) | 0.08–0.18 | — | 0.07 MB |
| `UIVisualEffectView`, ultra-thin | 0.33 (0.52) | 0.10–0.15 | — | ≈ 0 |
| `UIScrollView` with 10 cards | 1.78 (2.02) | 0.47–1.49 | — | 0.32 MB |
| `UITextField` | 1.56 (2.42) | 0.54–0.57 | — | 0.04 MB |
| `UITextView` | 2.03 (2.29) | 0.61–0.84 | — | 0.06 MB |
| `CAMetalLayer` + first frame (raw Metal, not wgpu) | 2.91 (3.58) | 0.97 same size; 2.55 new size | — | in GPU memory; not visible on the simulator |
| `AVPlayer` + item + `AVPlayerLayer` | 6.4 (11.1) + 0.5 insertion | 0.3 (player moved to a layer) | first frame 27.7 vs 11.0 (`replaceCurrentItem`) | 1.06 MB |
| `WKWebView`, shared configuration | 4.4 (init 1.7 + insertion 2.7); 7.7 with a fresh configuration, as the arm makes today | 0.7 | `didFinish` 505 (584) vs 4.0 (6.4) | 0.15 MB in the app, plus a WebContent process of 14–15 MB each |
| `MKMapView` | 19.6 (init 13.7 + insertion 5.9) | 1.7 | fully rendered 119 (same region) or 484 (new region) vs 1,045 (new region; tiles over Wi-Fi dominate) | 17.1 MB |

The first instance of each kind costs far more (a cold map 142 ms, a cold
web view 198 ms before its first load of 1.1 s, a cold text view 23 ms); a
list pays that once. On the Mac (AppKit, the same probe) the order is the
same: a text field 2.5 vs 0.9, a 10-card scroll view 3.9 vs about 1.6, a
material 0.84 vs 0.14, a Metal layer and frame 1.4 vs 0.77.

What the table says:

- **The heavy views are where reuse pays.** A map row, a web view row and a
  video row each spend a 120 Hz frame (8.3 ms) or more on creation alone,
  on top of the row's plain views; the reused view costs under 2 ms.
- **For a web view the asynchronous half decides it.** A new web view shows
  its page half a second after it scrolls in; a reused one in a frame. No
  deferral makes the new one faster.
- **For a map the asynchronous half is the network.** Reusing the view does
  not load the new region's tiles faster (the reused new-region number is
  slower than the fresh one, which is Wi-Fi variance). The saving is the
  20 ms on the main thread.
- **A Metal layer saves only at the same size.** Reuse at a new drawable
  size costs what a new layer costs; list rows of one width keep one size.
  The wgpu surface's own `create_surface` and `configure` come on top and
  were not measured here (§8 profiles them in the real app).
- **Materials, scroll views and fields are cheap either way** (under 2 ms).
  Their value is keeping the rest of their row eligible (§4.0, §4.1–§4.3).
- **Memory is the price.** A parked map holds 17 MB; a parked web view a
  15 MB process. That sets the caps in §6.

## 4. Per-kind decisions

Each decision names what a park must clear, what may carry, what the new
row's props re-apply, and when a view is ineligible instead.

### 4.0 Pooling around a heavy leaf (every kind; stage 1)

**Decision: a row's plain views pool even when it holds heavy leaves; each
heavy leaf is destroyed at park and created fresh at take.**

- **Shape.** A heavy node's kind still enters the shape string, marked as a
  leaf slot (`video*()`), so only rows with the heavy leaf in the same place
  match. A heavy node must be a leaf of the shape: its own children (a
  GPU canvas's overlay children, LLP 1014) make the row ineligible as today.
- **Park.** The heavy leaf goes down the existing destroy path
  (`release` + `forget` + `removeFromSuperview`), exactly as an unpooled
  destroy does today. Every heavy kind's own teardown (`VideoView.invalidate`,
  `destroyEmbedded`, `Canvases.destroy`) is unchanged.
- **Take.** `take` claims the plain views and leaves the heavy id unclaimed;
  the presenter creates a fresh `NodeView` for it, and the batch's
  `children` op inserts it at its index in the reused parent.
- **What it saves.** The row's text, image, box and button views: for a
  thread, carousel, video, map, web view or shader row, everything but the
  one heavy view.
- **Oracle.** Untouched: the heavy view is exactly as fresh as today.
- **Later stages.** When a kind gains its own pool (§4.3–§4.8), its leaf is
  parked with the row, under the kind's cap (§6), instead of destroyed; past
  the cap it is destroyed as here. A fresh leaf created at take is subject
  to §5.1 (not created mid-fling).
- **Scope.** About 60 lines in `NodePoolIOS.swift` and a test per kind in
  `NodePoolIOSTests.swift`.

Stage 1 also replaces refusal-when-full with **least-recently-parked
eviction**: a retire into a full pool (or a full shape) drops the oldest
parked tree, so the pool follows the feed rather than freezing on its first
32 rows.

### 4.1 Materials (`backgroundMaterial`; stage 1)

**Pool.** A `UIVisualEffectView` holds no content state.
- **Park clears:** nothing beyond the existing contract; `materialNodes` is
  already dropped by `release`.
- **Take re-applies:** `updateMaterial()` from the new props — kind,
  effect (`UIBlurEffect` or `UIGlassEffect`), `isInteractive`, corner radius.
  A new row without a material removes the view, as a prop change does today
  (`NodeViewIOS.swift:1017–1047`).
- **Ineligible:** a glass material whose interactive effect is mid-touch
  (already covered by `pressed`).
- **Oracle:** `backdrop-filter` is stateless; nothing to deviate from.

### 4.2 Nested scroll views (carousels; stage 1)

**Pool when at rest.**
- **Ineligible:** `isTracking`, `isDragging` or `isDecelerating`; a
  refresh control that is refreshing; a pending authored scroll (already).
- **Park clears:** `contentOffset` to the top-left inset (the oracle is
  offset 0), `contentSize` to zero, the scroll pump's entry
  (`scrollPump.forget`, already in `release`), the refresh control, zoom
  scale to 1, `keyboardDismissMode`, indicator flash state.
- **May carry:** the `UIScrollView` and its pan recognizer.
- **Take re-applies:** `fitScroll()`, `syncScroll()` and `updateRefresh()`
  from the new node's style and handlers.
- **Oracle:** a new scroll container starts at offset 0. An app that wants
  a carousel to come back where the reader left it binds `scrollLeft` in
  keyed data (SPEC's carousel does not).

### 4.3 Text fields and text areas (stage 2)

**Pool when idle.**
- **Ineligible:** first responder, editing, marked text (composition),
  a pending focus, autofill in progress; any row pinned by focus
  (LLP 1010's focus pin keeps such a row mounted anyway).
- **Park clears:** `text`, `attributedText`, `selectedTextRange`, the undo
  manager's actions (`undoManager?.removeAllActions()`), `pendingValue`,
  typing attributes, `placeholder`, input traits back to defaults.
- **May carry:** the `UITextField`/`UITextView` and its delegate wiring.
- **Take re-applies:** every trait from props (`keyboardType`,
  `autocapitalizationType`, `returnKeyType`, `textContentType`,
  `isSecureTextEntry`), then `value`.
- **Oracle:** a new `<input>` is empty unless `value` is set, with no
  undo history. Undo history is the one easy leak: an undo in a reused
  field that restored the last row's draft would be a visible bug, so the
  undo stack is cleared, and a test types, retires, takes and sends `undo`.
- **Stage 2, not 1,** because input traits on a reused field are the
  part of UIKit most likely to hold stale state (autocorrection caches,
  `textContentType`), and they deserve their own fixture.

### 4.4 2D canvases (`canvas2d`)

LLP 1056 D10 already rules this (stage 3 of that RFC): a 2D canvas is a
plain view; its backing pixels are cleared at park, its lifetime,
generation, sequence, requests and subscriptions dropped, and the next
node's `mounted` is its own. This RFC adds nothing but the shape rule of
§4.0 (a `canvas2d` is a leaf) and moves the citation: D10's
`NodePoolIOS.swift:117` is now `:60` (`kinds`) and `:136` (`retire`).

### 4.5 GPU canvases (stage 2, then 3)

LLP 1009 D2: "Instances are per canvas node, created when the module is
up, dropped with the node" (`:72–74`). The instance — the app's `Surface`
object, its `bind`/`render` state, its publisher, its carry — is the
node's, and the oracle (a new `<canvas>` with a new WebGPU context) agrees.
Nothing here moves an instance.

What a row's canvas costs besides the instance is the platform target: the
`MetalView`, its `CAMetalLayer`, the overlay `PlainView`, and inside the
module the `wgpu::Surface` created on that layer and `configure`d
(`gpu/src/native.rs:170–190`, `gpu/src/lib.rs:540–570`). Pipelines are
already shared across instances (`SHARED`, cryptobench GAPS §2).

**Stage 2: pool the layer, never the instance.**
- **Park:** `Canvases.destroy(view:)` runs as today — `gpu_destroy` drops
  the instance and its `wgpu::Surface` (D2 and D4 hold unchanged). The
  `MetalView` and overlay stay; the overlay must be empty (a canvas with
  children is a non-leaf and ineligible, §4.0). `canvasInput` is dropped.
- **Take:** `Canvases.surface(view:…)` creates a fresh instance on the
  same layer (`gpu_create` with the same pointer).
- **Pixels:** the layer still shows the last row's final drawable. The
  oracle is a transparent canvas, so the `MetalView` is hidden at park and
  shown on the new instance's first present.
- **Saves:** the UIKit view and layer creation and window insertion; not
  the surface configure.

**Stage 3: the target outlives the instance.** The module keeps each
configured `wgpu::Surface` keyed by its layer and reuses it for the next
`gpu_create` on that layer (reconfiguring only if the size changed). The
layer pointer alone is not a safe key (a freed layer's address can be
reused), so the host says when a layer really dies: one new entry,
`gpu_release_target(layer)`, called when a parked `MetalView` is evicted or
its list leaves the window. That amends D4's sentence "registers the
platform target … and unregisters it on destroy" to "… when the target is
destroyed", which is a change to LLP 1009's text and is Q2.
- **Saves:** surface creation and configure, which cryptobench names as the
  cost ("Surface configuration and layer churn are", GAPS §2).
- **Risk:** a reconfigure at a new size allocates drawables anyway; stage 3
  is worth building only if §8's Time Profiler run shows configure, not
  drawable allocation, dominating.

### 4.6 Video (stage 2)

LLP 1042 `:58–63`: "One node incarnation owns one player." The arm already
treats a new `src` as a new item with a new generation (`VideoArm.swift`
`loadSource`, `:184–227`), so a reset is mostly what the arm does for a
`src` change plus what `invalidate` does to playback.

**Pool the `AVPlayer` and its `AVPlayerLayer` (cap 2), never the item.**
- **Park:** pause; `replaceCurrentItem(with: nil)`; remove the time
  observer, KVO and notifications; bump the generation and poster
  generation; hide and clear the poster; `wantsPlay = false`; drop
  `naturalSize`, `pendingSeek`, `lastError`; unregister from
  `VideoVisibilityHost`. This is the web's destroy (`pause`, remove `src`,
  `load()`).
- **May carry:** the `AVPlayer`, the `VideoLayerView` and its layer.
- **Ineligible:** an `AVPlayerViewController` presentation (a controller
  "remains until the node is destroyed", LLP 1042 `:51–53`), full screen or
  PiP active, Now Playing ownership.
- **Take:** the new props arrive through `update` as a `src` change does.
- **Pixels:** with no item the layer shows nothing, which is the oracle's
  transparent-until-first-frame.
- **Needs:** one new arm export, `exact_video_reset(handle)`, and a host
  rebind of the arm's node identity.

**Why.** §3: creating the item, player and layer costs 6.4 ms of main
thread and the first frame comes 28 ms later; moving an existing player to
a layer costs 0.3 ms and a `replaceCurrentItem` reaches its first frame in
11 ms. Practice agrees for a second reason: an item paired with a player
builds a render pipeline that is not freed by dropping the player, and
enough of them exhaust the hardware decoders (`-11839`); feed apps keep 3–5
players and swap items (Appendix). A small player pool bounds that count.
Pausing off screen (§5.2) remains the app's, through its `paused` binding.

A finding on the way, which limits this section: on iOS the arm installs
an `AVPlayerViewController` unless the node sets both
`allowsVideoFrameAnalysis=false` and `allowsPictureInPicturePlayback=false`
(or `disablepictureinpicture`): both default to true in
`configurePresentation` (`VideoArm.swift:264–287`). A feed's muted inline
video therefore pays for a view controller, its child containment and its
subviews per row, where the web's `<video>` without `controls` has none,
and a controller-presented video is ineligible above. Whether an inline
video without `controls` should default to the layer is LLP 1042's question,
not this RFC's; it goes to `QUEUE.md`, and until it is answered the Extra
Heavy port sets both props to `false` so its video rows can pool.

### 4.7 Web views (`iframe`; stage 2: a small pool, hidden until the new document loads)

§3 changes what the draft of this section expected. A web view's synchronous
cost on the simulator is a few milliseconds; what costs is the **~0.5 s from
creation to the first `didFinish`**, nearly all of it a new WebContent
process (each web view gets its own, about 14–15 MB footprint each). An
existing web view loading a new HTML document reaches `didFinish` in about
4 ms. That is the difference between a web-view row that shows its content
as it scrolls in and one that shows its background for half a second, and
no deferral recovers it. So:

**Pool, with a cap of 2 parked web views.**
- **Park:** stop loading; serve an empty wrapper (the oracle discards the
  document: its timers, audio and pending loads stop); bump the arm's
  generation so a late `load` or `message` from the old document is
  dropped (both already carry `generation`, `WebArm.swift` `wrapper`);
  resign first responder; reset the scroll view's zoom and offset; hide.
- **May carry:** the `WKWebView`, its configuration, user content
  controller and script handlers, and its WebContent process.
- **Take:** rebind the arm to the new node id (the id is baked into the
  arm at creation — `wrapperURL` and `WebCallbackBox` — so this is one new
  arm export, `exactWebRebind(handle, id)`); apply `src` and `sandbox` as a
  fresh mount does (LLP 1020 D2's "sandbox is immutable per mount" holds:
  this is a new mount); keep the web view hidden until the new
  generation's `load`, so no pixel of the last row's page is shown. Until
  then the box shows its background, as a fresh web view does.
- **Ineligible:** a web view whose guest has focus or an active text input,
  a web view whose process was terminated (`webViewWebContentProcessDidTerminate`),
  and any web view while VoiceOver runs (as for every row).
- **What cannot be made invisible:** the back-forward list. No public API
  clears it, so the guest's `history.length` grows across reuses where a
  fresh `<iframe>` starts at 1. exact2's `iframe` exposes no navigation
  controller (LLP 1020 §5), so only the guest's own script can see it. This
  is the one declared deviation in this RFC, and it is Q3.
- **Memory:** 2 parked web views keep 2 WebContent processes, about 30 MB
  outside the app's footprint on the simulator. They are the first thing
  evicted under memory pressure and on entering the background.

### 4.8 Native-module views (maps; stage 2 deferral, stage 3 opt-in)

LLP 1024's table has no reuse entry: `destroy` "is the last call"
(D4, `:287–294`). The host cannot know what a module's view holds — a map's
annotations, overlays, camera, delegate state, tile requests, user location.

**Never pool a module view the module has not opted into.** That is also
what React Native settled on: Fabric recycles views by default, and every
Expo module opts out ("it may lead to more bugs than gains"), while
`react-native-maps` and `react-native-webview` recycle only their shell and
destroy the map or web view inside it (Appendix) — §4.0's design.

**Stage 2: never created mid-fling (§5.1).** The map is the most expensive
view to create in §3 (about 20 ms of main thread, 142 ms cold) and the
largest to keep (17 MB). A module view is built when the list slows.

**Stage 3: the opt-in.** One optional table entry, size-versioned as D4
allows (`size ≥ 80`; the major stays 1), and a roster bit per tag
(`{"native-map": {"snapshot": true, "reuse": true}}`):

```
72  prepare_for_reuse(handle, nonce) → i32
        nullable. 0: the instance is now as if created with no props, and
        its callbacks carry `nonce` from here on; anything else: refused
```

The host calls it at park with a fresh nonce that routes nowhere until a
take, so a late callback from the old row, or one fired while parked, is
dropped as a destroyed instance's is (LLP 1024 `:303–313`). At take the
host routes that nonce to the new node and sends the new row's props with
`set_props`. A refusal, a missing entry or a missing roster bit means the
view is destroyed as today. For `MKMapView` the module removes annotations
and overlays, cancels its own requests and snapshot work, resets the camera
and the delegate's state, and lets `set_props` set the region; the tile
cache carries, which the oracle cannot see.

`apps/map-demo`'s `NativeMap` is the first adopter, and the Extra Heavy map
row the proof. The entry is built only if stage 2's run shows map rows
still costing frames when the list comes to rest (the 20 ms then lands on
the settle frame, which D3 allows but the reader sees).
Cap: 1 parked map (17 MB).

## 5. Alternatives to pooling

### 5.1 No heavy leaf built mid-fling (stage 2)

LLP 1050.000 D3 says a row whose cost exceeds the frame is not built during
user motion; it shows as pending until the motion slows. That decision is
per row and needs the runner's cost memo (1050.000 stage 3, unbuilt). The
heavy kinds give a narrower, host-only form of it:

**During a fling, a heavy leaf's node is created but its platform view is
not.** The node's box is laid out as ever (every heavy kind is sized by its
style and never measured: `canvas`, `iframe` and `native` by LLP 1009 D3 and
LLP 1024; `video` once it has an authored size or `aspect-ratio`), and it
paints its background, border and radius — or its `poster` for a video. When
the list's velocity falls under the host's threshold, or the row comes to
rest, the host creates the web view, map, player or surface.

- **Oracle.** A browser does the same with `loading="lazy"` on an `iframe`
  (the load waits until the element nears the viewport) and with
  `preload="none"` on a video; Safari and Chrome also hold muted autoplay
  until a video is visible. Late creation is inside what a page can see; it
  is not a deviation. The `load` event and `canplay` fire later, as they
  would.
- **The agent.** Under `clock settle` nothing is moving, so every leaf is
  created; screenshots and `state` see the settled row.
- **Only a new view waits.** A parked view of the kind (§4.5–§4.8) is taken
  mid-fling as any parked row is: reuse costs under 2 ms. What waits is
  creating one when the pool has none.
- **Which kinds.** `iframe` and `native` always; `video` when its box has an
  authored size or `aspect-ratio` (otherwise its natural size feeds layout,
  and waiting would move the row later); GPU canvases never (a layer is
  under 3 ms to create in §3, and the shader row's content is the surface).
- **Contract.** Nothing new. `loading="lazy|eager"` on `iframe`, the web's
  name for asking, waits until an app wants eager creation in a list.

### 5.2 Pausing video off screen

Already specified: `playbackVisibilityThreshold` pauses a video below a
visible fraction, but only with an authored `paused` binding (LLP 1042
`:94–102`). In a virtualized list the rows past the viewport are retired
anyway, and the web pauses a removed element; what is left is the rows in
the window but outside the viewport. This RFC recommends no change: the
Extra Heavy port authors the binding, as SPEC's `BENCH_FREEZE` needs it
anyway.

### 5.3 A keyed keep-alive set (rejected)

Keeping a heavy view alive by row key — so scrolling back finds the same
web page or the same video at the same time — is the one design here that
*deviates* from the oracle: a web page's virtualized list reloads the
iframe. It also makes a row's state depend on whether its key was recently
seen, which the agent cannot reproduce, and LLP 1010 `:339–340` rejects an
unbounded version outright.

The keep-alive that already exists is the right one: the runner does not
retire a row until it is two viewports away or the window's own count is
exceeded (LLP 1050.000 §6), on every host alike. If a heavy kind needs a
longer life, the lever is that retention rule, stated in the runner and
seen by the web too — not a host cache.

### 5.4 Posters and snapshots

A snapshot of the last row's content (a `WKWebView.takeSnapshot`, an
`MKMapSnapshotter` image) shown until the live view exists is what the audit
suggested as a fallback. For a *new* row there is no last content to show,
so a snapshot is only a keep-alive by another name (§5.3). What remains is
the app's own `poster`: authored, the same on every host. Apple's
recommendation for static maps in lists is `MKMapSnapshotter`; a module can
do exactly that (LLP 1024 has a `snapshot` entry for the agent already), and
it is the module's choice, not the host's.

## 6. Pool sizing, memory and eviction

- **Trees, as today:** 32 trees, 8 per shape, now evicting the oldest
  (§4.0). A tree's plain views are about 720 bytes each (`QUEUE.md`,
  crypto-list), so 32 rows of 20 views is under 0.5 MB plus their layers,
  whose bitmaps `recycle` already drops.
- **Heavy views parked inside trees are counted by kind, with a cap each:**

  | Kind | Cap | Why |
  |---|---|---|
  | material | none beyond the tree cap | no backing store while hidden |
  | scroll view | none beyond the tree cap | its content is plain views, counted already |
  | text field / area | 4 | UIKit text-input objects are the heaviest plain views; a feed rarely shows more |
  | GPU canvas layer | 4, and 16 MB of drawable area | a `CAMetalLayer` keeps up to `maximumDrawableCount` (3) drawables; at C × 220 pt on a 2× iPad that is about 3 × 3.5 MB |
  | video player | 2 | decoders are a shared hardware resource; the visible set is the rest; 1 MB each |
  | web view | 2 | a WebContent process each, 14–15 MB outside the app's footprint (§3) |
  | native-module view (opt-in, stage 3) | 1 | an `MKMapView` is 17 MB (§3) |

  The heavy caps add up to about 70 MB at worst (two web processes, one map,
  four layers' drawables, two players), which is the budget: a parked heavy
  view that would exceed it is destroyed instead of parked.

- **Drawables at park.** A parked `MetalView` is hidden, but its layer's
  drawables stay resident. Shrinking `drawableSize` at park would free them,
  but §3 shows a reuse at a new size costs what a new layer costs, so it
  would cancel the saving. Parked layers keep their drawables and count
  against the cap; stage 2 measures what 4 of them hold on the iPad.
- **Memory pressure.** `UIApplication.didReceiveMemoryWarningNotification`
  drops every parked tree (wired to the existing `NodePool.reset()`). Entering
  the background drops parked web views, maps, video players and GPU layers.
- **Counted, not hidden.** The pool reports its parked trees by kind in
  `state` (the agent's existing collection section), so the benchmark and
  the smokes can see what the pool holds.

## 7. `rules/DEFERRED.md`

Nothing comes *off* the list: every kind here is already admitted (video
2026-09-18, `iframe` 2026-08-30, native modules 2026-09-26, Canvas 2D). One
line goes *on*, under "Features carried over as no", beside the
virtualList line:

> - No host keep-alive of heavy views by row key, and no reuse of a
>   native-module view without the module's opt-in (LLP 1068 §4.8, §5.3). A
>   reused view is indistinguishable from a new element, except a web view's
>   back-forward list (declared in LLP 1068 §4.7); state that must survive
>   scrolling lives in keyed data.

Take: none; this adds a "no", which the file does not charge for.

## 8. Staging, apparatus and measurement

| Stage | Ships | Proven by |
|---|---|---|
| 1 | Pool around heavy leaves (§4.0); oldest-first eviction; materials (§4.1); nested scroll views at rest (§4.2); parked-tree counts in `state` | `NodePoolIOSTests`: one test per heavy kind that a row holding it parks, the leaf is fresh (new platform view identity, new arm handle) and its siblings are reused; a material and a carousel row that come back reset (offset 0). The Extra Heavy fling ladder before/after on the iPad |
| 2 | Text fields (§4.3); GPU layers without instances (§4.5); the video player pool (§4.6); the web view pool (§4.7); no new heavy view created mid-fling (§5.1); the heavy caps, the budget and the memory-warning and background drops (§6) | Field tests: type, retire, take, `undo` changes nothing; a composing field never parks. A GPU test: the new instance's first frame is the first thing the layer shows. A video test (`apps/video-player` in a list): a taken player shows no frame of the old clip and starts at 0. A web test (Caltrain's deck in a list): the taken web view is hidden until the new generation's `load`, and an old document's late `message` is dropped. `smoke.mjs canvas` and `smoke.mjs ios` unchanged. The Extra Heavy ladder and the crypto-list GPU ladder (cryptobench `exact-gpu`) at 24k and 48k pt/s, with Time Profiler |
| 3 | Each only if stage 2's run asks for it: the module keeps configured surfaces per layer, `gpu_release_target` (§4.5); the native-module `prepare_for_reuse` entry with `native-map` adopting it (§4.8) | The same ladders; for the map, `apps/map-demo` in a list: a taken map shows none of the last row's annotations |

**Apparatus.** Nothing new in the repo. Tests go in the existing
`host/apple/tests/ExactKitTests/NodePoolIOSTests.swift`. Fixtures are the
existing apps: `apps/video-player`, `apps/map-demo`, `apps/canvas-gallery`,
`apps/sparkline`, Caltrain's `iframe` deck. The smokes are the existing
`smoke.mjs ios` and `smoke.mjs canvas`. Driving is `scripts/agent.mjs ios`
(`tree`, `state`, `clock settle`, `screenshot`). Benchmarks stay outside the
repo (LLP 1050.000 D4): `~/bench/xheavy` and `~/bench/cryptobench`, probe
`~/bench/heavybench/probe`.

**Measured on `~/bench/xheavy`.** On the M1 iPad Pro 12.9", landscape, the
SPEC's `fling` and `live` scenarios, three interleaved rounds against the
revision before each stage, as the crypto and heavy lanes did:
- fps and p95 frame time per ladder speed;
- main-thread CPU ms/s;
- footprint peak, end and 10 s after, by VM tag once (`BENCH_VM=1`);
- blank area (the probe's detector) — stage 2's deferral makes a heavy leaf
  show its background during a fling, which the detector must count as
  content-pending, not blank (it already treats a loading map as the grid
  colour, SPEC);
- per stage, the count of heavy platform views created per second
  (`state`'s counters), which is the number this RFC exists to lower.

The first measurement is **unpooled**: the port ships without any of this,
and its numbers are the baseline (the audit's own fallback).

## 9. What this does not do

- It does not reuse node ids or kernel nodes. The runner is unchanged.
- It does not bring the pool to macOS. The same contract applies when the
  Mac gets one (its own change; the arms are shared Swift, so §4.5–§4.8's
  resets are written once).
- It does not change the web, which is the oracle.
- It does not move a surface instance, an `AVPlayerItem` or a web document
  between rows, ever.

## 10. Questions for Charlie

**Q1. Build stage 1 now: pool rows around their heavy leaves, evict the
oldest parked tree when full, and pool materials and resting carousels?**
Recommendation: yes. It changes no heavy view's semantics, needs no ABI or
LLP amendment, and recovers the plain-view savings for every heavy kind.
Confidence: high (0.85).

**Q2. For GPU canvases, pool the layer and create a fresh instance on it
(stage 2, LLP 1009 unchanged); and, only if stage 2's profile shows the
surface configure dominating, let the module keep a configured surface per
layer with a new `gpu_release_target` entry, amending D4's "unregisters it
on destroy" to "when the target is destroyed" (stage 3)?**
Recommendation: yes to stage 2; stage 3 gated on the profile. Confidence:
medium (0.65); the saving of stage 2 alone is the least certain number here.

**Q3. Pool web views (cap 2, rebound to the new node, hidden until the new
document's `load`), accepting that a guest's `history.length` grows across
reuses where a fresh `<iframe>` starts at 1?**
Recommendation: yes. The alternative, never pooling, leaves every web-view
row blank for about half a second after it scrolls in (§3), which no
deferral fixes; the deviation is visible only to guest script, and exact2's
`iframe` has no navigation surface. Confidence: medium (0.6); the risk is
WebKit state the reset list misses, which stage 2's fixture must look for.

**Q4. For native-module views: never pool without the module's opt-in;
never create one mid-fling (stage 2); and build the opt-in
`prepare_for_reuse` entry, with `native-map` as its first adopter, only if
stage 2's run shows map rows still costing frames at rest (stage 3)?**
Recommendation: yes. Confidence: medium-high (0.7). The alternative
"never pool modules, full stop" is safer and is what Expo chose; it leaves
a 20 ms map creation on the settle frame of every map row.

## Appendix: sources

**Apple (official):**
- `UITableViewCell.prepareForReuse` / `UICollectionReusableView.prepareForReuse`: reset only non-content attributes there; content is set again by the data source. The pool's split between `recycle` and the create ops is the same split.
- WWDC21 10252, "Make blazing fast lists and collection views": prepared cells may never be displayed; cells scrolled off wait before returning to the reuse queue.
- `AVPlayer`: "You can reuse the player instance to play additional media assets using its `replaceCurrentItem(with:)` method." WWDC16 503: removing a player's only layer no longer pauses it.
- `WKProcessPool`: deprecated in iOS 15; "Creating and using multiple instances of WKProcessPool no longer has any effect." `WKBackForwardList` is read-only.
- `MKMapSnapshotter`: the recommended static map; overlays and annotations are drawn by the caller.
- `CAMetalLayer`: a pool of drawables per layer; `maximumDrawableCount` 2 or 3 (default 3).
- WWDC14 419: a blur is several passes; "budget for effect."
- Apple publishes no reuse guidance for `WKWebView`, `MKMapView`, `UIVisualEffectView` or `CAMetalLayer` in cells, and no cap on the reuse queue.

**Practice (not Apple):**
- Decoder exhaustion (`-11839`) past a device-dependent number of live players; an item paired with a player builds a render pipeline that setting the player to nil does not free (Hansmeyer, "Too Many AVPlayers?", 2017; Apple forums 67382). Feed apps keep 3–5 players and swap items.
- `WKWebView` creation is among the heaviest actions in an iOS app, each with its own WebContent process (Embrace, "WKWebView memory leaks").
- Nested carousels keep a per-item `contentOffset` in the model and restore it on display (Furrow); FlashList's `useRecyclingState` exists to reset exactly that.
- React Native Fabric recycles views per component type (`RCTComponentViewRegistry`, cap 1024, purged on a memory warning), but `react-native-video` opts out (`shouldBeRecycled` NO), every Expo module opts out ("it may lead to more bugs than gains", `ExpoFabricView.swift`), and `react-native-webview` and `react-native-maps` recycle only their shell: `prepareForRecycle` destroys the web view or map inside. That is §4.0's design, arrived at independently.
- Litho pools mount content per component type, `poolSize` 3 by default, with optional preallocation.

**The web (spec):**
- HTML §4.8.11.8, removing a media element: "Run the internal pause steps." A new element starts at `NETWORK_EMPTY`/`HAVE_NOTHING`, `currentTime` 0.
- HTML, the `iframe` element: the removing steps destroy the child navigable ("the element's content document is destroyed, not unloaded"); insertion creates a new one, which loads `src` again. `moveBefore()` (Chrome 133+) is the one way to move an iframe or video with its state, and a virtualized list does not move; it removes.
- HTML canvas: with no context the bitmap is transparent black; the context belongs to the element.
- HTML form controls: the value and the dirty-value flag belong to the element.
- `content-visibility: auto` keeps an element and its state while skipping its rendering: the web's own keep-alive, which exact2's runner retention already plays the part of (§5.3).
