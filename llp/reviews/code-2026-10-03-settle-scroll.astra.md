# Code review: clock settle waits for a list's smooth correction (bd1dacae6), 2026-10-03 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `bd1dacae6`.
- **Method:** one brief (sha256 `994c4f252043f5934fd2b80388e0b8f9363b031474ec4dcf7fdf1ff493ff7c49`), shared with grok. Round 1, blind to the other review. The fix itself came from a Claude fork in this session. The author is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (an interrupted correction leaves `animating` set): taken for the agent's own wheel, which now ends the correction as a drag does, with a test. The app's other plain offset writers (reveal, reorder, the keyboard toolbar, the bar's collapse) can leave the same stale entry. That is older than this change and affects corrections as well as settle, so it is recorded here and not fixed now.
  - 2 (macOS): taken. `AgentMac.nativeInFlight` counts `animating`.
  - 3 (the test proves the predicate only): taken in part. The test mounts a real list, runs a smooth correction, ends it and interrupts the next one with the wheel. A display link may not run in a unit test's window, so the landing is driven by hand, as `SmoothCollectionIOSTests` does.
  - LLP: the comment cites §11 and D5, and 1035.003 D5 and the 1012 clock row now say so.

---

Static review only; no files changed or tests run.

1. **Should-fix — interrupted scrolling can leave settle waiting on stale state.** The new check at [AgentIOS.swift:100](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:100) trusts the tracker unconditionally. However, an agent wheel replaces the animation at [AgentIOS.swift:617](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:617) without clearing it. If this interrupts before the first animation tick, the cancellation callback can be rejected by [Collection.swift:270](/tmp/x9-review/host/apple/Sources/ExactKit/Collection.swift:270): it is away from the target and `animationMoved` is empty. No running animation remains to clear the entry, so subsequent settles reach the two-second bound. Authored offset writes also lack explicit tracker cancellation.

   **Fix:** clear the old animation and owed target before cancelling offset writes; register any animated replacement separately. Add a regression that interrupts before the first tick. Do not rely on a nonanimated write producing a completion callback; Apple documents that callback for [animated requests](https://developer.apple.com/documentation/uikit/uiscrollviewdelegate/scrollviewdidendscrollinganimation(_:)).

2. **Should-fix — macOS retains the same premature-settle bug.** [AgentMac.swift:65](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/AgentMac.swift:65) always returns `false`, although [CollectionMac.swift:317](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/CollectionMac.swift:317) starts asynchronous AppKit scroll animations and tracks their completion. Platform-timing settle therefore skips waiting on macOS.

   **Fix:** include `!presenter.collections.animating.isEmpty` in macOS `nativeInFlight()`, update its outdated comment, and cover the macOS settle path.

3. **Should-fix — the XCTest proves the predicate, not settlement.** [SettleScrollAnimationIOSTests.swift:16](/tmp/x9-review/host/apple/tests/ExactKitTests/SettleScrollAnimationIOSTests.swift:16) manually inserts a nonexistent list and manually removes it. It never starts UIKit scrolling, calls `clock settle`, or verifies the final offset. It would pass even if every real animation timed out.

   **Fix:** mount a scrollable list under platform timing, start a smooth correction, call `agent.clock(["settle": true])`, and assert `settled: true`, an empty tracker, and the actual destination offset. Include interruption and timeout cases.

The ordinary lifecycle paths look sound: completion clears tracking or retains it through an owed continuation; ordinary corrections and real drags clear it explicitly. Full collection snapshots remove retired IDs before destruction/reuse, and reset clears everything ([Collection.swift:268](/tmp/x9-review/host/apple/Sources/ExactKit/Collection.swift:268), [Collection.swift:349](/tmp/x9-review/host/apple/Sources/ExactKit/Collection.swift:349)).

The added wait cannot loop indefinitely: [Agent.swift:451](/tmp/x9-review/host/apple/Sources/ExactKit/Agent.swift:451) bounds it at two seconds and reports `reason: "transition"`. Runner-driven typing timers remain frozen during this native wait; genuinely continuous external updates may legitimately exhaust the bound. Default frozen-clock corrections remain immediate, and both web agents suppress smooth collection animation.

LLP 1035.003 D5 supports the bounded native wait. LLP 1070.000 §6 step 2 is relevant, but **§11** is the more precise reference for the implemented tracking, continuation, and cancellation behavior.

Verdict: LAND WITH FIXES
