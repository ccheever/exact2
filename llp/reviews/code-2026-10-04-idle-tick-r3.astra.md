# Code review r3 (final): an idle timer tick only moves the clock (3f4e5af5e), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `3f4e5af5e`.
- **Method:** one brief (sha256 `356d9eb4adbe37d1e6a7fdb84c5fa88a14e0da50b9e8f61a8c87455071102ffe`), shared with grok. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (landed, no further round):**
  - Finding 1 is taken, as grok's finding 1 has it. `applyGeometry` outside a batch restores carried rows synchronously before it writes the frame. The queued sync no longer calls `prepare()`, so a trait refresh never reparents a row.
  - Finding 2 is taken. The queued sync mounts the grouped lists first, then syncs segments and controls, so a segment in a carried row is judged where it shows.
  - Finding 3 is taken in part: a carried custom row keeps a replayed box and is carried again, and the successor timer is asserted valid.
  - DEFERRED: separate trait-trigger and coalescing-count tests (no test seam counts syncs), and editing or scrolling preservation (covered by the no-reparenting change, untested).

---

Static review of `3f4e5af5e`; no files modified or tests run. Two correctness fixes remain.

1. **Should-fix — The queued refresh overwrites replayed geometry for carried custom rows.**  
   [PresenterIOS.swift:31](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:31), [GroupedListIOS.swift:506](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:506).

   `applyGeometry` writes the new frame immediately, then schedules this callback. For a custom row already carried into a cell, `prepare()` subsequently restores its **old saved frame**. `mount()` then derives the row frame and cell height from that old geometry. A dismissal replay changing the custom row’s width or height is therefore undone.

   **Fix:** Restore rows before replaying geometry, then remount afterward; alternatively, update their saved authored geometry in place. Use a refresh without reparenting for trait-only changes. Add a regression that replays geometry onto a carried custom row and checks both its size and cell height.

2. **Should-fix — Synchronizing segments before remounting can disable a visible custom row’s control.**  
   [PresenterIOS.swift:32](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:32), [SegmentsIOS.swift:121](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:121), [SegmentsIOS.swift:321](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:321).

   Consider a custom grouped row containing a text-only tablist projected as `UISegmentedControl`. `prepare()` returns it beneath the hidden authored scroll. `segments.sync()` therefore evaluates `available(owner)` as false and disables the control. The final grouped-list mount makes the row visible again without recomputing availability. The tab-bar projection has the equivalent interaction-disable path.

   This ordering also exists in full batches, but round 3 newly invokes it for an otherwise harmless appearance refresh—even one originating elsewhere.

   **Fix:** Refresh segment availability after carried rows reach their final presentation hierarchy, or avoid restoring rows during this refresh. Test that a custom row’s segmented control remains enabled and accepts selection after an out-of-batch refresh.

3. **Nit — The new tests leave important assertions unproven.**  
   [GroupedListIOSTests.swift:214](/tmp/x11-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:214), [IdleTickTests.swift:32](/tmp/x11-review/host/apple/tests/ExactKitTests/IdleTickTests.swift:32).

   The grouped-list test proves eventual outer-frame and switch updates. Its explicit `requestProjectionSync()` means it still passes if `applyGeometry` stops requesting synchronization; final-state assertions also cannot establish coalescing. It exercises neither an actual trait change nor custom rows. The timer test never asserts that the successor timer exists and is valid before clearing it; the final `XCTAssertNotEqual` accepts nil.

   **Fix:** Test geometry and trait triggers independently, assert one refresh for multiple requests, and unwrap/assert the successor timer before testing invalidation. Include custom-row scrolling, animation and editing preservation.

Round-2 disposition:

- **GPU drain: resolved.** Both platforms now drain only after `deferred.isEmpty`, including a wave containing failures.
- **Projection coverage: expanded as requested, but incomplete in correctness** because of findings 1–2.
- **Clock tests: substantially improved**, including replacement invalidation and clearing `clockDue`; the remaining weakness is finding 3.
- **Trait condition: no separate defect established.** Native tabs hit the control trigger. Authored symbol tabs also refresh indirectly: their appearance changes the symbol cache key, queues intrinsic reporting, and causes a full apply.
- For ordinary rows, `sync(changed: [])` preserves the snapshot and does not explicitly rewrite the scroll offset. That does not establish safety for carried rows or active editing.

Verdict: LAND WITH FIXES
