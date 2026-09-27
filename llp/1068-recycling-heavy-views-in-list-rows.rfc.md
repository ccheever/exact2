# LLP 1068: Recycling heavy native views in list rows

**Type:** RFC
**Status:** Draft (r2: two reviews folded, §0; five questions await Charlie, §10)
**Systems:** Apple host (`NodePoolIOS.swift` and its reset contract; `NodeViewIOS.swift`'s material, scroll and field views; `GpuIOS.swift`'s canvases; the video, web and native-module arms), GPU module ABI (LLP 1009: stage 3 only), native-module ABI (LLP 1024: one optional entry, stage 3 only), Runner (none), Contract (none)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** none named. `rules/RULES.md` wants one before this is built; the lane that builds the Extra Heavy feed (`~/bench/xheavy`) is the natural owner of stages 1–2, and Charlie names it when he rules
**Date:** 2026-09-27 (r1 and r2)
**Related:**
- `~/bench/xheavy/EXACT2-GAPS.md` gap 1 (the audit this answers) and `SPEC.md` (the 17 row kinds).
- LLP 1010 §6 (the collection; row state dies on retirement, `:334–340`, `:729–730`).
- LLP 1050.000 D1 and D3 (never blank by default; a costly row is never built mid-fling) and §7 (view reuse landed on iOS, `:294–302`).
- LLP 1009 D2 (a surface instance is per canvas node, `:72–74`) and D4 (the target is registered on create and unregistered on destroy, `:99–101`).
- LLP 1056 D10 (2D canvas pooling), LLP 1024 D4 (the native table), LLP 1042 (video lifecycle, `:58–63`), LLP 1020 (iframe), LLP 1008 §9 (the iOS host).
- `QUEUE.md` "Pooling a list row that holds a canvas needs a ruling" (2026-09-26).
- Reviews: `llp/reviews/1068-recycling-heavy-views-in-list-rows.{astra,grok}.md`.

## Summary

`NodePoolIOS` parks a retired list row's views and hands them to the next
row of the same shape. It refuses any row that holds a video, a web view, a
GPU canvas, a 2D canvas, a native-module view, a material, a live nested
scroll view or a text field (`NodePoolIOS.swift:153–163`); its `kinds` leave
out `native`, `iframe`, `video`, `canvas`, `canvas2d`, `input` and
`textarea` (`:60`). In the Extra Heavy feed about 9 of 17 row kinds
therefore rebuild every view they hold, plain or heavy, each time they
scroll in.

This RFC decides, per heavy kind, whether the host may reuse the platform
view, what a reuse must reset so it cannot be told from a fresh element, and
what to do instead where reuse cannot be made invisible. Four findings
shape it:

1. **Refusal is whole-row.** One text field in a thread row makes its
   labels, images and boxes rebuild too. Pooling *around* a heavy leaf —
   reuse the row's plain views, build the leaf fresh — recovers the
   plain-view savings for every kind without touching any heavy view's
   semantics. React Native arrived at the same design: `react-native-maps`
   and `react-native-webview` recycle their shell and destroy the map or web
   view inside it.
2. **The oracle is "removed, then a new element inserted."** The runner
   never reuses ids and row state dies on retirement (LLP 1010). A reused
   heavy view must be indistinguishable from a new element: time 0, a fresh
   browsing context, an empty draft, a transparent bitmap, offset 0.
3. **The heavy views are where reuse would pay** (§3, simulator): a map
   costs 20–47 ms of main thread to create against 1.7 ms to reuse; a web
   view reaches a new document in 4 ms when reused and about 500 ms when new.
4. **The heavy views are also where a reset is hardest to make complete.**
   Both reviews found state that r1's reset lists missed in every heavy kind
   — a web view's `sessionStorage` and `window.name`, a player layer's last
   frame, a map's last tiles, asynchronous callbacks that resolve the node's
   id after a rebind. So heavy reuse is staged behind proof, one kind at a
   time, and the first stage reuses no heavy view at all.

**Per-kind decisions (recommendations for Charlie):**

| Kind | Decision | Stage |
|---|---|---|
| Any row with heavy leaves | Pool the row around them: plain views reused, each heavy leaf destroyed at park and created fresh at take | 1 |
| Material (`UIVisualEffectView`) | Treated as a heavy leaf: destroyed and recreated (0.33 ms); the row pools | 1 |
| Nested scroll view (carousel) | Pool when fully at rest, with the full scroll reset (§4.2) | 2 |
| Text field / text area | Pool when idle, with the full editor reset (§4.3) | 2 |
| 2D canvas (`canvas2d`) | As LLP 1056 D10 rules (children allowed) | LLP 1056 stage 3 |
| GPU canvas | Profile `gpu_create` in the real app first; then, if it pays, pool the `MetalView` and layer with a fresh instance (LLP 1009 D2 holds) and a presentation signal before unhiding | 2 (profile), 3 (pool) |
| Video | Not pooled until the iPad measures it; the player pool amends LLP 1042's "one node incarnation owns one player" | 3, gated |
| Web view (`iframe`) | Not pooled. A reused `WKWebView` is a reused browsing context; the fresh one costs its row ~0.5 s to content (simulator), which is declared | not now; stage 3 experiment |
| Native-module view (map) | Never pooled without the module's opt-in, keyed by artifact and tag | 3, gated |
| Heavy leaf created mid-fling | A declared deviation: a leaf whose *measured* creation cost exceeds the frame is created when the list slows; its row is present | 2 |

The questions for Charlie are in §10.

## 0. What r2 changed (reviews by Astra and Grok)

