# Code review: the image component, 2026-08-29 (grok)

- **Family:** grok (xAI): `grok -p`, `grok-4.6`, `--reasoning-effort xhigh`, `--disable-web-search`; document-embedded prompt, no repository access
- **Method:** one-shot code review from the brief (sha256 a90fdc1458af93917e9f4b8611b88b088e567264d619e1b97a2fcbf88b6a12ac), mutually blind to the other family; capsule sha256 3b5c5988c697a2f690c72495c591327c9437de6d71bb8179e9155d1e8ce51595.
- **Disposition:** the orchestrator's fold is recorded at the end of this file.

---

## Review (received 2026-08-29T11:01Z, verbatim)

> Provenance note: the CLI offloaded the oversized prompt to a file and the model read it back (its two leading status lines are preserved below); that read is the CLI's own prompt delivery, not repository access — the prompt carried no repository beyond the embedded excerpts.

The prompt was truncated; I’ll read the offloaded full request so the review covers every cited file.The first chunk ends mid-file; I’ll keep reading so the remaining kernel, host, and spec sections are in the review.## Overall assessment

The kernel model is consistent: `Image` is a measured leaf, the arena column is cleared on alloc and free, `set_intrinsic_size` re-derives Taffy style (so the intrinsic ratio is installed unless a row set `aspect_ratio`) and no-ops on an unchanged value. Flex + one dimension, clear, and the declared block-stretch deviation are locked by tests. Web emits `<img src>` + `object-fit`; macOS reimplements load, report, relayout, and paint. The loader completion is not bound to the live view identity, so a destroy/reuse can write another node’s size. Several CSS-parity and host-safety edges are unimplemented or untested.

## Findings

1. **HIGH —** `host/apple/macos/Sources/ExactMac/Presenter.swift` `loadImage` (~88–119) and `Presenter.apply` `destroy`/`reset` (~314–318, ~367–370). The background work captures `[weak self]`, but the inner `DispatchQueue.main.async` promotes `self` and then calls `presenter?.intrinsic(id, size)` without checking that `presenter.views[id] === self`. Destroy only does `removeFromSuperview` + `views.removeValue`; it does not nil `imageSource` or `presenter`. Sequence: load finishes and queues the main block (strong retain of the old `NodeView`) → a later batch destroys that id and creates a new node with the same id → the completion still runs, `imageSource == source` holds on the *old* view, and `exact_intrinsic` writes the old pixel size into the new node (or, if the id is gone, into the batch as `intrinsic: UnknownView`). Resolve: before `intrinsic`, require `presenter.views[id] === self`; on destroy/reset set `imageSource = nil` (so the guard fails) and drop the presenter pointer. A generation token per load is the robust form.

2. **MEDIUM —** `host/web/src/host.rs` `tag_for`/`props_for` (~1055–1124) vs `kernel/src/layout.rs` Image measure (returns `MeasureOutput::ZERO` until a host reports) and `host/web/index.html` (~14, `img { display: block; }`). The web host never calls `set_intrinsic_size`. Kernel frames for the Caltrain logo are therefore `96×0` for the life of the page (`host/apple/tests/host.rs` shows that frame explicitly before a report). macOS matches CSS after load (`96×36`); web matches CSS only if glue sizes `<img>` from the browser and does **not** apply kernel `h` as CSS height, and only if sibling positions are also not kernel frames. If glue applies `frame` ops the way macOS applies them to `NSView`, the logo is `96×0` and the header column is short of a browser. FLAG: `glue.js` is not in this review set.

3. **MEDIUM —** `Presenter.swift` `resolveSource` / `loadImage` (~80–86, ~88–96). Any `scheme != nil` URL (`http(s):`, `file:`) is loaded with blocking `Data(contentsOf:)`. Relative sources are `appendingPathComponent` onto `EXACT_ASSETS` (else cwd) with no containment check, so `../` and absolute `file://` escape the asset root. There is no timeout, size cap, or cancellation; a hung URL occupies a `userInitiated` thread. Resolve: allow only relative paths, `standardizedFileURL` and require `path.hasPrefix(assetsRoot)`; refuse non-file schemes (or use `URLSession` with a timeout). Contract-authored sources do not make arbitrary filesystem/network reads free.

4. **MEDIUM —** `Presenter.swift` `loadImage` (~93–96) and `draw` (~237–260). `NSImage(data:)` runs on a global queue (AppKit image objects are not a background-thread API). The bitmap is not decoded or downsampled there; first `img.draw` on the main thread can decode a full-resolution image into a 96×36 box, with no cap. Resolve: decode with `CGImageSource` / `NSBitmapImageRep` on the background queue (or `preparingForDisplay`), wrap an `NSImage` on the main queue, and optionally downsample to the view’s backing size.

