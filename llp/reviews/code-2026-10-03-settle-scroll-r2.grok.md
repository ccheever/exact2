# Code review, round 2: clock settle and smooth corrections (f3e676311), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f3e676311`.
- **Method:** one brief (sha256 `5cd1dfbba79924a9beb2f5c8439e7916802e0ef211c9099a0f30c035e58bd9ea`), shared with astra. Round 2, blind to the other review. The author is not a reviewer.
- **Transcription:** the run's output after its progress lines, unedited.
- **Verdict:** LAND.

---

## Round 1

1. **Stale `animating` after a plain offset write — resolved for the agent wheel, and the rest is acceptably recorded.** `Agent.scroll` now calls `animationEnded(id, dragging: true)` before `setContentOffset(_:animated: false)` at `AgentIOS.swift:620`. That is the drag path: `dragging: true` skips the stale-end guard and drops `owedTargets` (`Collection.swift:268`). Reveal, reorder, the keyboard toolbar, and the bar collapse are still plain writes. Both dispositions and the commit message record them as older than this change.

2. **macOS never waited — resolved.** `AgentMac.nativeInFlight` is `!presenter.collections.animating.isEmpty` (`AgentMac.swift:67`). The settle loop already waits on macOS when that is true: the early break at `Agent.swift:455` is only for an idle host, and AppKit's completion handler clears the set. A reader wheel already ends the correction in `collectionWillScroll` (`NodeViewMac.swift:1011`). The other caller, `layout agree` (`AgentNativeMac.swift:55`), now reports `incomplete: ["in-flight"]` and skips frame comparison for the same interval, which is what LLP 1080.001 says `nativeInFlight` means, and it matches iOS. Frozen timing still never calls `beginAnimation`, so the clause stays false there.

3. **The test only toggled a fake id — resolved in substance.** `SettleScrollAnimationIOSTests.swift:19` mounts a windowed list, applies a real smooth correction, lands it through `scrollViewDidEndScrollingAnimation`, and interrupts the next one with `Agent.scroll`. Not calling `clock settle` is the recorded partial: a unit-test window may not drive the display link, and the settle loop itself did not change. The predicate is what that loop reads.

4. **LLP cites — resolved.** The comment at `AgentIOS.swift:97` points at LLP 1070.000 §11 and LLP 1035.003 D5. D5 and the LLP 1012 `clock` row both say platform-timing settle waits for a list's smooth correction on iOS and macOS.

## The three new spots

The list lookup is the same one this function already uses for overscroll, and it is how a scroll view is parented: `syncScroll` does `addSubview(sv)` on the node (`NodeViewIOS.swift:1160`). `NodePool` finds a list the same way. An inner scroller that takes the tick is not the animating list, so it correctly leaves the outer correction running; the walk only clears the scroller it actually writes.

`dragging: true` matters once a successor is owed. Without it, `animationEnded` would keep the flag and `landAnimation` would restart toward `owedTargets` after the wheel. The call passes `dragging: true`, so the owed target is dropped, as a drag does. The cursor still advances: the flag is cleared first, then `setContentOffset` runs `scrollViewDidScroll`, and `changed(user:)` advances because the list is no longer in `animating`.

The test locks that interrupt. It would fail if the superview lookup missed and UIKit delivered the end inside the plain write before `animationMoved` was set, which is the §11 case this fix is for. It does not hold an owed successor, and there is no macOS twin; `SmoothCollectionMacTests` already drives the animator, and this commit only publishes that set through `nativeInFlight`.

No findings.

Verdict: LAND
