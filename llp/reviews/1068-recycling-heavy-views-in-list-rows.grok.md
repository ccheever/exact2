# Review: LLP 1068 r1, 2026-09-27 (grok)

- **Family:** xAI — grok CLI (`grok-4.6-build`), `--reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, cwd the worktree `exact2-wt-poolllp` at 25902e09 (r1 committed); 2026-09-27T18:24:53Z to 2026-09-27T18:36:09Z; stopReason end_turn; 33,474 output tokens (24,504 reasoning).
- **Method:** the shared brief (sha256 `ffc74c3978f2860d03bc5dce8b8fbaeefc07ae011e0c6ed94db77d816f607279`) verbatim; one round; blind to the other review.
- **Reviewed revision:** LLP 1068 r1.
- **Redacted:** the streamed progress lines before the review ("I'll start with the binding rules…", "Create/destroy paths are clear…"); the review text below is otherwise verbatim.
- **Verdict:** build with named changes.
- **Disposition:** LLP 1068 §0 (r2). Accepted:
  - BLOCKER, web reuse leaks `sessionStorage` and `window.name`; §2's iframe cell was wrong: §2 corrected; web views not pooled (§4.7, Q3).
  - BLOCKER, `shape()` and `rebind` cannot park material or carousel rows: §4.0 (logical-child walk, index restore); materials recreated, not reused (§4.1), as recommended.
  - §5.1 is not `loading="lazy"`; declare it (Q2, the recommended fifth question): §5.1 rewritten as a declared third policy.
  - Video and map last pixels, `VideoView.last`, the LLP 1042 amendment: §4.6, §4.8, Q5.
  - GPU: surface drop before re-create, `contentsScale`, hidden views drawn into, cache per artifact and device loss, stage 3 separated from the ruling: §4.5 (the cache is no longer proposed).
  - Fields: `interactionState`, `UITextView` offset, secure text: §4.3 (secure text entry is ineligible rather than reordered).
  - Map: `first: true`, interaction flags, user location, snapshotter, annotation-view queue: §4.8.
  - Measurements: the raw-Metal row is not wgpu's cost; GPU stage 2 is now a profile, not a pool; drawable arithmetic fixed (§3, §4.5, §6).
  - Same-slot skipping in both walks, a global LRU generation, per-kind cap decrement on eviction, RTL scroll origin: §4.0, §4.2, §6.
  - The shader row's overlay photo: §4.5 says the port should feed it as asset bytes to be poolable.
- **Not taken:**
  - "Put `state` for a deferred leaf": taken as `pending` in §5.1, not as a `loading` value.
  - Hiding `webView` rather than the node on rebind: moot, since web views are not pooled in r2.

---

## Review (verbatim)

The RFC’s architecture is right: pool the row around a heavy leaf, treat reuse as “remove + insert a new element,” refuse keyed keep-alive, and require a module opt-in. Several per-kind reset lists and the web-oracle claims are not.

---

## 1. Per-kind decisions and reset leaks

### §4.0 Pool around a heavy leaf — **right (stage 1)**
Destroy the video / iframe / native / canvas / field NodeView; park the rest. Matches how RN maps/webviews recycle the shell. Destroy order is already parent-first (`kernel/src/arena.rs:354–368`, `host/apple/src/host.rs:1151–1160`), so `retire` on the wrapper still sees the heavy child.

### §4.1 Materials — **wrong to pool the effect view**
Create is 0.33 ms (`§3`). The reason to touch them is eligibility, not that 0.2 ms.

`shape()` walks `view.subviews` and demands every subview (except `symbolView`) is a `NodeView` (`NodePoolIOS.swift:117–125`). A `UIVisualEffectView` sits in that list (`NodeViewIOS.swift:1031`). Glass also makes `container !== view` (`NodeViewIOS.swift:636`). As written, a material row still cannot park.

Pixel leak: a hidden `UIVisualEffectView` often keeps a snapshot of the old backdrop. CSS `backdrop-filter` does not. Treat the effect view as a §4.0 extra: destroy and recreate it; skip it in the shape walk like `symbolView`.

Also: `materialView` `didSet` is what inserts `materialNodes` (`NodeViewIOS.swift:1009–1013`). `release` drops the old id (`PresenterIOS.swift:774`). `rebind` does not reassign `materialView`, so the new id never re-enters `materialNodes` and `sendSubviewToBack` at batch end (`PresenterIOS.swift:746`) is skipped.

### §4.2 Nested scroll — **right to pool at rest; reset incomplete**
Same `shape()` / `container === view` wall: children live in the `UIScrollView`, not on the `NodeView`. `scroll.didSet` registers `scrollers` by id (`NodeViewIOS.swift:284–288`); after `rebind` the new id is missing.

Park must also clear: `hiddenScroll` (`NodeViewIOS.swift:907, 988–993`), in-flight `setContentOffset` animations, `minimumZoomScale` / `maximumZoomScale`, `isPagingEnabled`, `isScrollEnabled`, indicator insets, `alwaysBounce*`, `directionalLockEnabled`. RTL: “top-left inset” is not CSS `scrollLeft = 0`.

### §4.3 Fields — **right as stage 2; reset incomplete**
Good: ineligible while focused/composing; undo cleared; traits re-applied; draft lives in keyed data (LLP 1010 `:334–340`).

Missed:
- **`UITextInput.interactionState`** — Apple’s own restore blob (text, selection, undo). Nil it.
- **`UITextView` is a scroll view** — `contentOffset` / zoom leak. Not in the list.
- **`isSecureTextEntry`** — must empty text *before* toggling; UIKit will otherwise show the previous row’s string.
- Markup textarea: `MarkupEditor` bookmark / `NativeTextUndo` (`TextAreaIOS.swift:41–55, 82–84`).
- Autofill strong-password overlay; `inputAssistantItem`.

### §4.4 2D canvas — **right**
Defers to LLP 1056 D10. Line-number update (`:60` / `:136`) is correct. Until that stage, §4.0 destroying the `canvas2d` node is fine.

### §4.5 GPU — **stage 2 direction right; stage 3 not yet**
See §4 below. Extra Heavy’s shader row may put the photo in the overlay (`EXACT2-GAPS.md` shader). Non-empty overlay is ineligible (`§4.0`, `§4.5`). Stage 2 then does not apply to the row this RFC exists to help unless the photo is an asset texture.

### §4.6 Video — **right to pool player+layer, never the item; reset incomplete**
Agrees with LLP 1042 `:58–63` for the *item*. Pooling the `AVPlayer` across node incarnations **amends** “One node incarnation owns one player” and is not in §10.

Missed:
- **Last frame stays on `AVPlayerLayer` after `replaceCurrentItem(nil)`.** Hide until the new item’s first frame (as GPU already does). “Layer shows nothing” is not true.
- **`VideoView.last`** (`VideoModule.swift:47, 123–126`). If `last` is not cleared, `update()` skips the new props.
- Controller vs layer: `configurePresentation` (`VideoArm.swift:264–287`) installs `AVPlayerViewController` unless both analysis and PiP are false. Ineligible must be “has a controller”, not only “PiP/fullscreen active.” Punting the default to QUEUE is fine; Extra Heavy must set both props.
- Time observer: if removed at park, re-add at take. Player-level KVO (`VideoArm.swift:80–85`) should stay; item-level must go.
- `exact_video_reset` must re-point the callback context if `VideoView` is not the pooled object.

### §4.7 Web view — **do not accept as specified**
The 0.5 s vs 4 ms gap is real enough to want a pool. The oracle table (`§2`) is too loose: a new `<iframe>` does **not** inherit `sessionStorage` or `window.name`. Those survive document navigation in the same `WKWebView`.

Must reset, not only declare `history.length`:
- `sessionStorage.clear()` on the wrapper origin (`https://exact.invalid`, `WebArm.swift:209–212`) or a per-mount wrapper host.
- `window.name = ""` on the main frame (persists across loads).
- Unique wrapper URL already includes id (`WebArm.swift:69`); `exactWebRebind` must change it or the next `serve` is a same-URL reload.
- `WebCallbackBox.id` and `WebViews.entries` (`WebModule.swift:86–90, 174–177, 250`) are baked at create; rebind has to move both, or late `load`/`message` land on the wrong node.
- Do not call `invalidate()` on park (it strips script handlers, `WebArm.swift:115–122`).
- Hide **`webView`**, not the `NodeView`: `rebind` unhides the node (`NodePoolIOS.swift:239`).
- `WKWebView.scrollView` zoom/offset (RFC has this); also `isScrollEnabled` (SPEC disables guest scroll).
- In-flight media capture / audio until the empty wrapper commits.

