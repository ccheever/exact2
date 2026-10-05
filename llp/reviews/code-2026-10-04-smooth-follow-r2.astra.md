# Code review r2: a smooth list correction is one retargetable motion on iOS (8fa76110a), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `8fa76110a`.
- **Method:** one brief (sha256 `52c3c8d8ee36f983c8886a2df8b0606f560f37c182f12e2868367bf842ad1d75`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: each queued start has its own token, checked before any pending state is touched. 2 taken: targets clamped to the reachable range in animateOffset (so in shift too) and reclamped each frame, with animationTargets updated. 3 taken: applyPendingScroll stops a smooth correction before an authored write. 4 taken: scrollViewDidEndScrollingAnimation does not end a driver-owned or pending correction. 5 in part: predicate waits, the cursor sequence asserted unchanged through a flight, a shrink mid-flight; DEFERRED: a timestamp-stepped driver test, reset/recreation and two-list cases.

---

Static review only; no files changed or tests run. The redesign fixes the main presentation mismatch, but cancellation ownership and clamping still need fixes.

1. **Should-fix — An obsolete queued start can cancel its replacement.** [CollectionIOS.swift:173](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:173)

   The closure captures no scheduling token. It removes the current `startOwed` entry and reads the current `pendingSerial`, then compares that with the current animation serial. After scheduling A, resetting/recreating the list, and scheduling B under the same ID, A consumes B’s pending state. A’s old scroll view fails the identity/window check, so `stopAnimation(id)` cancels B. B’s closure subsequently does nothing.

   **Fix:** Capture an immutable token for each queued start. Preserve it across coalesced target updates, invalidate it on cancellation, and compare it **before removing any pending state**. A stale closure must neither start nor stop its replacement.

2. **Should-fix — The driver can write outside the current scrollable range.** [CollectionIOS.swift:93](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:93), [CollectionIOS.swift:348](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:348)

   `correct` correctly clamps its initial target after `fit`. However, the animating branch of `shift` passes `headed + delta` without clamping, and the driver never revisits the range. If content shrinks or the viewport grows without a replacement correction, UIKit’s adjustment can be overwritten by the next interpolation toward the obsolete target. Completion then unconditionally claims `atTarget: true`.

   **Fix:** Clamp shifted targets after fitting. When bounds, insets, or content size change, rebase from the actual offset toward the newly clamped destination, update `animationTargets`, and constrain subsequent samples to the current range. Complete at that reachable destination.

3. **Should-fix — Authored scroll-position writes do not cancel the driver.** [PresenterIOS.swift:928](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:928), [NodeViewIOS.swift:984](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:984), [CollectionIOS.swift:352](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:352)

   A `scrollTop`/`scrollLeft` update calls `userIntent` and `applyPendingScroll`, but neither cancels `OffsetDriver`. A nonanimated write can therefore move the viewport immediately, only for the next driver tick to overwrite it. A smooth write introduces a second animation mechanism targeting the same offset. Previously, these writes operated on UIKit’s own scroll animation.

   **Fix:** Cancel the collection’s pending/running correction before applying an authored scroll-position command, or route that command through the same driver. Ensure the resulting collection report describes the replacement position.

4. **Should-fix — UIKit’s delegate still bypasses driver completion ownership.** [NodeViewIOS.swift:596](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:596), [Collection.swift:286](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:286)

   Comparing the real offset restores part of the old discriminator, but does not make it sufficient: `animationEnded` accepts an off-target callback once `animationMoved` contains the list. Thus an unrelated UIKit completion after movement can still cancel the pending/running correction without its serial. UIKit animations remain reachable through `applyPendingScroll`.

   **Fix:** Let the serial-checked driver completion exclusively finish these corrections. Keep ordinary-scroller bookkeeping in `scrollViewDidEndScrollingAnimation`, but remove its authority over driver-owned corrections. Drag/wheel cancellation should remain explicit.

5. **Should-fix — Tests improve coverage but retain timing assumptions and omit ownership regressions.** [SmoothCollectionIOSTests.swift:74](/tmp/x17-review/host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:74), [SmoothCollectionIOSTests.swift:128](/tmp/x17-review/host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:128), [SettleScrollAnimationIOSTests.swift:45](/tmp/x17-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:45)

   The new interior-offset assertion does exercise an in-flight retarget when it passes. However, fixed 30/120/600 ms spins still assume display scheduling. The drag directly invokes the delegate, and the “stopped animation’s end” test no longer delivers a stale completion at all. Reset/recreation, two-list independence, shrinking content, shifts, and authored-offset takeover remain uncovered.

   **Fix:** Expose a timestamp-driven step for deterministic driver tests, retaining a bounded display-link integration test. Add regressions for findings 1–4, and assert that ticks preserve the cursor sequence while takeover advances it appropriately.

Round-1 dispositions:

| Finding | Round-2 assessment |
|---|---|
| Astra 1: wheel/drag use destination | Resolved: model offset now follows the visible motion. |
| Astra 2: hit testing/occlusion mismatch | Resolved for this motion: no animated ancestor-bounds/model split. |
| Astra 3: missing per-frame observation | Resolved for the reported destination jump; callback details below. |
| Astra 4: pending-start ownership | Partially resolved; finding 1 remains. |
| Astra 5: unrelated delegate completion | Not fully resolved; finding 4 remains. |
| Astra 6: inadequate/timing-sensitive tests | Partially resolved; finding 5 remains. |
| Grok 1: halt completion precedes adoption | Resolved: cancellation performs no offset adoption or completion callback. |
| Grok 2: global pending flag/cancellation | Partially resolved: per-list clearing and vanished-view cleanup work; finding 1 remains. |
| Grok 3: destination-model readers | Resolved. |
| Grok 4: drag baseline at destination | Resolved by keeping the model at the displayed offset. |
| Grok 5: tests never interrupt motion | Partially resolved; finding 5 remains. |

The requested callback trace is otherwise sound. Each setter reaches `scrollViewDidScroll`; stickies and viewport observations receive the current offset. [Collection.swift:463](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:463) recognizes animation ticks, schedules reporting, and avoids cursor advancement and synchronous rescue. Collection geometry deliberately reports the destination, while [PresenterIOS.swift:175](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:175) masks runner travel. The pump samples actual movement for text/fill preparation rather than r1’s artificial destination jump; its short-lived velocity tail after stopping also existed with UIKit-driven scrolling.

Normal lifetime cleanup is present: [reset:349](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:349) and [retirement:368](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:368) call `stopAnimation`; completion and a missing window invalidate the link. Invalidation releases the retained target, breaking the cycle. [Apple documentation](https://developer.apple.com/documentation/quartzcore/cadisplaylink/invalidate%28%29?language=objc). Backgrounding does not explicitly cancel it: while callbacks are suspended, the driver remains pending; on resumption it finishes if its media-time duration has elapsed. That deserves lifecycle coverage, but is not itself evidence of a permanent leak.

Retargeting from `contentOffset` is position-continuous; restarting velocity at zero is acceptable under §6.8. `targetTimestamp` is appropriate for preparing the next displayed frame. [Apple documentation](https://developer.apple.com/documentation/quartzcore/cadisplaylink/timestamp?changes=_2__1&language=objc). On `x >= 1`, the final write occurs before completion, so its callback still sees `animating`; the outstanding problem is range validation, not that ordering.

The declared unbuilt-spacer traversal limitation remains. I found no macOS behavioral change.

Verdict: LAND WITH FIXES
