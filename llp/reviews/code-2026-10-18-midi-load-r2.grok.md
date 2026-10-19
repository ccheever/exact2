# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ace12a15d`.
- **Method:** one brief (sha256 `c8cd1030634f8bebac2ef7aeee6064df63376789377e738e630dbe5ce056e6da`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking.
- **Disposition:** nothing to take.

Nothing blocking. The ceiling matches the hub for a steady main-queue stall, and the production hook is a single nil load. One test hole can still let a second hold pass.

1. **SHOULD-FIX** — `late` is measured from the stamp, so it stacks on the new handoff credit and a second hold can pass. `host/apple/tests/ExactKitTests/MIDIMacTests.swift:374`

   The drain is due at `max(stamp + 0.050, heard)` (`MIDIModule.swift:181`). The assertion allows `max(0.100, handoff + 0.050) + late`, and `late` is `MainLag.most(oldest, d.at)`: the worst beat whose deadline falls anywhere from the stamp to delivery. That span includes the handoff, before this batch is waiting on main. A beat in that prefix can set `late` without the drain handler being that late.

   Scenario: oldest stamp at T, hub hears it at T+0.080 (deadline T+0.080), a beat due at T+0.010 runs 60 ms late, then main is idle. The drain itself is 2 ms late, and a bug holds the batch one more 50 ms period. Delivery is T+0.132. `late` is 0.060, so the ceiling is `max(0.100, 0.130) + 0.060 = 0.190`. `0.132 < 0.190` passes. With `late` taken only while the batch waited, the ceiling is about 0.130 and the assertion fails. Under a steady stall, `late` matches the drain and a full extra 50 ms still fails; the miss is a stall that ends before the deadline.

   Fix: measure `late` from the deadline, `max(oldest + 0.050, oldest + handoff)`, through `d.at`. Keep `+ late` so main's own delay after that deadline still excuses the drain.

2. **NIT** — the hook is installed after the sender is already running. `host/apple/tests/ExactKitTests/MIDIMacTests.swift:345`

   `DispatchQueue.global().async` starts the ticks, then `MainLag()` runs, then `observeHeard` is set. `heard()` can run on the arm's thread before that store. A miss hits `?? 0` at line 376, which puts that batch back on `0.100 + late` and can revive the load-95 false failure. The same store and the `defer` clear at line 359 are unsynchronized with the arm thread (`nonisolated(unsafe)` at `MIDIModule.swift:157`). Production never writes the static, so that race is test-only.

   Fix: set `observeHeard` before the send loop. Clear it after `shutdown()` returns and the arm is idle.

Stamp matching holds. The JSON number is `(message.at - ExactEnv.t0) * 1000` (`MIDIModule.swift:300`), parsed back with `JSONSerialization`, and `ExactEnv.t0 + ms/1000` reproduced `message.at` with error 0 over a sweep of stamps (double string round-trip, including a large `t0`). The hook key is that same `message.at`, not the sender's `CACurrentMediaTime()`. Ticks in this drive are `0.5/24` s apart (~20.8 ms), so the 1 ms window contains one stamp; the nearest-of-those pick cannot land on a neighbor. Two messages with the identical `at` would keep only the later hear time and loosen the ceiling; this test does not send those. A missed key becomes handoff 0, which tightens the bound.

Production cost matches the claim. `observeHeard?(message.at, CACurrentMediaTime())` does not evaluate the arguments when the optional is nil (checked with a side-effecting argument). Each message is one load of the static and a nil branch, including ticks that `heard()` then drops. There is no production writer, so the concurrent reads are not a data race.