`history.length` as the one declared deviation is incomplete. Cookies/localStorage persisting is oracle-correct; sessionStorage/`window.name` is not.

### §4.8 Native / map — **opt-in is right; pixel leftover is missing**
Never pool without the module’s bit: correct, and what Expo/`react-native-maps` do.

`prepare_for_reuse` at offset 72, `size ≥ 80`, nonce like destroy (`NativeModule.swift:289–295, 304–312`): sound.

For `MKMapView` the RFC resets annotations/overlays/camera/delegate. Missed:
- **Old tiles stay on screen until the new region paints.** Hide (or snapshot) until idle, same as web/GPU. Stage 3’s test (“no last row’s annotations”) does not catch this.
- `NativeMap.apply(..., first: false)` animates region (`NativeMap.swift:63–66`). Reuse must behave as `first: true`.
- `showsUserLocation` / tracking, `mapType`, interaction flags (SPEC disables all interaction), in-flight `MKMapSnapshotter` (`NativeMap.swift:98–114`).
- Annotation-view reuse queue inside MapKit after `removeAnnotations`.

Cap 1 is right (17 MB).

---

## 2. Is §4.0 implementable in take/retire / create / children?

**Yes, with changes the RFC under-specifies. Not “as written” today.**

What already works:
- Parent-first destroy, so `retire(root)` sees the heavy child.
- `take`: unclaimed ids fall through `tried.insert(root)` (`NodePoolIOS.swift:188–189`) and `create` builds a fresh `NodeView` (`PresenterIOS.swift:643–651`).
- `children` inserts that view (`PresenterIOS.swift:674–691`).
- Parked roots stay in the list (`isParked`, `:680`).