Both reviews: "build with named changes." Dispositions are in the review
files. The changes, in order of weight:

- **Heavy reuse moves behind proof.** r1 pooled web views, video players,
  GPU layers and (opt-in) maps in stages 2–3 with reset lists both reviews
  showed incomplete (§4.5–§4.8). r2 keeps only row pooling around heavy
  leaves in stage 1; each heavy kind now names a gate and a fixture that
  must look for the leaks the reviews found.
- **Web views are not pooled** (both reviews). A new `<iframe>` does not
  inherit `sessionStorage` or `window.name`, and navigating a `WKWebView`
  does not synchronously end the old document. r1's claim that the
  back-forward list was the only deviation was wrong.
- **An incarnation rule** (§4.9, Astra's blocker). Any object that outlives
  a row needs every asynchronous callback to carry a token of the node
  incarnation it was issued for; object identity and "the owner's current
  id" are not enough.
- **§5.1 is a declared deviation, not `loading="lazy"`** (both). Lazy loading
  is proximity to the viewport; a visible row mid-fling is in it. r2 narrows
  it to kinds whose measured creation cost exceeds the frame, defines its
  hit, focus, props and settle behaviour, and asks Charlie (Q2).
- **§4.0 is larger than r1 said** (both): `shape()` rejects any non-node
  subview and any node whose `container` is not itself, `take` zips a
  complete preorder array, and `rebind` does not restore the presenter's
  indexes. §4.0 now specifies holes, full-subtree validation and index
  restoration.
- **Materials are recreated, not reused** (Grok; Astra's sequencing
  finding points the same way): `rebind` clears props and the next
  `updateMaterial` would destroy the retained view, a hidden effect view
  can hold a stale backdrop, and creation costs 0.33 ms.
- **§3's reading is corrected.** Web view (4.4–7.7 ms) and video (6.9 ms)
  creation do not individually exceed an 8.3 ms frame, as r1 said; the map
  does. The raw-Metal row is not wgpu's cost, so the GPU decision waits on
  a profile of `gpu_create`. The Mac and second-run numbers are added.
- **§6's arithmetic is fixed**: a C × 220 pt layer at 2× with three BGRA
  drawables is about 6.3 MB at SPEC's C = 600, not 3.5 MB per drawable ×3
  into 16 MB. Caps are trial values; the parked caps bound parked views
  only.
- **§5.3's reason is corrected** (Astra): a host keep-alive is rejected
  because it changes row lifetime on one host, not because the agent could
  not reproduce it.
- **Baseline first** (Astra): the unpooled port is measured before stage 1.

## 1. What the pool is today

The collection's runner retires a row by destroying its nodes and creates
the next under new ids (`runner/src/instance/collection/api.rs`). On iOS,
`Presenter.apply`'s `destroy` case asks the pool first
(`PresenterIOS.swift:702`): a row whose whole subtree the batch destroys, in
a collection's list, of an eligible shape, parks — its views stay in the
list, the root hidden, each view released from the presenter's maps and
`recycle()`d. A row created later in a batch with the same shape (kinds and
child counts in preorder) `take`s them and applies its create ops. The file
header (`NodePoolIOS.swift:15–43`) is the reset contract. On listbench the
presenter's share of a built row fell from 1.23 to about 0.7 ms (LLP
1050.000 §7).

Properties of today's pool that matter here:

- **Refusal is whole-row.** `retire` requires every view in the tree to be
  `recyclable` (`:143`). `shape` returns nil for a kind outside `kinds`, for
  a node whose `container` is not itself (a scroll, an overlay, glass, a
  clip box, `:118`) and for any subview that is not a node other than the
  symbol glyph (`:121–123`) — so an effect view, a `UIScrollView`, a
  `MetalView`, a `WKWebView`, a `UITextField` or a video container fails
  before eligibility is even asked.
- **`take` binds by position.** `take` walks the new subtree in preorder and
  zips its ids with the parked tree's views (`:191–207`).
- **`rebind` does not restore the presenter's indexes.** `release` removes
  the old id from `scrollers`, `materialNodes` and the rest
  (`PresenterIOS.swift:774`); those sets are filled by property observers
  (`NodeViewIOS.swift:284–288`, `:1009–1013`) that a rebind does not fire.
- **A full pool refuses; it does not evict.** `retire` returns false when
  `count ≥ 32` or the shape already has 8 (`:137`, `:141`). With 17 row
  kinds the pool can fill with shapes that no longer scroll in. `drop`
  forgets a tree's views but leaves its root in the list (`:104–107`); the
  callers that remove it today are `reset` and the list leaving the window.
- **iOS only.** macOS has no pool; the web never reuses DOM nodes
  (`host/web/glue.js:745–749` removes the element and, for a video, pauses
  it, removes `src` and calls `load()`). The web is the oracle.

## 2. The oracle

exact2's lifecycle is that a row leaving the window loses its view
instances and component-local slots, on every host (LLP 1010 `:334–340`).
On the web that is literally removing elements and inserting new ones. Per
kind, what a page shows:

| Kind | Removing the old element | The new element | Carries over |
|---|---|---|---|
| `video` | HTML's removal steps run the internal pause steps | `currentTime` 0, `HAVE_NOTHING`, a fresh buffer; poster or transparent until the first frame | nothing but the HTTP cache |
| `iframe` | the child navigable is destroyed ("destroyed, not unloaded"): document, timers, audio, scroll, focus | a new child navigable loading `src`; `load` fires; a new `sessionStorage`; an empty `window.name` | origin-scoped storage and cookies only |
| `canvas` / `canvas2d` | the bitmap and its context go with the element | transparent black | nothing |
| `input` / `textarea` | value, selection, undo history, composition | empty unless `value` is set; the dirty-value flag clear | nothing |
| scroll container | its offset | offset 0 (the start edge; the right edge in RTL) | nothing |
| `backdrop-filter` | stateless | stateless | nothing |
| native module | `destroy` (LLP 1024 D4) | a new instance from `create` | what the module keeps outside its instance |