5. **MEDIUM —** `host/web/src/host.rs` `props_for` (~1097–1106) and `host/web/tests/host.rs` `an_image_is_an_img_with_its_source_and_object_fit`. `AccessibilityLabel` becomes `aria-label`, not `alt`. HTML’s replaced-element accessible name and the visual broken-image fallback are `alt`. A failed load on web therefore does not show “A Caltrain train”; macOS has no equivalent fallback either (stderr + `intrinsic(nil)` → 0×0). Resolve: map the label to `alt` (and keep `aria-label` only if you still need it). FLAG: LLP 1007 is not in this set, so I cannot confirm a declared divergence.

6. **MEDIUM —** `Presenter.swift` `draw` (~237–260) vs CSS object-fit. Fitting uses `bounds` (border box), clips to a sharp `NSBezierPath(rect: bounds)`, and paints the image *after* the border fills. CSS object-fit uses the content box and `border-radius` clips the replaced content. Padding, borders, and radius on an `image` therefore diverge from the web host’s `<img>`. Resolve: inset by padding/border, clip to the same rounded path used for the background. FLAG: I cannot see how padding is represented in the apple style dict.

7. **MEDIUM —** `host/apple/src/abi.rs` ~166–167 (`(width > 0.0 && height > 0.0).then_some(...)`) and `exact.h` ~783–785. `≤ 0` and `NaN` clear (good). `+Inf` is `> 0` and is stored; `taffy_style` then sets `aspect_ratio = Some(w/h)` and the measure closure returns Inf (`kernel/src/style.rs` Image aspect-ratio rule; `kernel/src/layout.rs` Image branch). Resolve: require `is_finite()` as well, otherwise clear. Same ABI path: `pixelsWide`/`pixelsHigh` of `0`/`-1` (some `NSImageRep`s) still take the success branch in `loadImage` (~109–112) and report `(0,0)`/`(-1,-1)`, which clears layout while `self.image` stays set and `draw` falls back to `img.size`. Resolve: treat non-positive pixel extents as a failed load (or fall back to `img.size` *before* reporting).

8. **MEDIUM —** `kernel/src/layout.rs` Image measure (~864–878) and `kernel/src/style.rs` (~919–927). Measure is `known.unwrap_or(intrinsic)` and does not look at `space` or min/max. That is correct only if Taffy has already transferred the ratio onto `known` and applied min/max in the CSS replaced-element order. `kernel/tests/image.rs` covers neither dimension, width-only, an explicit `aspect_ratio` row, clear, unknown view, and the declared block-stretch deviation (`in_a_block_parent_an_auto_width_image_fills_it_a_declared_deviation`, ~105–133). It does not cover height-only, both dimensions, `min-*`/`max-*`, flex stretch vs `flex-start`, or shrink below intrinsic (`min-width: auto`). A Taffy order change would show up first on those paths. Resolve: add those kernel cases (especially height-only and max-width vs intrinsic). FLAG: `txn.rs` is not in this set — I cannot confirm `new_leaf(..., is_measured_leaf)` calls `taffy_style` after a pre-attach `set_intrinsic_size`, or that `SetStyle` re-derives the intrinsic ratio.

9. **MEDIUM —** Tests vs what they claim. `kernel/tests/image.rs` `an_image_is_nothing_until_it_loads_then_its_intrinsic_size` comments “a broken `<img>` is 0×0” but only exercises `set_intrinsic_size(..., None)`, not a failed host load. Apple host tests lock the Caltrain 96×36 path and the unchanged-size fast path; they do not change `imageSource`, clear it, fail a load, or destroy during load. `host/apple/smoke.mjs` ~25–26 only greps `caltrain.png 320x120` and `96x36`. `Presenter.swift` `draw` `cover`/`none`/`scale-down` (~254–264) has no assertion (Caltrain is `contain`). `NodeView.imagesLoaded` (~76) is never cleared on `reset`, and `main.swift` ~218–220 uses `views.values.filter { $0.kind == "image" }.first` (dictionary order) for “first image frame”. Resolve: kernel tests for height-only and min/max; a presenter test that stale completions do not call `intrinsic` after destroy; smoke or host coverage for a failed/cleared/changed source. FLAG: whether smoke waits for load or just happens to print after idle is not in the snippet.