What breaks if you only flip `kinds` / `recyclable`:

| Failure | Where |
|---|---|
| `shape()` returns nil on any non-`NodeView` subview (effect view, scroll view, `MetalView`, `WKWebView`, `UITextField`, video container) | `NodePoolIOS.swift:121–123` |
| `container === view` fails for scroll, glass, overlay, `clipBox` | `:118`, `NodeViewIOS.swift:636` |
| `zip(ids, tree.views)` mis-binds if only one of the two shape walks skips the heavy slot | `NodePoolIOS.swift:191–207` |
| `rebind` does not restore `scrollers` / `materialNodes` / `textViews` | `NodePoolIOS.swift:236–251`, `PresenterIOS.swift:774` |
| §5.1 cannot use current `NodeView.init` — video, metal, iframe, native, field are eager | `NodeViewIOS.swift:602–624` |

Stage 1 is ~the two shape functions + skip-or-destroy heavy NodeViews + LRU, **not** “60 lines” if it also claims materials and carousels. Test that `ids.count == views.count` after a mixed tree.

Eviction: `parked` is per-shape arrays (`NodePoolIOS.swift:55`). Global LRU needs a generation, not “the last slot of some shape.”

---

## 3. Web oracle and §5.1 vs LLP 1050.000

**§2 is mostly right; the iframe “storage” cell is wrong; §5.3 is right.**

A new iframe is a new browsing context: cookies/localStorage persist by origin; **sessionStorage and `window.name` do not carry**. `content-visibility: auto` as the analogue of runner retention (two viewports, 1050.000 §6) is fair. Rejecting keyed keep-alive matches LLP 1010 `:339–340`.

