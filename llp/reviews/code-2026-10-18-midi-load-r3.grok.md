# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `bd89cc2f2`.
- **Method:** one brief (sha256 `3d33bbac1b487ea94f4b82bb5cb3ee265ed23e1a4b18c4ed5e1863364f9a4f7c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker (lag added to the ceiling hides a second hold) and two should-fix (the send allowance; the observer's thread safety and the probe's lifetime).
- **Disposition:** taken in `ce32fa972`.

**1. BLOCKER** — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:388` (lag from `:384` and `MainLag.most` at `:96`)

A second hold still passes once any strict beat in the batch window is more than about 5 ms later than the drain itself. The ceiling is `max(0.050, handoff) + 0.045 + late`. Production arms the drain at `max(stamp + 0.050, heard)` (`MIDIModule.swift:181`), so a second 50 ms hold lands at `max(0.050, handoff) + 0.050` plus the delay that drain actually saw. `XCTAssertLessThan` then fails only while `late` is within 5 ms of that delay.

`late` is not that delay. It is the worst 10 ms strict beat whose deadline falls anywhere in `[oldest, d.at]`, scored when the handler eventually runs. On the main queue a stall that ends before the drain deadline leaves an early beat late and the drain on time. With the 9.6 ms lag from the original flake, Astra's case (heard at 80 ms, delivered at 130 ms) compares 130 ms with `125 + 9.6` ms and passes. The same window with the 83 ms or 470 ms readings already cited in the comment at `:381` also lets a coalesced drain of about 140 ms through (`0.140 < 0.095 + late` once `late` exceeds that drain's own delay by about 45 ms).

A quiet batch in the same run can still fail a systematic bug. The batch that actually took the extra hold is the one whose window is dirtiest, and that is the assertion that goes green. The lone-tick check (`:473`, hard 100 ms) does not cover this: it skips when main was over 20 ms late (`:475`).

Fix: take the lateness of the strict beat due nearest the drain deadline (a beat that ran by `d.at`), and keep fixed slack around 20 ms. Slack plus one scheduling quantum stays under 50 ms, so one extra hold still fails; post-fire work in `drain()` still fits.

**2. SHOULD-FIX** — `MIDIMacTests.swift:135` (`MainLag` at `:123`)

A user-interactive strict 10 ms beat is a fair witness that this QoS class is being scheduled late. The arm thread and its hold timer are both user-interactive and strict (`MIDIArm.swift:118`, `:341`). It is not a sound budget for this assertion. `busy.most` is the worst beat on a different, idle serial queue over the whole 600 ms wait, including after the note has arrived, added with no cap: limit = `2 + most×1000` ms.

A coalesced hold is 90–100 ms late when that proxy is near 0, so the test still fails the regression while `most` stays under about 88 ms. The strict arm was already 75 ms late at load 117, which leaves 13–25 ms of headroom. The max of ~60 beats overshoots the delay the send itself hit, and this queue is not the arm's thread and not MIDIServer, which is where that 75 ms was observed. Once the worst beat reaches ~90 ms, a coalesced hold passes. Uncapped, the two cases overlap.

Fix: compare the send with the strict beat due nearest the note's time, and assert the send is no more than a small excess past that beat (well under 90 ms). That allows a wedged machine and still rejects coalescing. An uncapped whole-interval max will not, at the gap between 75 ms and 90–100 ms.

**3. SHOULD-FIX** — `MIDIModule.swift:157` and `:162`; cleared at `MIDIMacTests.swift:355`. Lifetime: `MIDIMacTests.swift:85` and `:93`

`observeHeard` is a process-wide `nonisolated(unsafe)` optional written on the test thread and called on the arm thread with no lock or atomic. The `handed` update inside the hook is locked; publishing and clearing the hook is not. `defer` sets it to nil while the arm can still be loading or calling the closure, which is a retain/release race on the closure.

`MainLag`'s `beats` array is properly locked, and `due` is only touched on the timer's serial queue. The handler is `[unowned self]`, and `stop()` only `cancel()`s. Cancel is asynchronous, so a queued handler on the send test's user-interactive queue can run after the local `MainLag` is released.

Fix: copy the hook out under a lock both sides share, and `[weak self]` or wait for cancel on that queue before releasing `MainLag`.

Nothing else in these two commits is blocking. The hook and `MainLag` now start before the sender (`:354`–`:361`). The new clock ceiling is 5 ms tighter than the one that already rejected a coalesced hub at load 100–200, so those runs would still fail. The −304 path only changes the skip text (`:35`) and does not loosen a timing bound.
