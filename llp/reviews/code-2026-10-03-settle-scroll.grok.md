# Code review: clock settle waits for a list's smooth correction (bd1dacae6), 2026-10-03 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `bd1dacae6`.
- **Method:** one brief (sha256 `994c4f252043f5934fd2b80388e0b8f9363b031474ec4dcf7fdf1ff493ff7c49`), shared with astra. Round 1, blind to the other review. The author is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (a cancelled smooth scroll leaves `animating` set): taken for the agent wheel. The other plain writers are an older issue, recorded (see astra's disposition).
  - 2 (macOS): taken.
  - 3 (the test): taken, as for astra 3.
  - 4 (LLP cites): taken.

---

1. **Should-fix** — A cancelled smooth scroll leaves `animating` set, so every later `clock settle` spends the two-second bound. `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:100`

`nativeInFlight` is true for as long as `presenter.collections.animating` is non-empty. That set is cleared from `scrollViewDidEndScrollingAnimation` and from `scrollViewWillBeginDragging` (`NodeViewIOS.swift:554` and `:574`). A finger drag is covered: `animationEnded(dragging: true)` calls `stopAnimation` even when an owed target is waiting (`Collection.swift:268`).

A programmatic cancel is not. The agent wheel writes the offset with `animated: false` and never touches the set:

```616:617:host/apple/Sources/ExactKit/IOS/AgentIOS.swift
                    let target = CGPoint(x: takeX ? min(max(o.x + dx, minX), maxX) : o.x, y: takeY ? min(max(o.y + dy, minY), maxY) : o.y)
                    sv.setContentOffset(target, animated: false)
```

UIKit's `scrollViewDidEndScrollingAnimation` runs only when an animated scroll finishes. `animated: false` and a direct `contentOffset` write do not send it. The same write sites are `ReorderIOS.swift:70`, `PresenterIOS.swift:277`, `KeyboardToolbarIOS.swift:65`, and `NavigationBarIOS.swift:510`. `scrollViewDidScroll` only latches `animationMoved` and returns (`Collection.swift:439`).

If the cancel arrives before that latch, a late `didEnd` is dropped on purpose: `atTarget == false` and `!animationMoved` means "a stale end, leave it running" (`Collection.swift:270`). The flag then stays set. The settle loop in `Agent.swift:451` keeps seeing native work, and at two seconds returns `settled: false` with `reason: "transition"`. The next smooth correction does not recover: `correct` sees `animating` and only stores `owedTargets` (`CollectionIOS.swift:167`), and `landAnimation` runs solely from `animationEnded`.

Completion, a collections snapshot that drops the id (`Collection.swift:349`), `reset()` (`Collection.swift:331`), and a user drag do clear it. A reused id that is still in the live snapshot is left alone, which is right only while that same scroll animation is still running.

Fix: before every non-animated offset write, call `animationEnded(id, dragging: true)` (the drag path already ignores the stale-end guard). Do that in `Agent.scroll` and the other writers above. Add a test that starts a smooth correction, applies `setContentOffset(_:animated: false)` before any scroll tick, and expects `nativeInFlight()` to be false.

2. **Should-fix** — macOS platform timing has the same animation and does not wait. `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:100`

`CollectionMac.correct` starts the clip-view animator whenever `smooth && !ExactEnv.agentFreezes` (`CollectionMac.swift:363`), the same gate as iOS. `ExactEnv.agentFreezes` is false under `EXACT_AGENT_TIMING=platform` on both platforms (`Session.swift:73`). `AgentMac.nativeInFlight` is hardcoded `false` (`AgentMac.swift:65`), with the comment that AppKit animates nothing a seek does not move. This scroll is AppKit's animator, not a seek. `clock settle` therefore returns while the transcript is still moving, which is the iOS bug on macOS.

The shared loop already waits when `nativeInFlight` is true on macOS: the early break is only for an idle host (`Agent.swift:454`). AppKit's completion handler clears `animating`, and a reader scroll calls `animationEnded(dragging: true)` (`NodeViewMac.swift:1012`).

Fix: make `AgentMac.nativeInFlight` return `!presenter.collections.animating.isEmpty`, and cover it the way `SmoothCollectionMacTests` already drives a smooth correction.

3. **Nit** — The new test locks the predicate, not settle. `host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:16`

`testASmoothCorrectionInFlightIsNativeWork` calls `beginAnimation(4242)` and `stopAnimation(4242)` on an id that is not a list, then reads `nativeInFlight()`. It never calls `clock`, never builds a scroll view, and never runs a smooth correction. It stays green if `correct(smooth:)` stops setting the flag, if `clock` stops consulting `nativeInFlight`, or if a wheel leaves the set stuck. `SmoothCollectionIOSTests` already proves the flag moves for a real correction, and it ends the animation by hand because a unit-test window may not drive UIKit's display link.

Fix: on that existing windowed list, assert `nativeInFlight` after a smooth correction, after `scrollViewDidEndScrollingAnimation`, after `scrollViewWillBeginDragging`, and after `setContentOffset(_:animated: false)`.

4. **Nit** — The LLP cites do not say this. `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:97`

`1070.000` §6.2 is the staging line that a smooth correction is the platform's scroll animation. The built rules (report the destination, hold `owedTargets`, frozen clock lands at once) are §11, which does not mention `clock settle`. `1035.003` D5 is the right two-second bound and the `reason: "transition"` reply, and its text names the navigation phase and the keyboard. `1012`'s `clock` row still says settle waits for the navigation and modal hosts.

Fix: point the comment at §11 for the animation and at D5 for the bound, and add one sentence to D5 and to the `1012` clock row that platform-timing settle also waits while `CollectionHost.animating` is non-empty.

What holds. Default agent timing is unchanged: `agentFreezes` skips `beginAnimation` (`CollectionIOS.swift:166`), so the new clause is false. The web agent applies a smooth correction with `behavior: "instant"` (`collection-glue.js:526`). Linux never reads `smooth` when it places the port. The JS target does the same under the agent clock. A typing indicator cannot spin the wait: the runner clock is owned for the call, so timers do not append during it, a further correction while one animation runs is a single owed target, and a gap of 24 points or less is set at once (`CollectionIOS.swift:134`). The loop returns `settled: false` at two seconds (`Agent.swift:460`). It does not hang.

Verdict: LAND WITH FIXES
