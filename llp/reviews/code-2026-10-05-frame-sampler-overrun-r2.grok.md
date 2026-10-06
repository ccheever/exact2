# Code review: the frame sampler sees missed render deadlines, round 2 (d9f97e21a..36c588d1a), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `36c588d1a`, plain output.
- **Method:** one brief (sha256 `893230920610039befa5359651c2b38d6efaa27add5402b84022164a568ae45c`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition (r3):** 1 taken in part: the activities and orders are asserted (after-waiting at 0, before-waiting at `CFIndex.max`), and a real held turn is read through the observers; the two-callback case is unchanged. 2 taken: iOS and macOS.

---

LAND

Round 1 holds in the code. `stop` drops an unsampled overrun and `turnBegan` (`FrameSampler.swift:113-117`); the test's overrun, stop, baseline, and on-time callback leaves `overruns == 2` (`FrameSamplerTests.swift:52-64`). A callback inside an unslept turn is scored against the previous target before it is replaced (`FrameSampler.swift:163-167`), and that path is what makes the 6 ms record. The installing turn is timed from `watchTurns` (`FrameSampler.swift:130`). Nested `beforeWaiting` still ends the turn, which is the commit point. The iOS test sees both observers in the common modes and gone after `stop` (`FrameSamplerIOSTests.swift:16-22`). The journal phrase sits before `seq` and `apply` (`FrameSampler.swift:192-193`). Disposition 3's platform sentence does not hold.

1. **MINOR** — Observer activity and the install-time clock are unpinned. `FrameSamplerIOSTests.swift:17-18` only checks that two observers are in `.commonModes`. Swapping `afterWaiting` and `beforeWaiting`, or deleting `turnBegan = CACurrentMediaTime()` at `FrameSampler.swift:130`, stays green. `FrameSamplerTests.swift:45-51` ends the two-callback turn at `u + p + 8 ms`, before the replaced target `u + 2p`, so a `turnEnded` that ignored the new target also stays green. **Fix:** read `CFRunLoopObserverGetActivities` (after-waiting at order 0, before-waiting at `CFIndex.max`). Drive one install via `activity()` with no later `turnBegan(at:)`, one callback, and a `turnEnded` past that target. Extend the two-callback case so `turnEnded` is past the second target and that next sample is late on its own.

2. **NIT** — The amendment still says the watch is iOS-only. `llp/1079-speed-hygiene-in-the-agents-hands.rfc.md:708-710` ("On iOS the sampler now watches"). `watchTurns` is shared; macOS installs it from `activity()` (`FrameSampler.swift:85-96`). **Fix:** say both Apple platforms.

No batch, trait, recycle, nested apply, or fill path changed, so no visible result differs. `applyUnlessEmpty` still reports a skipped tick's `seq` (`Session.swift:949`); projection sync, carried rows, and deferred GPU are untouched. Production builds never create a sampler (`FrameSampler.swift:22`, `Session.swift:481`). A virtual clock returns `virtual` without `reply` (`Agent.swift:270-271`) and `activity()` returns while `clock != nil` (`FrameSampler.swift:82`). Blocks are `[weak self]` (`FrameSampler.swift:131-135`); `destroy` calls `stop` (`Session.swift:1433`). Cost while a segment's link runs is two `CACurrentMediaTime` reads per main-thread turn. `FrameSampler.swift` is 264 lines; `scripts/agent-inspect.mjs` is 513.