**§5.1 is a deviation, not `loading="lazy"`.** Lazy is proximity to the viewport. A visible row during a fling *is* in the viewport; Chrome/Safari would start the load. Velocity-gated creation is host policy, closer to D3 than to HTML.

**D1:** no conflict. D1 is “the row is present, not a pending gap.” A box with background is still a row.

**D3:** not the same grain. D3 (unbuilt, 1050.000 `:235–239, :290`) omits the **whole row** from the owed set. §5.1 **builds** the row and defers the platform view. That is a third policy, and a better default for Extra Heavy than blanking the header. It does not need the cost memo. **Declare it.** Do not call it web-lazy. Put it in §10.

GPU “never wait” (`§5.1`) is based on raw Metal 2.91 ms (`§3`), not wgpu `create_surface`+`configure`. QUEUE.md’s crypto-list (375 creates/s, 99% main thread) contradicts that exclusion.

Agent `clock settle` → create every leaf: good.

---

## 4. GPU

**Reusing `CAMetalLayer` with a new wgpu `Surface` is the existing recovery path** (`gpu/src/recovery.rs:30–49`, `gpu/src/native.rs:164–191`). Park destroys the instance (`GpuIOS.swift:144–149`, `gpu/src/lib.rs:1204–1206`); take calls `gpu_create` later. Same layer, new instance, D2 holds.

Caveats:
- Drop of the old `wgpu::Surface` must finish before `create_surface_unsafe` on that layer. Today’s destroy does not wait for GPU idle. Reuse makes the race real.
- **Stale drawable:** hide `MetalView` until the new instance’s first present (`§4.5`). Also consider `layer.contents = nil`. Showing on first present must happen *after* `m.render`, while hidden views can still obtain drawables (`GpuIOS.swift:495–508` does not check `isHidden`).
- **`contentsScale`:** re-set in `create` (`GpuIOS.swift:214`). Parked views stay in the window, so `didMoveToWindow` (`GpuIOS.swift:26–29`) will not run again.
- **Device:** `configure` sets `CAMetalLayer.device`. Same module device. Device-loss while parked: no instance in `reattach_layers`; take’s `gpu_create` must survive a lost device.

**Stage 3 target cache:** pointer-reuse concern is right; `gpu_release_target` is the fix. Not sound yet:
- Cache **per `GpuModule` artifact** (LLP 1009 D6), not a process-global layer map. A parked layer must not be handed to another dylib’s wgpu instance.
- Device loss must drop or rebuild cached surfaces (`recovery.rs` only walks live `instances`).
- `gpu_destroy` today drops `presentation` with the instance. Stage 3 has to steal the `wgpu::Surface` out first.
- Amending D4 is a real LLP 1009 change (Q2). Gate it on a profile that separates **configure** from **drawable alloc**. §3 never measured wgpu.

---

## 5. Measurements

Plausible as a **simulator ranking**, not as budgets.

| Claim | Verdict |
|---|---|
| Map 20 ms create vs 1.7 ms reuse | Believable; N=10 + Wi-Fi makes 484 vs 1045 unusable |
| Web 500 ms vs 4 ms to `didFinish` | Believable for process spawn + tiny HTML; Extra Heavy’s embed is tiny, Castle decks are not |
| Video 6.4 vs 0.3 ms; first frame 28 vs 11 | Believable; simulator decoder ≠ iPad |
| Metal 2.91 vs 0.97 ms | **Wrong workload.** Raw Metal, not `gpu_create`+`configure`. Undercuts GPU stage 2 and §5.1 “GPU never waits” |
| Materials/fields/scroll cheap | Yes → pool for eligibility, or destroy the effect view |
| Memory 17 MB map / 15 MB WebContent | Order-of-magnitude OK on simulator; caps must wait for `BENCH_VM=1` on the iPad |

Measure instead, on the M1 iPad in the real host:
1. Unpooled Extra Heavy fling first (RFC already does this).
2. wgpu `create_surface` vs `configure` vs first present, Time Profiler at 24k/48k (QUEUE.md).
3. Screenshot after `replaceCurrentItem(nil)` and after map `setRegion` (last pixels).
4. `WKWebView` after empty wrapper: `history.length`, `sessionStorage`, `window.name`, process id.
5. Create/reuse **inside a moving `UIScrollView`**, not a window root.
6. Hitch histogram, not median init.

