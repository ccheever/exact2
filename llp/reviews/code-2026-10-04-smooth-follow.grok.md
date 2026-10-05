# Code review: a smooth list correction is one retargetable motion on iOS (5220497f8), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `5220497f8`.
- **Method:** one brief (sha256 `293ec6973a6211cd6c6d2c244712e2bc1747443aac195f7ed42a4a808df41d48`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):** 1 resolved by the redesign (no Core Animation completion; stopAnimation cancels the driver with the offset where it is). 2 taken (per-list token, cancellation, a vanished view stops). 3 and 4 resolved: the model offset is what shows throughout, so handsOff, stickies and the drag baseline are right. 5 in part: a moving correction is interrupted and retargeted in a test.

---

**1. Blocker — A mid-flight halt lands the animation before the port is back where it shows.** `CollectionIOS.swift:184-186`, `CollectionIOS.swift:201-202`

`startOffsetAnimation`'s completion ignores `finished` and calls `animationEnded(id, atTarget: true)` whenever the serial still matches. `haltOffsetAnimation` removes the bounds animations and only then assigns the presentation offset. Removing a `UIView.animate` animation runs that completion on the same stack with `finished == false`, so `stopAnimation` (`Collection.swift:294`) clears `animating` before `contentOffset = shown`. The assignment fires `scrollViewDidScroll`, and `changed(user:)` (`Collection.swift:454-455`) treats it as the reader's travel and advances the cursor. The rows were built for the destination, so `covers` of the presented port is false on a long `scrollIntoView` and `changed` flushes. `onFeedback` applies that batch inline (`Session.swift:675-682`) from inside `scrollViewWillBeginDragging` (`NodeViewIOS.swift:619`) or the wheel (`AgentIOS.swift:670`), and `userIntent` (`NodeViewIOS.swift:621`) then advances the cursor a second time. The ordinary-correction caller is inside `correcting`, so it does not take this path. The settle test interrupts before the layer animation exists, so it does not either.

Fix: take the completion only when `finished` is true and the serial matches. Keep `animating` set across the presentation write so `changed` stays on the animation branch (`Collection.swift:454`) and neither advances nor flushes. Let the existing caller (`animationEnded(dragging:)` or `stopAnimation`) end it afterward. If `finished` is false, the serial still matches, and the list's window is gone, call `stopAnimation` there so a removed view cannot leave `nativeInFlight` true.

**2. Should-fix — The pending start is a process-wide flag, and cancelling it does not cancel the block.** `CollectionIOS.swift:169-173`, `CollectionIOS.swift:195`, `Collection.swift:266-269`

`haltOffsetAnimation` does `startOwed.removeAll()`, so a drag, a wheel, or an ordinary correction on one list clears every list's coalesce bit. The async block does not read `startOwed`; it starts whenever `animating` and `animationTargets` are set. `stopAnimation`, `reset` (`Collection.swift:340`), and retirement (`Collection.swift:359`) never remove the id. A stop and a new smooth target before that block runs queue a second block; both call `startOffsetAnimation` and the ease restarts. If the block runs after `syncScroll` has dropped the scroll view (`NodeViewIOS.swift:1228-1234`) while the id is still in `animating`, the `guard let scroll` returns and the list stays in `animating`, so `nativeInFlight` stays true.

Fix: key the block with the serial from the `beginAnimation` that scheduled it, and start only when that serial is still current and `startOwed` still contains the id. `haltOffsetAnimation` should remove that id only. `stopAnimation` and `reset` should remove it too. If the scroll view is gone and the id is still animating, `stopAnimation`.

**3. Should-fix — For the whole flight the model offset is the destination, and several readers still treat it as what is on screen.** `CollectionIOS.swift:183`, `ScrollViewIOS.swift:70-76`, `Sticky.swift:118`

`UIView.animate` writes the model `contentOffset` to the target when the animation begins. `geometry` is safe: while `animating` it reports `animationTargets` (`CollectionIOS.swift:52`), which is what §6.8 asks the runner to plan from. The runner therefore builds the headed window, including during the one-turn gap before the layer animation exists. `covers` and `rowsToCover` follow `scroll.bounds`, so once the animation has begun they see that same destination. `captureScrollPosition` and `restoreScrollPosition` do not run for these lists (`PresenterIOS.swift:716`, `PresenterIOS.swift:927`), so `followedTop` is not in this path.