10. **LOW —** `Presenter.swift` `draw` (~254–264): an unknown `object_fit` value takes the `default` branch (`scale-down`), not CSS’s initial `fill`. Only `"fill"` is special-cased before the switch. Emit lowercase from the host and default the switch to fill (put `scale-down` in its own arm).

11. **LOW —** `kernel/src/kernel.rs` `set_intrinsic_size`: a live non-`Image` view is accepted and stored; only the Image measure/style paths read it. After finding 1, a reused id that is no longer an image still gets a silent write. Refuse non-image (and non-finite) sizes at the kernel.

12. **LOW —** `kernel/src/node.rs` `rules_cover_every_type` (~159–174) never calls `is_measured_leaf()`, and never asserts `NodeType::Image` is a measured leaf that cannot hold children. Cheap to add.

13. **LOW —** FLAG on items this set cannot close: rehydration/layout-equality (the intrinsic column is live arena state; a rehydrated kernel is 0×0 until the host reports again; apple only reports from `loadImage`, not from an already-held `NSImage`); `host/web/src/css.rs` default when `object_fit` is unset; LLP 1001 §1 / 1007 / 1008 §5 / `rules/DEFERRED.md` text. The current shape also has no `srcset`, cache, `URLSession` cancellation, load/error events to the app, `object-position`, or `tint_color` — fine if DEFERRED says so, but `object-position` other than 50% 50% cannot be added without changing the draw math in finding 6.

## Verdict
NOT READY

---

## Disposition (orchestrator, 2026-08-29)

1. HIGH (stale completion after destroy/reuse) — **CONFIRMED, FOLDED.** `loadGeneration` + `presenter.views[id] === self` guard; `forget()` on `destroy` and `reset` (generation bumped, source/picture/presenter dropped).
2. MEDIUM (web frames 96×0) — **REFUTED.** The web host never applies kernel frames: `host/web/glue.js:5` "layout and motion are the browser's"; the kernel does not lay out on the web (LLP 1007). The `96×0` in the Apple test is the Apple host's pre-report frame.
3. MEDIUM (source policy) — **FOLDED.** `http(s)` only; relative paths contained under the asset root after standardization; `file:` and other schemes refused. Timeout/size cap declared not in v1 (LLP 1011 §6).
4. MEDIUM (decode on main) — **FOLDED.** `NodeView.decode` decodes on the background queue (`CGImageSource`, cache immediately); `draw` wraps an already-decoded `CGImage`. Downsampling to the backing size: not done, not needed for v1 (declared with `srcset` in §6).
5. MEDIUM (`alt`) — **FOLDED.** Images get `alt`, never `aria-label`. LLP 1007 did not declare a divergence; LLP 1011 §3 now states the mapping.
6. MEDIUM (content box / radius) — **FOLDED.** Content box = frame inset by `border_width_*` + `padding_*` from the style dictionary; clipped to it and to the rounded border path.
7. MEDIUM (`+Inf`, zero pixel extents) — **FOLDED.** Kernel refuses non-finite/non-positive; ABI passes non-finite through to be refused; `decode` treats `width`/`height` ≤ 0 as a failed load.
8. MEDIUM (constraint coverage) — **FOLDED**, and the coverage found a real defect: see codex finding 1 (Taffy patch 5). Height-only, both, min/max, flex stretch now pinned. FLAG on `txn.rs`: `new_leaf` derives the style at creation and `set_intrinsic_size` re-derives it, so the order does not matter; `SetStyle` re-derives through the same `taffy_style`.
9. MEDIUM (tests vs claims) — **PARTLY FOLDED.** `imagesLoaded` cleared on `reset`; "first image" is the lowest id; host tests for a cleared source and refusals; the test comment "a broken `<img>` is 0×0" corrected. A presenter test that stale completions do not report needs Swift test apparatus — declared (LLP 1011 §5/§6). The smoke does not wait for the load; it reads the report after the app's idle — and has been passing because the load completes first (recorded in §5 as an observation).
10. LOW (unknown fit) — **FOLDED.** `default` is `fill`; `scale-down` has its own arm.
11. LOW (non-image accepted) — **FOLDED** (`NotAnImage`).
12. LOW (`rules_cover_every_type`) — **FOLDED.**
13. LOW (FLAGs) — answered: rehydration keeps the column (LLP 1011 §1); `object_fit` unset lowers nothing on the web (the browser's initial `fill`); `object-position` declared not in v1.

Verdict NOT READY binds to the reviewed tree; the folds are unreviewed by this family.