**The rule:** a reused platform view must be indistinguishable from a
fresh one to the app, to guest content, to the agent (LLP 1012) and to the
reader — including its first pixels. What may carry is only what a page
cannot observe either: processes, decoders, layers, drawable pools, caches.

That is a statement about exact2's declared lifecycle, not about every
virtualized list on the web (a page could keep elements alive itself). The
app keeps what should survive scrolling — a draft, a video's time, a
carousel's offset, a map's camera — in keyed parent data, as LLP 1010 says
of drafts. The xheavy SPEC already keeps drafts and thread expansion that
way.

## 3. Cost of creating versus reusing

Measured 2026-09-27 with a scratch UIKit/AppKit probe outside the repo: an
iPad Pro 11-inch (M5) simulator on iOS 27.0, and the Mac (M5 Max, macOS
26.6); main thread, 3 warm-ups, then N = 30 (N = 10 for new-region maps),
median with p90 in brackets, in ms. "Create" is init, insertion into a view
in the window, layout and `CATransaction.flush`. "Reuse" is unhide, move and
the kind's own re-point (a new HTML document, a player moved to another
layer, a new region, a same-size frame); it does not include a reset or a
rebind, which is part of why these numbers rank kinds and do not budget
them. The machine was shared with other sessions; a second map run moved
its create time about 2×, so read every figure as ±50%.

| Kind | Create (sync), simulator | Reuse (sync), simulator | To content, new vs reused | Memory per live view | Mac: create vs reuse |
|---|---|---|---|---|---|
| Plain view + label (reused today) | 0.35 (0.43) | 0.08–0.18 | — | 0.07 MB | 1.09 vs 0.38–0.58 |
| `UIVisualEffectView` | 0.33 (0.52) | 0.10–0.15 | — | ≈ 0 | 0.84 vs 0.14 |
| `UIScrollView` + 10 cards | 1.78 (2.02) | 0.47–1.49 | — | 0.32 MB | 3.92 vs 0.56–1.05 |
| `UITextField` | 1.56 (2.42) | 0.54–0.57 | — | 0.04 MB | 2.50 vs 0.35–0.87 |
| `UITextView` | 2.03 (2.29) | 0.61–0.84 | — | 0.06 MB | 1.70 vs 0.72 |
| `CAMetalLayer` + first frame (raw Metal, **not** wgpu) | 2.91 (3.58) | 0.97 same size; 2.55 new size | — | not visible on the simulator | 1.44 vs 0.77 |
| `AVPlayer` + item + layer | 6.4 (11.1) + 0.5 insertion | 0.3 (player to another layer) | first frame 27.7 vs 11.0 (`replaceCurrentItem`) | 1.06 MB in the app | 0.73 + 0.35 vs 0.31; first frame 10.7 vs 11.3 |
| `WKWebView` | 4.4 (init 1.7 + insertion 2.7); 7.7 with a fresh configuration, as the arm makes today | 0.7 | `didFinish` 505 (584) vs 4.0 (6.4) | 0.15 MB in the app + one WebContent process each, 14–15 MB | `didFinish` 70–90 vs 1.2; WebContent ≈ 50 MB each |
| `MKMapView` | 19.6 (init 13.7 + insertion 5.9); 47 on a second run | 1.7 | rendered 119–156 (cached region); new region network-bound | 17 MB in the app | 16.6 vs 1.3; 59 MB each |

Also measured: the first instance of each kind costs far more (a cold map
139–142 ms, a cold web view 198 ms before a first load of 1.1 s, a cold text
view 23 ms); every web view observed got its own WebContent process (10
views, 10 processes plus a spare), though Apple documents that WebKit may
share one; and **destroyed maps are not released promptly**: after about 48
create/destroy cycles the app kept about 100 MB on the simulator and 300 MB
on the Mac beyond its live maps. `didFinish` is not first visible content,
and `mapViewDidFinishRenderingMap` never fired for a move between two cached
regions, so the reused-map figure is `setRegion` plus a flush.

What the table supports:

- **The map is the one heavy view whose creation alone exceeds a 120 Hz
  frame** (8.3 ms), by 2–5×, and whose churn retains memory. A web view
  (4.4–7.7 ms) and a video (6.9 ms) come close to a frame on the simulator,
  and are far cheaper on the Mac (1–1.1 ms): device numbers decide them.
- **For a web view the asynchronous half dominates**: a fresh one reaches
  its document 0.5 s (simulator) or 70–90 ms (Mac) after it scrolls in.
  Only reuse removes that; no deferral does.
- **A Metal layer saves only at the same drawable size**, and says nothing
  about wgpu's `create_surface` and `configure`, which cryptobench names as
  the cost (GAPS §2). The GPU decision needs that profile (§4.5).
- **Materials, scroll views and fields are cheap either way.** Their value is
  keeping the rest of their row eligible.
- **What §8 must measure instead** (Astra): complete retire → reset → take →
  first correct pixel cycles on the iPad, fresh and reused interleaved, same
  and different sources and sizes, warm and cold caches, with the arms as
  configured; p95/p99 hitches, pool hit rate, settle latency, pending
  content area × time, and stale pixels.

## 4. Per-kind decisions

