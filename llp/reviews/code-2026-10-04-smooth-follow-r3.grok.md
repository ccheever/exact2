# Code review r3 (final): a smooth list correction is one retargetable motion on iOS, 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at the r2 artifacts commit.
- **Method:** one brief (sha256 `e9d7ea6cf51ee55f71d33fe2ce5fe5b227b38ec4dd913ce724a3bbf3f7ba04b5`), shared with astra. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** Landed with no further round. 1 taken as suggested: the current offset and each frame are clamped, the rebase starts at targetTimestamp (progress 0), and a driver already at its clamped target completes at once. 2 DEFERRED (the settle test's fixed wait).

---

## Round 1

| Finding | Status |
|---|---|
| Grok 1 — halt completes at the destination | **Resolved.** `cancel()` does not complete. `stopAnimation` does not write `contentOffset`. A drag or wheel calls `animationEnded(dragging: true)` and leaves the offset where it is (`NodeViewIOS.swift:619`, `AgentIOS.swift:669`). |
| Grok 2 / Astra 4 — pending start is global and uncancellable | **Resolved.** Each start has its own `pendingToken`, checked before any pending state is cleared (`CollectionIOS.swift:180`). `stopAnimation` drops the token (`Collection.swift:276`). `reset` and retirement both call `stopAnimation` (`Collection.swift:354`, `Collection.swift:373`). |
| Grok 3 / Astra 1–2 — model offset is the destination, so wheel, drag, hit-testing, and stickies are wrong | **Resolved.** Each frame writes the on-screen offset. Those readers use `contentOffset`. |
| Grok 4 — drag baseline is the model target | **Resolved.** The model is the screen, so the pan starts there. |
| Astra 3 — scroll observation jumps, and the pump records it as travel | **Resolved.** `scrollViewDidScroll` → `changed(user:)` while `animating` only marks `animationMoved` and does not advance the cursor (`Collection.swift:468`). `geometry` still reports the target, which is what §6.8 asks the runner to plan from. `motion` is nil while `animating` (`PresenterIOS.swift:176`). The pump does not sample those frames (`ScrollPumpIOS.swift:125`), and `stopAnimation` drops the travel sample (`Collection.swift:279`, `ScrollPumpIOS.swift:271`). Stickies read the live offset (`Sticky.swift:118`). |
| Astra 5 / Grok 1 — a UIKit end finishes the driven motion | **Resolved.** `scrollViewDidEndScrollingAnimation` returns while a driver or a pending start exists (`NodeViewIOS.swift:600`). |
| Grok 5 / Astra 6 — tests | **Resolved in substance** for the smooth-correction tests: predicate waits, a mid-flight retarget, a drag stop, the cursor sequence held through the flight, and a shrink. A timestamp-stepped driver was explicitly deferred. One test gap is finding 2. |

Retarget is `from = contentOffset` and a new ease (`CollectionIOS.swift:361`). The position does not jump. Velocity starts again at 0, which §6.8 allows. `targetTimestamp` is the time that frame will be shown; on `x >= 1` the landing write happens before `done`, so `scrollViewDidScroll` still sees `animating`. Backgrounding leaves the link paused; Mach time keeps running, and the next foreground frame catches up, completing if the 0.3 s has elapsed. That is the right result. The display link retains the driver until `invalidate`, and every exit calls `cancel()` (drag, wheel, ordinary correction, an authored scroll write, retirement, `reset`). A list removed from the snapshot is stopped in `beginBatch`.

## Findings

**1. Should-fix — A shrink that leaves the offset past the new edge is eased through empty space, while the runner is told the port is already at the edge.** `CollectionIOS.swift:377`, `CollectionIOS.swift:381`

`reachable` clamps the stored target, including in `shift` and on the initial `animateOffset`. The frame only replaces `to` when that target is no longer reachable. `from` stays `contentOffset`, and the written point is the lerp. `contentOffset` assigns `bounds.origin` and does not clamp, so an offset already past the new maximum (content 2000, port 300, offset 1600, content shrinks to 1500, maximum 1200) is driven through the blank region for the restarted 0.3 s. `reclamp` has already stored the edge in `animationTargets`, so `geometry` reports the edge the whole time. Hit-testing and stickies follow the blank offset. A shrink whose current offset is still inside the new range is fine: both ends of the lerp are inside it. `shift` with a zero delta hits this same path, because it updates `contentSize` and returns before `animateOffset` (`CollectionIOS.swift:88`).

The rebase also sets `began = link.timestamp` while progress is `(targetTimestamp - began) / duration`, so the reclamp frame is already one vsync into the new curve instead of exactly `from`.

**Fix:** clamp the current offset through `reachable` as well as the target, and clamp the point written every frame. On that rebase set `began = link.targetTimestamp`, so this frame's progress is 0 and the write is the clamped offset. If the clamped offset is already the clamped target, complete the driver there instead of holding `animating` for another 0.3 s. In the shrink test, start from an offset already past 1300 and assert that every sample until landing is `<= 1300` and that `animationTargets` is 1300 while the driver is still running. `> 1250` often fires while the offset is still inside the new range, and the landing check passes either way.

**2. Nit — The settle test can still fail because the run loop returned early.** `SettleScrollAnimationIOSTests.swift:45`

`RunLoop.main.run(until:)` runs in the default mode and returns if another source stops the loop. The 0.6 s spin is a single call, so an early return leaves `animating` set and the test fails even though the motion is in progress. `SmoothCollectionIOSTests.wait` already retries until a predicate. The wheel on line 52 still runs before the first tick; the comment says so, and the moving-driver interrupt is the other test.

**Fix:** wait until `nativeInFlight()` is false, with the same predicate helper, and fail with the driver and `animating` state on timeout.

Verdict: LAND WITH FIXES
