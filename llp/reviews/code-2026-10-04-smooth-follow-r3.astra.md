# Code review r3 (final): a smooth list correction is one retargetable motion on iOS, 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the r2 artifacts commit.
- **Method:** one brief (sha256 `e9d7ea6cf51ee55f71d33fe2ce5fe5b227b38ec4dd913ce724a3bbf3f7ba04b5`), shared with grok. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** Landed with no further round. 1 taken: a change in the reachable range rebases from the clamped current offset, and every written sample is clamped; the shrink test starts past the new edge and asserts no frame passes it. 2 DEFERRED: a timestamp-stepped driver for deterministic interior samples, and ownership regressions (reset, two lists).

---

Static review only; no files changed or tests run. Two should-fix findings remain.

1. **Should-fix — Shrinking content can still make the driver overwrite UIKit’s clamp.** [CollectionIOS.swift:377](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:377)

   The driver rebases only when `reachable(to)` changes. Its stored `from` and interpolated sample are unconstrained.

   For example, start a reverse correction from 1700 toward 300, then shrink the maximum offset to 1300 near the beginning. The destination remains reachable, so the rebase branch does nothing. Even if UIKit moves the actual offset to 1300, the next tick interpolates from the old 1700 and writes beyond the new edge. The new shrink test covers a destination becoming unreachable, not this case.

   **Fix:** Detect changes to the reachable range, rebase from the actual offset constrained to that range, and constrain every interpolated sample before assignment. Keep `animationTargets` synchronized with the reachable destination. Add a descending-motion regression that checks every sample, not just eventual landing.

2. **Should-fix — Tests still depend on observing short-lived states and leave ownership regressions untested.** [SmoothCollectionIOSTests.swift:83](/tmp/x17-review/host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:83), [SmoothCollectionIOSTests.swift:93](/tmp/x17-review/host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:93), [SettleScrollAnimationIOSTests.swift:45](/tmp/x17-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:45)

   Predicate waits improve startup and completion checks, but cannot guarantee an interior sample: a delayed callback can land directly at 1700 and fail the first retarget assertion. Conversely, the later shrink and drag predicates also accept completed animations, allowing those cases to pass without exercising an active driver. The settle test retains its fixed 600 ms wait.

   The “stopped animation’s end” test never delivers a stale completion. Reset/recreation, independent pending lists, authored-scroll takeover and running wheel cancellation remain uncovered.

   **Fix:** Extract a timestamp-driven step for deterministic interpolation, retarget, clamp and cancellation tests; retain one bounded display-link integration test. Explicitly assert an active driver before interruption. Add token-replacement and two-list tests, deliver an unrelated UIKit completion during a running correction, and replace the settle test’s fixed wait with a completion predicate.

Round-1 dispositions, covering both reviews:

| Round-1 finding | Assessment |
|---|---|
| Astra 1: wheel/drag destination reads | **Resolved.** Readers now use the moving model offset; cancellation performs no offset adoption. |
| Astra 2: hit-testing/occlusion mismatch | **Resolved.** This correction no longer creates an animated ancestor-bounds/model split. |
| Astra 3: missing per-frame observation | **Resolved.** Actual offsets reach observations and painting; the pump excludes driven ticks. |
| Astra 4: pending-start ownership | **Resolved.** The captured token is checked before touching pending state and survives coalesced updates. |
| Astra 5: unrelated delegate completion | **Resolved.** The delegate excludes pending and driver-owned corrections. |
| Astra 6: inadequate/timing-sensitive tests | **Partially resolved; finding 2 remains.** |
| Grok 1: halt completion precedes adoption | **Resolved.** `cancel()` neither writes an offset nor invokes completion. |
| Grok 2: global pending flag/cancellation | **Resolved.** Cancellation is per-list; reset and retirement invalidate pending ownership. |
| Grok 3: destination-model readers | **Resolved.** Stickies, handoff and viewport observations read each actual sample. |
| Grok 4: drag baseline at destination | **Resolved.** The model stays at the displayed offset. |
| Grok 5: tests never interrupt motion | **Partially resolved; finding 2 remains.** |

The callback path is otherwise coherent. [NodeViewIOS.swift:627](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:627) forwards each sample to viewport observations and stickies. [Collection.swift:468](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:468) treats it as an animation tick, preserving the cursor sequence and avoiding synchronous rescue. Collection geometry deliberately reports the destination. [ScrollPumpIOS.swift:125](/tmp/x17-review/host/apple/Sources/ExactKit/IOS/ScrollPumpIOS.swift:125) skips travel sampling, and stopping clears residual travel. Authored scroll writes now cancel the correction before taking over.

Lifetime cleanup is sound on the normal paths: completion, window loss and explicit cancellation invalidate the display link; reset and [retirement at Collection.swift:373](/tmp/x17-review/host/apple/Sources/ExactKit/Collection.swift:373) call `stopAnimation`. The closures retain the host weakly. Backgrounding does not itself invalidate the link; when callbacks resume, elapsed-time evaluation catches up and completes an expired motion. I found no permanent retain-cycle leak in these paths.

Retargeting from `contentOffset` preserves position and intentionally restarts velocity at zero, which fits the stated contract. `targetTimestamp` is appropriate for the frame being prepared, consistent with [Apple’s timing guidance](https://developer.apple.com/documentation/quartzcore/optimizing-iphone-and-ipad-apps-to-support-promotion-displays). On `x >= 1`, the terminal sample is assigned while `animating` remains set, then completion cancels and schedules the final report. Initial and shifted destinations are clamped after fitting; finding 1 concerns the remaining intermediate-sample gap.

Verdict: LAND WITH FIXES