### 4.0 Pooling around heavy leaves (stage 1)

**Decision: a row's plain views pool even when it holds heavy leaves; each
heavy leaf is destroyed at park and created fresh at take.**

A heavy leaf is a node of kind `video`, `iframe`, `native`, `canvas`,
`canvas2d` (until LLP 1056 D10), `input` or `textarea` with no node
children, or a node's material view (§4.1). The mechanics r1 understated:

- **One shape walk, with holes.** Both `shape` walks (the parked tree's at
  retire and the created subtree's at take, `:117` and `:191`) emit the
  heavy leaf as a typed hole (`video*`) and skip it identically; the parked
  tree stores its views with explicit holes, so `take`'s zip binds every
  plain id to the view it was parked from and leaves each hole unclaimed. A
  test asserts that binding on a mixed tree.
- **Walk logical children.** The walk goes through each node's logical
  child container rather than rejecting a node whose `container` is not
  itself, and ignores a node's own platform subviews (the material view,
  the symbol glyph). A node with a live `UIScrollView` still ends the walk
  as ineligible until §4.2.
- **Validate the whole subtree before destroying anything.** Every node,
  heavy leaves included, must be destroyed by this batch and eligible; only
  then are the heavy leaves destroyed (`release` + `forget` +
  `removeFromSuperview`, the unpooled path, so `VideoView.invalidate`,
  `destroyEmbedded` and `Canvases.destroy` run unchanged) and the rest
  parked.
- **Take.** Unclaimed ids fall through to the presenter's ordinary create
  (`PresenterIOS.swift:643–651`), and the batch's `children` op inserts them
  at their index in the reused parent; no new op is needed.
- **Restore the presenter's indexes at take.** `scrollers`,
  `materialNodes`, `textViews` and any other id-keyed set filled by a
  property observer are re-entered under the new id.
- **Eviction.** A retire into a full pool drops the least recently parked
  tree across all shapes (a parked generation counter, not the last slot of
  one shape), and eviction removes the tree's root from its superview.
- **Size.** Both shape walks, the hole bookkeeping, the index restore and
  eviction: a few hundred lines in `NodePoolIOS.swift` and its tests, not
  r1's "about 60."

What it saves: the row's text, image, box and button views for a thread,
video, map, web view or shader row — everything but the heavy view.
The oracle is untouched: every heavy view is exactly as fresh as today.
When a later stage pools a kind, its leaf parks with its row under that
kind's cap (§6) instead of being destroyed.

### 4.1 Materials (stage 1: recreated, not reused)

The material's `UIVisualEffectView` is a leaf of §4.0: destroyed at park,
created by `updateMaterial()` from the new row's props at take. It costs
0.33 ms (§3). Reusing it would need create sequencing the presenter does not
have (`rebind` clears props, so the next handler or style update would call
`updateMaterial` with no material and remove it), a proof that a hidden
effect view does not show a stale backdrop, and, for interactive glass, a
settled effect state that `pressed == false` does not establish. None is
worth 0.2 ms. `materialNodes` is restored under the new id (§4.0).

### 4.2 Nested scroll views (stage 2)

**Pool the `UIScrollView` when it is fully at rest.**
- **Ineligible:** tracking, dragging or decelerating; any programmatic
  scroll or smooth-scroll animation in flight; a refresh control that is
  refreshing; a pending authored scroll; a zoomed scroll view.
- **Park clears, with the delegate suppressed:** `contentOffset` to the
  start edge (the right edge in RTL), `contentSize`, `contentInset` and
  indicator insets, zoom scale and its limits, the refresh control,
  `keyboardDismissMode`, and exact2's own scroll state: `beforeLayoutScroll`,
  `hiddenScroll`, `followedScroll`, reading anchors, `anchoredScrollTop`,
  `retainedScrollTop`, `lastScrollEvent`, the queued scroll-event
  generation, `scrollNeeded` and the pump's entry (`NodeViewIOS.swift:682`,
  `:906–993`; `scrollPump.forget` is already in `release`).
- **May carry:** the `UIScrollView` and its pan recognizer.
- **Take:** `scrollers` re-entered under the new id; `fitScroll()`,
  `syncScroll()` and `updateRefresh()` from the new node.
- **Proven by:** a carousel row scrolled, retired and taken shows offset 0,
  and a later layout does not restore the old offset.

### 4.3 Text fields and text areas (stage 2)

**Pool when idle.**
- **Ineligible:** the descendant control (not the node) is first responder
  or editing; marked text; a pending focus; secure text entry; a strong
  password or autofill suggestion showing (`textContentType` of a password
  or one-time code kind is ineligible outright, since there is no public
  test for "autofill in progress"); a `markup` editor (its bookmark,
  selection state and native undo integration, `TextAreaIOS.swift:38–84`,
  `MarkupEditor.swift`, are not reset here).
