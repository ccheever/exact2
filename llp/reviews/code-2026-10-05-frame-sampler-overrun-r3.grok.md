# Code review: the frame sampler sees missed render deadlines, round 3, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `5ee4ebccb`, plain output.
- **Method:** one brief (sha256 `9d139a2699a05d20bb52663d8b7db2a8cb13b467fa57b93c91b4a28e23255e0f`), shared with astra. Round 3, blind to the other review. The authors are not reviewers.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition:** No findings.

---

LAND

No findings. Round 1 and round 2 dispositions hold in this tree.

1. **Correctness.** Nothing in the diff changes what a batch, a trait change, a recycled view, an out-of-batch callback, a nested apply, or a fill draws. The sampler only records. A turn that starts before `target` and sleeps after it is one overrun, kept as the worst since the last sample (`FrameSampler.swift:146-149`, sampled at `:179-185`). A callback inside that turn is scored against the previous target before the target is replaced (`:164-167`); the unit test's 6 ms record is that path (`FrameSamplerTests.swift:48-64`). The installing turn is timed from `watchTurns` (`:130`). `stop` drops an unsampled overrun and `turnBegan` (`:113-117`), so the next segment's on-time frames stay at `overruns == 2`. Nested `beforeWaiting` ends the turn, which is the commit point; `afterWaiting` opens the next one. Observers are in `.commonModes` at order 0 and `CFIndex.max` (`:131-138`), so `UITrackingRunLoopMode` and AppKit tracking are included. Blocks are `[weak self]`; `Session.destroy` calls `stop` (`Session.swift:1468`). Cost while a segment's link runs is two `CACurrentMediaTime` reads per main-thread turn.

2. **Regressions.** No presentation skip changed. `applyUnlessEmpty` still reports a skipped tick's `seq` at zero cost (`Session.swift:964`). Projection sync, carried rows, and deferred GPU are untouched, so the idle-tick holes stay closed.

3. **Tests and dispositions.** `stop` clearing state, the in-turn callback, observer activities and orders (`FrameSamplerIOSTests.swift:20-22`), a real 60 ms hold read only through the observers (`:31-44`), and `Agent.clock` stopping the watch (`:50-59`, `Agent.swift:471-473`) are all in the diff and match the round-3 notes. Production never creates a sampler (`FrameSampler.swift:22`, `Session.swift:486`). `perf frames` returns `virtual` without `reply` once `session.clock != nil` (`Agent.swift:284`), and `activity()` returns while the clock is set (`FrameSampler.swift:82`). Still untested, as those dispositions said: a nested loop, a tracking-mode drive, and a two-callback turn that ends past the second target. The arithmetic those would pin is what the direct `turnBegan` / `turnEnded` case already checks.

4. **Quality.** The amendment matches the line and both platforms (`llp/1079-speed-hygiene-in-the-agents-hands.rfc.md:708-716`). `agent-inspect.mjs` prints `overruns` only when the host sends them, and the late-line phrase only when `overrun` is non-zero. `FrameSampler.swift` is 264 lines, `Agent.swift` 717, `scripts/agent-inspect.mjs` 517. No dead code.