---

## 6. Pool sizing and eviction

LRU instead of refuse-when-full: **right** (17 kinds × 8 would freeze on the first 32). Global cap 32 will bind, not per-shape 8.

Heavy caps: right idea. Arithmetic for GPU is off: 600×220 @2×, 3 drawables ≈ **6 MB/layer, 24 MB for 4**, not “3 × 3.5 MB” / 16 MB (`§6`). Recalculate on device.

Memory warning → `reset()` (`NodePoolIOS.swift:99–102`): right. Background drops of web/map/video/GPU: right. Count parked trees in `state`: right (not a ninth agent op).

Stage 2 must decrement per-kind caps when LRU evicts a tree that still holds a parked heavy leaf.

70 MB is parked-only. A live window can hold another map + web + videos on top. Pressure path matters more than the sum of caps.

---

## 7. Gaps and Charlie’s questions

**Should decide, doesn’t:**
- §5.1 as a **declared** D3-shaped deviation (default in lists); `loading=` as author override, not as justification.
- LLP 1042 “one incarnation, one player” amendment if video pools.
- Browsing-context reset for web (sessionStorage / `window.name`), not only `history.length`.
- Last-pixel policy for `AVPlayerLayer` and `MKMapView`.
- `state` for a §5.1 deferred leaf (`loading` vs nothing).
- Shader row with overlay children: pooled chrome only, or asset-texture so the layer can park.

**Decides what it shouldn’t:** GPU stage 3 ABI in the same breath as stage 2. That is a profile follow-up.

**Q1** — Yes, with named changes: LRU; pool around heavy **NodeViews**; **do not pool `UIVisualEffectView`**; shape walk skips container chrome; rebind restores presenter indexes. Confidence belongs lower than 0.85 until those tests exist.

**Q2** — Stage 2 GPU only after wgpu configure is measured; hide-until-present is mandatory. Stage 3 not in this ruling.

**Q3** — Not yet. Cap 2 + hide-until-`load` + generation bump are necessary, not sufficient.

**Q4** — Yes: no pool without opt-in; defer create mid-fling; gate `prepare_for_reuse` on settle-frame cost. Add hide-until-new-tiles and `first: true` region apply.

**Add Q5:** Is velocity-gated heavy-leaf creation an accepted deviation from the web?

**DEFERRED.md “no”** for keyed keep-alive and unopted module reuse: right, and it does not spend a doing-list slot.

Apparatus: existing tests/apps/out-of-repo benches — fine. ABI exports are not new repo apparatus.

---

### BLOCKER
- Web reuse as specified leaks **sessionStorage / `window.name`** (and possibly last pixels) across rows; §2’s iframe cell is wrong.
- §4.1/§4.2/`shape()` as currently coded **cannot park** material or carousel rows; `rebind` drops `materialNodes`/`scrollers`.

### MATERIAL
- §5.1 is not `loading="lazy"`; declare it; GPU exclusion is unmeasured.
- Video/map last pixels; `VideoView.last`; LLP 1042 player-identity amendment.
- GPU: idle-wait before second `Surface` on the same layer; stage 3 cache per artifact + device loss.
- Field: `interactionState`, `UITextView` offset, secure-text order.
- Map: region animation on reuse; overlay children vs shader pooling.
- §3 does not justify GPU stage 2; drawable cap math is wrong.
- Two shape walks must skip the same slots or `zip` rebinds the wrong views.

### MINOR
- “60 lines”; per-shape array is not global LRU; RTL scroll origin; markup textarea; `history.length` as a guest-only footnote once the rest is reset.

**Verdict: build with named changes** — stage 1 (pool around heavy leaves + LRU + destroy materials, not reuse them) after the shape/`rebind` fixes; do not ship web/video/GPU/map reuse or §5.1 until the leaks and the deviation are written down.