`handsOff` is. A follow-end list's model offset is the end for the whole 0.3 s, so an outward drag is treated as already at the edge and given to an enclosing scroller. The pan never begins, `willBeginDragging` never halts the motion, and the correction keeps running. `StickyHost` reads `contentOffset` from the single `scrollViewDidScroll` that the model write produces, so a sticky box jumps to the destination and stays there while the presentation is still moving. The pump does sample that write (`ScrollPumpIOS.swift:91`), but `motion` is nil for an animating list (`PresenterIOS.swift:176`) and the sample is older than 0.15 s when the motion ends, so the fill does not lead on it. `nativeInFlight` stays true from `beginAnimation` through the completion, and the bounds animation is also visible to `nativeGeometryInFlight`, so `clock settle` does not read the split. Hit-testing follows the presentation.

Fix: decide `handsOff`, stickies, and the scroll event from `layer.presentation()?.bounds.origin` while `offsetMoving` is true (and during the `startOwed` gap, from the pre-animation offset, which is still the model). Leave `geometry` on the target.

**4. Should-fix — The drag baseline is the model target by the time `willBeginDragging` runs.** `NodeViewIOS.swift:616-620`

The pan has already begun, and UIKit's drag offset is that beginning `contentOffset` plus the translation. The beginning value is the animation's model target. Assigning the presentation origin in the delegate does not replace that baseline, so the next move continues from the target and the port jumps. `haltOffsetAnimation`'s key filter is the right one for this animation: `UIView.animate` of `contentOffset` adds a `bounds` or `bounds.origin` animation, and the check is on `keyPath`, so the dictionary key and an additive flag do not matter.

Fix: sync the model to the presentation in `ScrollView.gestureRecognizerShouldBegin` (`ScrollViewIOS.swift:37`), before returning, and `panGestureRecognizer.setTranslation(.zero, in: self)`. Do that only when this list is in `animating`. `handsOff` then sees the on-screen offset too.

**5. Should-fix — The tests wait on Core Animation's clock and never interrupt a moving correction.** `SmoothCollectionIOSTests.swift:30-32`, `SmoothCollectionIOSTests.swift:53-69`, `SettleScrollAnimationIOSTests.swift:44-53`

`RunLoop.main.run(until:)` is in the default mode and returns early if another source stops the loop. The 0.03 s "it is moving" check and the 0.6 s "it ended" check fail when the display link does not tick, and they pass immediately when animations are disabled. The second target in `SmoothCollectionIOSTests` is applied before the first motion has a layer animation, so it locks same-turn coalescing only. The wheel in the settle test runs before the async start, which is the path with no bounds animation. A stale `scrollViewDidEndScrollingAnimation` is no longer delivered. Nothing asserts the presentation offset, a shift retarget, a second list's `startOwed`, or a halt once `offsetMoving` is true.

Fix: drive the completion from a test hook on the animation's serial (the production completion stays), and add one case that starts the motion, applies a new target and a `shift` while `offsetMoving` is true, and one that calls `haltOffsetAnimation` then and expects a single cursor advance to the presentation offset.

`scrollViewDidEndScrollingAnimation` is not on this path. `UIView.animate` of `contentOffset` does not send it. The delegate remains the end of `setContentOffset(_:animated:)` for `restoreScrollPosition`, `followEndIfOwed`, and `applyPendingScroll`, and it still clears `followingEndAnimated`. Collection lists never set that flag, because restore is skipped for them. §6.7 and §6.8 stay separate, and the flag is still coherent.

A retarget uses `.beginFromCurrentState`, so the new ease-in-out starts at the presentation position. Velocity goes back to zero at each mid-flight retarget. That kink is acceptable. The same-turn coalesce, which is the measured send, never starts the first motion, so the estimated row and the measured row do not pay it.

macOS does not take this path. `startOwed` is unused there. `shift` and `correct` in `CollectionMac.swift` still hold `owedTargets` and land them from `animationEnded`. Nothing shared changed behavior.

Verdict: DO NOT LAND
