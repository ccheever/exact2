# Code review: PageFactsSoonMacTests' load flake (test timing, not ordering), round 1 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ecc3a2fa91`.
- **Method:** one brief (sha256 `ead292e89fa2c5ca6598a91a37892b1d888a1f5883d57f8a73cf10b47e13cc8f`), blind to the other reviewer; 2 rounds (the lead's ask). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix and the opening narration before it.

**BLOCKER:** none

**SHOULD:** `QUEUE.md:10` — the new entry says this test stays green if `Session.apply`'s join is removed. On this call path that is false, and the suggested direct `runtime.press` + `apply` is the same join the tap already uses.

**NIT:** none

The diagnosis matches the code, and the new wait closes the race without hanging or swallowing a failed activation.

The flake is the soon path. `firstDrawn` posts `prepareData`, which sets `dataPreparing` and runs the source on the owner (`SessionActivation.swift:33-45`). While that flag is set, `PageFacts.observe` (`Session.swift:544`, `PageFacts.swift:305-334`, including `didBecomeKey`) calls `tellPageSoon`, and `tellPageSoon` takes `tellSoon` (`SessionNotices.swift:55-68`): three batches, `noticesInFlight += 3`. `publish` only enqueues them and hops to `landFill` (`Session.swift:620-625`); the counter drops in `landFill` (`Session.swift:653`). The press is `apply(runtime.press(...))` (`Session.swift:719`). `runtime.press` waits on the owner, so the test's `later` job has published by the time `apply` runs, and the join at `Session.swift:1068-1072` lands those batches before the press batch. That is why the staged notice hit zero before the press, and why a second soon-notice posted during the click — after that join, while a late first draw still had `dataPreparing` set — leaves `noticesInFlight == 3`. The test's own `dataPreparing = false` does not cover that, because `prepareData` sets the flag again.

That promise is not newly hidden. The wait finishes real activation before `tellPageSoon`, so the click is no longer told soon and cannot inflate the counter. If the join stops landing the staged notice, the `== 0` assert still fails. What this test still does not record is apply order: both asserts can pass if the press batch were applied and the notices landed afterward, before `tap` returns. That was already true, and this wait does not drain the staged notice.

The wait itself is bounded and fails closed. `while A, B` requires both `dataGeneration != generation || activationInFlight` and `Date() < calm`, and each `run(until:)` is 10 ms, so a draw that never comes ends at 10 s and `XCTAssertEqual` at `PageFactsSoonMacTests.swift:46` fails. It does not spin forever. `dataGeneration` is set only at `SessionActivation.swift:57`, in the same main-queue block that clears `dataPreparing` before `activateData`, and `activationInFlight` stays true until `dataPreparing` is clear. Notices posted while `dataPreparing` was set are ahead of that completion on the owner, so the data batch's `apply` joins them before the wait's condition can pass. The staged `tellPageSoon`, the `== 3` check, and the 50 ms check still run after that.

`agentInstance.tap` (`AgentMac.swift:776-803`) does not settle the session. `drainAll` runs only from `Agent.handle` (`Agent.swift:156-159`), which this test does not call. Nothing else decrements `noticesInFlight` before `tap` returns except `landFill`, and the only synchronous `landFill` on this path is the join inside the press's `apply`. Removing that join leaves the counter at 3: `publish`'s main-queue hop has not run. Pressing through `runtime.press` and `apply` directly is the button's `onPress` (`Session.swift:719`), so it does not observe order either. A real order check is the hook the entry already names.

The new comment matches that sequence: the late first draw, the click's notice told soon while `dataPreparing` is set, and the `== 0` failure looking like the staged notice never landed.
