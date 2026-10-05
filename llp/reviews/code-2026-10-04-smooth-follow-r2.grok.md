# Code review r2: a smooth list correction is one retargetable motion on iOS (8fa76110a), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `8fa76110a`.
- **Method:** one brief (sha256 `52c3c8d8ee36f983c8886a2df8b0606f560f37c182f12e2868367bf842ad1d75`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: the pump does not sample while a list is animating, and stopAnimation drops its travel sample. 2 taken (clamp, per-frame reclamp, stored target). 3 taken: stopped short, the list reports again (also when its view went away before it began). 4 in part: predicate waits and the sequence assertion.

---

## Round 1

| # | Finding | Status |
|---|---|---|
| Astra 1, Grok 3–4 | Wheel, drag, hit-testing, stickies, and `handsOff` read the destination | **Resolved.** Each frame writes `contentOffset`, so those readers see the screen. `willBeginDragging` and the wheel call `animationEnded` without moving the offset first (`NodeViewIOS.swift:616`, `AgentIOS.swift:669`). |
| Astra 2 | Hit-testing and occlusion use the model while the screen is elsewhere | **Resolved.** The model is the screen. |
| Astra 3 | Scroll observation jumps to the destination | **Resolved for the runner, stickies, and the scroll event.** `scrollViewDidScroll` → `changed(user:)` while `animating` only marks `animationMoved` and does not advance the cursor (`Collection.swift:463`). `geometry` still reports the target (`CollectionIOS.swift:52`). Stickies read the live offset (`Sticky.swift:118`). **Not resolved for the scroll pump** — finding 1. |
| Astra 4, Grok 2 | Pending starts are a process-wide flag and are not cancelled | **Resolved.** `pendingSerial` is per list; `stopAnimation` drops `startOwed` and the serial (`Collection.swift:271`); the block starts only if that serial is still current and the same scroll view is in a window (`CollectionIOS.swift:173`). |
| Astra 5, Grok 1 | A halt or a stale delegate finishes the motion at the destination | **Resolved.** `cancel()` does not run the completion. `stopAnimation` does not write `contentOffset`. Mid-flight, `contentOffset` is not the target, so `scrollViewDidEndScrollingAnimation`’s `atTarget` check is false. |
| Astra 6, Grok 5 | Tests never interrupt a moving correction and spin on the wall clock | **Partly resolved.** `testAMovingCorrectionRetargetsFromWhereItIsAndADragStopsIt` retargets from an interior offset and stops a moving driver. The wheel in the settle test still runs before the first tick. Wall-clock spins remain — finding 4. |

Retarget is `from = contentOffset` and a new ease (`CollectionIOS.swift:335`). The position does not jump. Velocity starts again at 0, which §6.8 allows. `targetTimestamp` is the time that frame will be shown; `x` is clamped to 1, so the landing frame writes the end point and then completes. `CADisplayLink` retains its target until `invalidate`, and every exit calls `cancel()` (drag, wheel, ordinary correction, retirement in `beginBatch`, `reset` from `destroy`). The completion captures the host weakly. Backgrounding does not invalidate the link; Mach time keeps running, so the next foreground frame catches up and a pause longer than the remainder completes. That is the right result for a 0.3 s motion. Retirement does call `stopAnimation`.

## Findings

**1. Should-fix — The driver’s frames are recorded as the reader’s travel.** `ScrollPumpIOS.swift:123`

`changed` treats those `contentOffset` writes as animation ticks, but `ScrollPump.scrolled` samples them whenever `correcting` is false. The driver runs outside `correcting`. `motion` hides the speed only while `animating` is set (`PresenterIOS.swift:176`). A 1700 pt ease at 60 Hz still has about 1900 pt/s stored at the last frame, and `velocity()` keeps it for 0.15 s after `stopAnimation`. Any collection report in that window — a clamp that makes the landing offset differ from `animationTargets`, a slice still `pending`, a scroll handler’s batch — is sent as reader travel. `lead` then extends the window by `v * 0.25` (hundreds of points). Two such reports cancel a smooth `scrollIntoView` (`into_view.rs:220`) and clear an opening `scroll-start: end` (`start.rs:44`).

**Fix:** In `scrolled`, do not `sample` when `collections.animating` contains that list. When `stopAnimation` runs, drop that list’s travel sample (`ScrollPump.forget`’s `travel` entry, without necessarily clearing fill costs) so the tail cannot outlive `animating`.

**2. Should-fix — A target past the content is still driven, and a shrink mid-flight is not reclamped.** `CollectionIOS.swift:90`, `CollectionIOS.swift:352`

`correct` clamps once, after `fit`. `shift` while a motion runs retargets to `headed + delta` with no clamp, and a `delta` of 0 still `fit`s a smaller `contentSize` and leaves the old target (`CollectionIOS.swift:88`). `OffsetDriver.frame` writes that point every frame. The stored offset either sticks at UIKit’s clamp or moves past the end, while `geometry` keeps reporting `animationTargets` for the rest of the flight — the runner and the screen split again, for this case only.

**Fix:** Clamp the point to the scroll view’s current minimum and maximum in `shift` before `animateOffset`, and again in `frame` before the write. Store the clamped point in `animationTargets` so the offset the runner plans from is one the port can reach. Ease toward the new edge when the content shrinks.

**3. Should-fix — A view that leaves the window stops the driver and does not report.** `CollectionIOS.swift:349`, `CollectionIOS.swift:190`

The guard calls `done(serial, false)`, which calls `stopAnimation` and does not mark the list dirty. The same is true of the pending-start path when the view is already gone (`CollectionIOS.swift:177`). Flight reports already told the runner the offset was the destination. The port is left where it was, and `nativeInFlight` goes false, until some later batch happens to flush.

**Fix:** After `stopAnimation` for `finished == false`, if the entry is still live, `dirty.insert` and `schedule`. `geometry` will then report `contentOffset` (the view is still there; only `window` is nil). `animationEnded(atTarget: false)` is the wrong call for a motion that never ticked: it returns without stopping when `animationMoved` is empty (`Collection.swift:288`).

**4. Nit — The new mid-flight test still passes or fails on the wall clock.** `SmoothCollectionIOSTests.swift:30`, `SmoothCollectionIOSTests.swift:80`

`RunLoop.main.run(until:)` returns if that loop is stopped, and `moving` only checks that a driver object exists. `testAMovingCorrectionRetargetsFromWhereItIsAndADragStopsIt` needs a display-link tick inside 0.12 s (`mid > 0`) and a landing inside 0.6 s. Nothing asserts the scroll sequence stayed put during the flight, a second list’s pending serial, or a shrink under a moving driver. The settle test’s wheel still runs before the driver starts.

**Fix:** Spin in `.common` until a predicate (`contentOffset` past a threshold, then `animating` empty) and fail with the offset and driver state on timeout. Assert the cursor sequence is unchanged across the flight.

Verdict: LAND WITH FIXES