- **Park clears, without firing the old node's handlers:** `text` and
  `attributedText`, selection, `interactionState`, the undo manager's
  actions (again after the take's initial `value` write), `pendingValue`,
  typing attributes, placeholder, input traits; for `UITextView`, its
  content offset and zoom.
- **Take:** traits from props, then `value`.
- **Proven by:** type, retire, take, send `undo`: nothing changes; a
  composing or focused field never parks.

### 4.4 2D canvases (`canvas2d`)

LLP 1056 D10 rules this, stage 3 of that RFC. Until then a `canvas2d` is a
heavy leaf of §4.0 and is destroyed. D10 allows a 2D canvas ordinary
children, so the leaf rule of §4.0 does not apply once D10 lands; a retained
bitmap must also reset the replayer's clip, transform, save stack, path and
gradients, not only the pixels, and stays under D10's budget. D10's
citation `NodePoolIOS.swift:117` is now `:60` (`kinds`) and `:136`
(`retire`).

### 4.5 GPU canvases (stage 2: profile; stage 3: pool the layer)

LLP 1009 D2: "Instances are per canvas node, created when the module is
up, dropped with the node" (`:72–74`). The instance — the app's `Surface`
object and its `bind`/`render` state, publisher and carry — is the node's,
and a new `<canvas>` agrees. Nothing here moves an instance.

**Stage 2 is a profile, not a pool.** §3 measured raw Metal, not
`gpu_create`. On the crypto-list GPU ladder at 24k and 48k pt/s, Time
Profiler splits a row's canvas cost into the UIKit view and layer,
`create_surface_unsafe`, `configure`, first drawable allocation and first
render. Pooling the layer can save only the first and part of the fourth.

**Stage 3, if the profile shows that share is large: pool the `MetalView`
and layer, never the instance.** A second `wgpu::Surface` on the same
`CAMetalLayer` after the first is dropped is what device-loss recovery
already does (`gpu/src/recovery.rs:30–49`). The reset:
- **Park:** `Canvases.destroy(view:)` as today (`gpu_destroy`, D2 and D4
  unchanged); `canvasInput` dropped; the `MetalView` hidden. A canvas whose
  overlay holds children is ineligible (so a shader row whose photo is an
  overlay child pools only its chrome; the Extra Heavy port should feed the
  photo as asset bytes, `gpu/src/lib.rs:188–201`).
- **Take:** `contentsScale` set again (a parked view never moves windows,
  so `didMoveToWindow` does not run); the old surface's drop finished
  before `create_surface_unsafe` on the same layer; a lost device while
  parked handled by the ordinary create path.
- **Unhide on a presentation signal, not on `render`'s return.** `gpu_render`
  returns "wants another frame", which a timeout or occlusion also returns
  (`gpu/src/lib.rs:1129`). The pool needs the module to report that a frame
  of *this* instance was presented, which is a change to the GPU ABI; and
  hidden views must not be drawn into until then (`GpuIOS.swift:495–508`
  does not check `isHidden`).
- **Later, only on a second profile:** a module that keeps the configured
  surface per layer. That needs a cache per GPU artifact (LLP 1009 D6),
  release on every destroy path (unpooled destroy, refusal, failed create,
  session reset), invalidation on device loss (recovery walks live
  instances only) and an amendment of D4's "unregisters it on destroy."
  It is not proposed here.

### 4.6 Video (stage 3, gated)

**Not pooled until the iPad measures it.** The simulator says creation is
6.9 ms and a reused player reaches its first frame 17 ms sooner; the Mac
says creation is 1.1 ms and the reused first frame is no sooner. Apple and
feed apps favour one player per slot with `replaceCurrentItem`, mainly
because item/player pairs build render pipelines that exhaust hardware
decoders (Appendix) — but a parked-player cap does not bound the players
the visible rows hold, so it does not settle that either.

If the iPad shows a saving worth a frame, the pool is the `AVPlayer` and its
`AVPlayerLayer` (cap 2), never the item, and:
- **It amends LLP 1042** `:58`, "One node incarnation owns one player", to
  "… owns one item; a player may be reused".
- **Eligible:** the inline layer presentation only. Today the arm installs
  an `AVPlayerViewController` unless the node sets
  `allowsVideoFrameAnalysis=false`, `allowsPictureInPicturePlayback=false`
  and `playsinline=true` (`VideoArm.swift:264–287`), so by default no video
  is eligible — and a feed's inline muted video pays for a view controller
  per row, which the web's `<video>` has no equivalent of. Whether the
  layer should be the default without `controls` is LLP 1042's question;
  it goes to `QUEUE.md`.
- **Park:** pause; remove the item; remove item-level KVO, notifications and
  the periodic time observer (re-added at take); keep player-level KVO;
  bump the generation and poster generation; clear the arm's `props`,
  `wantsPlay`, `naturalSize`, `pendingSeek`, `lastError`, `lastPaused`,
  `lastTimeStatus`, `defaultRate`; clear the host `VideoView`'s `last`,
  `observed`, `intrinsicSize` and `visibilityBlocked`
  (`VideoModule.swift:47–80`) — otherwise a take with the same `src` never
  loads; hide the layer view.
- **Pixels:** the layer keeps the last frame after the item is removed, so
  it stays hidden until the new item's `isReadyForDisplay`.
- **Callbacks** follow §4.9: the host's queued intrinsic-size and event
  deliveries check `owner.video === self` and then use the owner's current
  id (`VideoModule.swift:145`), which after a rebind is the new row's.

### 4.7 Web views (not pooled)

A `WKWebView` that loads a new document is still the same top-level
browsing context. Against the oracle's fresh child navigable it carries:
- `sessionStorage` of the wrapper origin (`https://exact.invalid`,
  `WebArm.swift:209–212`) and `window.name`;
- the joint session history, which guest script can read and traverse
  (`history.length`, `history.back()`);
- the old document until the new navigation commits: loading is
  asynchronous, so its script, audio and network run on after "park", and
  hiding stops only the pixels;
- arm and host state bound to the creation: the id in `wrapperURL` and in
  `WebCallbackBox`, `WebViews.entries`, `servePending`, `serving`,
  `recovering`, `suppressLoad`, `guestFrame`, outstanding snapshot and
  evaluation replies, and the `exactAgent` document-start message, which
  carries no generation (`WebArm.swift:57–69`, `:146`, `:266`, `:321`;
  `WebModule.swift:86–177`).

Each could be reset (a per-mount wrapper origin would scope
`sessionStorage`). But "not yet shown to leak" is the bar, and the reset
list keeps growing. **So r2 does not pool web views.** The row pools around
the web view (§4.0), and the cost is declared: a web-view row shows its
background for about 0.5 s on the simulator (70–90 ms on the Mac) after it
scrolls in, until its WebContent process loads the document. A stage-3
experiment may revisit this with a fixture that, in the reused view, reads
`sessionStorage`, `window.name`, `history.length`, audio and timer activity
of the old document, and the process id — and passes only if each matches a
fresh iframe's.

### 4.8 Native-module views (stage 3, gated)

LLP 1024's table has no reuse entry: `destroy` "is the last call" (D4,
`:287–294`). The host cannot know what a module's view holds.

**Never pool a module view without the module's opt-in.** Fabric
recycles by default and every Expo module opts out ("it may lead to more
bugs than gains"); `react-native-maps` destroys its map on recycle.

The opt-in, if stage 2's run shows map rows still costing frames when the
list comes to rest (the map's 20–47 ms then lands on the settle frame):
- **Table:** one optional entry at offset 72 (`size ≥ 80`, the major stays
  1), and a roster bit per tag (`{"native-map": {"reuse": true}}`):
  `prepare_for_reuse(handle) → i32`, 0 when the instance is now as if
  created with no props.
- **Pooled by artifact and tag**, never by `kind == "native"` alone.
- **Callbacks** follow §4.9: the host retires the old event context object
  at park (so any callback from the old row is dropped as a destroyed
  instance's is, LLP 1024 `:303–313`) and hands the instance a new one at
  take; a nonce used while parked is never activated.
- **Fresh-mount semantics:** the first `set_props` after a take must behave
  as a first mount (for `NativeMap`, `first: true`: no animated region
  change, and its `load` event, `NativeMap.swift:42–66`).
- **Pixels:** the view stays hidden until the module reports the new
  content ready; a map's last tiles otherwise show under the new region.
- **`NativeMap`'s reset:** props cache, annotations, overlays, selection and
  callouts, the annotation-view reuse queue's views, camera, user location
  and tracking, `mapType` and interaction flags, pending snapshot work.
- **Cap 1** (17 MB on the simulator, 59 MB on the Mac).

### 4.9 Incarnations (every pooled heavy object)

An object that outlives its row needs a lifetime boundary that asynchronous
work respects (Astra's blocker). Any pooled heavy object — a scroll view's
delegate traffic, a field's notifications, and in stage 3 a Metal layer, a
player or a module view — follows one rule:

- every asynchronous callback captures the incarnation token current when
  it was issued, and is dropped if that token is no longer current;
- park invalidates the token *before* the reset begins;
- take issues a never-used token and activates it only after the new
  node's id, props and handlers are installed;
- nothing resolves "the owner's current id" at delivery time.

## 5. Alternatives to pooling

### 5.1 A costly heavy leaf is not created mid-fling (stage 2; a declared deviation)

**What it is.** During user motion (a drag or a fling, the signals of LLP
1050.000 §2.2), a heavy leaf whose *measured* creation cost exceeds the
host's slice is not given its platform view. Its node exists, its box is
laid out and painted (background, border, radius) and the rest of its row
is built. When the list's velocity falls under the threshold 1050.000 uses
for D3, or the row comes to rest, the host creates the view.

**Which leaves.** Those the host has measured, per kind, at more than its
slice (`FillCost`, `ScrollPumpIOS.swift:44–48`, extended by kind). On the
simulator that is the map; on the iPad it is whatever the §8 run finds.
A video is included only when its box's size does not depend on its
metadata (both dimensions definite); a GPU canvas never (§4.5 first).

**Why it is a deviation.** It is not `loading="lazy"`: HTML's lazy loading
is proximity to the viewport, and a visible row mid-fling is in it; a
browser would start the load. It is not D3 either: D3 withholds a whole
over-budget row from the owed set (1050.000 `:29–33`, `:235–239`); this
builds the row and withholds one view. D1 holds: the row is present, never
a gap. It is a third policy, narrower than D3, and needs Charlie's ruling
(Q2).

**Its behaviour, defined:**
- **Props:** the latest props at creation are applied; intermediate ones are
  dropped, as a batch coalesces them.
- **Events:** none fire before creation; `load`, `canplay` and a module's
  own events come later, as they would after a slow load.
- **Hits:** the box takes hits as an empty box of its kind; a press on it
  creates the view at once (a pointer-down is not motion).
- **Focus and accessibility:** a focus, keyboard or accessibility move into
  the leaf makes it owed, as a move into a pending row is (1050.000 §6).
- **Retirement:** a row retired before its leaf was created creates nothing.
- **The agent:** `state` shows the leaf as `pending`; `clock settle` ends
  motion, so every leaf is created, though a web view's or a map's own
  content may still be loading (settle does not wait on WebKit or MapKit).
- **The benchmark:** the probe counts a pending leaf's box as content
  missing, even when its background would pass the blank detector.

**Contract.** Nothing new. An app that wants eager creation in a list asks
for it when one does; the web's name would be `loading="eager"`.

### 5.2 Pausing video off screen

Specified: `playbackVisibilityThreshold` pauses a video below a visible
fraction, with an authored `paused` binding (LLP 1042 `:94–102`). Rows past
the window are retired anyway. No change; the Extra Heavy port authors the
binding.

### 5.3 A keyed keep-alive set (rejected)

Keeping a heavy view alive by row key — so scrolling back finds the same
page or the same video frame — changes row lifetime on one host: iOS would
keep an instance the web and Linux retire, and a row's state would depend on
whether its key was recently seen there. LLP 1010 `:339–340` rejects the
unbounded form outright. The keep-alive exact2 has is the runner's
retention (a row retires two viewports away, or when the kept rows
outnumber the window's, LLP 1050.000 §6), the same on every host; it plays
the part of the web's `content-visibility: auto`. If a heavy kind needs a
longer life, that rule is the lever.

### 5.4 Posters and snapshots

A snapshot of a row's last content is a keep-alive by another name (§5.3).
What remains is authored: a `poster` on a video, the same on every host;
and a module may render a static image itself (Apple recommends
`MKMapSnapshotter` for maps in lists). The Extra Heavy map row wants live
tiles ("maps in a list are the point", SPEC), so it does not use one.

## 6. Pool sizing, memory and eviction

Trial values, to be replaced by the iPad's numbers.

- **Trees:** 32, at most 8 per shape, least-recently-parked eviction across
  shapes (§4.0). The cost of a parked tree is its views' retained graphs
  and layers, not the ~720-byte `NodeView` allocation `QUEUE.md` counted;
  `recycle` already drops their bitmaps.
- **Heavy views parked inside trees, per kind:**

  | Kind | Cap | Why |
  |---|---|---|
  | scroll view (stage 2) | 8 | its content is plain views, counted with the tree |
  | text field / area (stage 2) | 4 | a feed rarely shows more |
  | GPU layer (stage 3, if built) | 2 | about 6.3 MB of drawables each at C = 600, 220 pt, 2×, three BGRA drawables |
  | video player (stage 3, if built) | 2 | 1 MB in the app; decoders are shared hardware |
  | native-module view (stage 3, opt-in) | 1 | 17 MB on the simulator, 59 MB on the Mac |

  A kind at its cap evicts its least recently parked view (with its tree)
  rather than refusing the newer one. These caps bound *parked* views only;
  the visible window holds its own heavy views on top, which is why the
  memory-pressure path matters more than their sum.
- **Drawables at park** stay resident: shrinking `drawableSize` would free
  them, but a reuse at a new size costs what a new layer does (§3).
- **Memory pressure:** a memory warning drops every parked tree
  (`NodePool.reset()`); entering the background drops parked players, layers
  and module views.
- **Counted:** `state`'s collection section reports parked trees and parked
  heavy views by kind, and pending leaves (§5.1), so the smokes and the
  benchmark see what the pool holds. It is a field in an existing reply, not
  an operation.
- **Measured separately** on the iPad: the app's footprint, WebContent
  processes, and GPU memory (`BENCH_VM=1`).

## 7. `rules/DEFERRED.md`

Nothing comes off the list: every kind here is already admitted (video
2026-09-18, `iframe` 2026-08-30, native modules 2026-09-26, Canvas 2D). One
line goes on, under "Features carried over as no", beside the virtualList
line:

> - No host keep-alive of heavy views by row key, and no reuse of a web view
>   or of a native-module view without that module's opt-in (LLP 1068 §4.7,
>   §4.8, §5.3). A reused view is indistinguishable from a new element;
>   state that must survive scrolling lives in keyed data.

It adds a "no", which the file does not charge for.

## 8. Staging, apparatus and measurement

| Stage | Ships | Proven by |
|---|---|---|
| 0 | Nothing: the Extra Heavy port ships unpooled and is measured (the audit's fallback) | the baseline for every later stage |
| 1 | §4.0 (holes, full-subtree validation, index restore, cross-shape eviction), §4.1 (materials recreated), parked counts in `state` | `NodePoolIOSTests`: for each heavy kind, a row holding it parks, the leaf is a new platform view (new identity, new arm handle), its siblings are the parked views bound to the right ids (a mixed tree); a material row comes back with a new effect view and in `materialNodes`; eviction removes the root. The Extra Heavy ladder against stage 0 |
| 2 | §4.2 scroll views and §4.3 fields, each under §4.9; §5.1 deferral (if Q2 is yes); the GPU profile (§4.5) | Scroll, field and deferral tests as their sections say. The crypto-list GPU ladder (cryptobench `exact-gpu`) at 24k and 48k pt/s with Time Profiler, attributing `create_surface`, `configure`, drawable allocation and first render |
| 3 | Each only if its gate passes: GPU layer pool with a presentation signal (§4.5); video player pool and the LLP 1042 amendment (§4.6); the native opt-in with `NativeMap` adopting it (§4.8); the web view experiment (§4.7) | Per kind, a stale-pixel test (the first pixels after a take are the new content's or the box's background, never the old row's) and its leak fixture (§4.6–§4.8); the same ladders |

**Apparatus.** Nothing new in the repo. Tests go in the existing
`host/apple/tests/ExactKitTests/NodePoolIOSTests.swift`. Fixtures are the
existing apps (`apps/video-player`, `apps/map-demo`, `apps/canvas-gallery`,
`apps/sparkline`, Caltrain's `iframe` deck), placed in a list by the tests'
own plans. The smokes are the existing `smoke.mjs ios` and `smoke.mjs
canvas`; driving is `scripts/agent.mjs ios`. Benchmarks stay outside the
repo (LLP 1050.000 D4): `~/bench/xheavy` and `~/bench/cryptobench`, with the
probe in `~/bench/heavybench/probe`. The new ABI entries of stage 3 are
interface, not apparatus.

**Measured on `~/bench/xheavy`**, M1 iPad Pro 12.9" in landscape, SPEC's
`fling` and `live` scenarios, three interleaved rounds against the previous
stage: fps and p95/p99 frame time per ladder speed; main-thread CPU ms/s;
footprint peak, end and 10 s after, with WebContent and GPU memory apart;
blank area and pending-leaf area × time; heavy platform views created per
second, from `state`.

## 9. What this does not do

- It does not reuse node ids or kernel nodes; the runner is unchanged.
- It does not bring the pool to macOS. The same contract applies when the
  Mac gets one (its own change); the arms are shared Swift, so §4.6–§4.8's
  resets would be written once.
- It does not change the web, which is the oracle.
- It never moves a surface instance, an `AVPlayerItem` or a document between
  rows.

## 10. Questions for Charlie

**Q1. Build stage 1: pool rows around their heavy leaves (holes,
full-subtree validation, index restore, cross-shape eviction), recreating
materials rather than reusing them?**
Recommendation: yes, after the stage 0 baseline. It reuses no heavy view,
needs no ABI or LLP amendment, and gives every heavy kind's row its plain
views back. Confidence: high (0.8).

**Q2. Accept §5.1 as a declared deviation: during user motion, a heavy
leaf whose measured creation cost exceeds the frame is created when the
list slows, its row present and its box painted?**
Recommendation: yes. On the simulator it applies to the map (20–47 ms)
and nothing else. Confidence: medium (0.6); the alternative is a late frame
per map row mid-fling, which D3 forbids for a whole row.

**Q3. Do not pool web views: accept that a web-view row shows its background
for its WebContent process's first load (~0.5 s simulator, 70–90 ms Mac)
after it scrolls in, and leave reuse to a stage-3 experiment that must prove
a fresh browsing context?**
Recommendation: yes. Both reviews found state a reused `WKWebView` carries
that a fresh `<iframe>` does not. Confidence: medium-high (0.7).

**Q4. GPU canvases: profile `gpu_create` first, and pool the layer (with a
new presentation signal in the GPU ABI) only if that profile shows the
UIKit layer and surface setup are a large share of a row's canvas cost?**
Recommendation: yes; LLP 1009 D2 and D4 stand. This answers the `QUEUE.md`
item. Confidence: medium (0.65).

**Q5. Video players and native-module views: pool neither until the iPad
run shows the saving; then pool players (amending LLP 1042's "one node
incarnation owns one player" to "one item") and add the native
`prepare_for_reuse` opt-in with `NativeMap` as its adopter?**
Recommendation: yes. Confidence: medium-high (0.7).

## Appendix: sources

**Apple (official):**
- `UITableViewCell.prepareForReuse` / `UICollectionReusableView.prepareForReuse`: reset only non-content attributes there; content is set again by the data source — the same split as `recycle` and the create ops.
- WWDC21 10252, "Make blazing fast lists and collection views": prepared cells may never be displayed; cells scrolled off wait before returning to the reuse queue.
- `AVPlayer`: "You can reuse the player instance to play additional media assets using its `replaceCurrentItem(with:)` method." WWDC16 503: removing a player's only layer no longer pauses it. `AVPlayerLayer.isReadyForDisplay`.
- `WKProcessPool`: deprecated in iOS 15; "Creating and using multiple instances of WKProcessPool no longer has any effect." `WKBackForwardList` is read-only.
- `MKMapSnapshotter`: the static map; overlays and annotations are drawn by the caller.
- `CAMetalLayer`: a pool of drawables per layer; `maximumDrawableCount` 2 or 3 (default 3).
- WWDC14 419: a blur is several passes; "budget for effect."
- Apple publishes no reuse guidance for `WKWebView`, `MKMapView`, `UIVisualEffectView` or `CAMetalLayer` in cells, and no cap on the reuse queue.

**Practice (not Apple):**
- Decoder exhaustion (`-11839`) past a device-dependent number of players; an item paired with a player builds a render pipeline that dropping the player does not free (Hansmeyer, "Too Many AVPlayers?", 2017; Apple forums 67382). Feed apps keep 3–5 players and swap items.
- `WKWebView` creation is among the heaviest actions in an iOS app, each with its own WebContent process (Embrace, "WKWebView memory leaks").
- Nested carousels keep a per-item `contentOffset` in the model and restore it on display (Furrow); FlashList's `useRecyclingState` exists to reset exactly that.
- React Native Fabric recycles views per component type (`RCTComponentViewRegistry`, cap 1024, purged on a memory warning), but `react-native-video` opts out, every Expo module opts out ("it may lead to more bugs than gains", `ExpoFabricView.swift`), and `react-native-webview` and `react-native-maps` recycle only their shell: `prepareForRecycle` destroys the web view or map inside — §4.0's design.
- Litho pools mount content per component type, `poolSize` 3 by default, with optional preallocation.

**The web (spec):**
- HTML §4.8.11.8, removing a media element: "Run the internal pause steps." A new element starts at `NETWORK_EMPTY`/`HAVE_NOTHING`, `currentTime` 0.
- HTML, the `iframe` element: the removing steps destroy the child navigable ("the element's content document is destroyed, not unloaded"); insertion creates a new one, which loads `src` again. `moveBefore()` (Chrome 133+) moves an iframe or video with its state; a virtualized list removes rather than moves.
- HTML lazy loading: the `loading` attribute defers by proximity to the viewport, not by scroll velocity.
- HTML canvas: with no context the bitmap is transparent black; the context belongs to the element.
- HTML form controls: the value and the dirty-value flag belong to the element.
- `content-visibility: auto` keeps an element and its state while skipping its rendering: the web's own keep-alive, whose part the runner's retention plays (§5.3).
