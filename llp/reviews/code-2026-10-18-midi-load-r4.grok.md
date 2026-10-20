# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 4, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ce32fa972`.
- **Method:** one brief (sha256 `1d5c104c1b2e43a53ec3d71a0580935f0ab1c2548e7a58fc59c0612af8684579`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker (a late Start unchecked while the test passes) and a should-fix (the send probe at 20 ms against a 2 ms ceiling, not retried).
- **Disposition:** taken in `067dc3c86`.

**1. BLOCKER — A late Start is not checked, and the test still passes, whenever main is only slightly late.** `host/apple/tests/ExactKitTests/MIDIMacTests.swift:414`

Scenario: Start is held with the tick batch and shows up about 50 ms after it was sent (or a coalesced main-queue timer delivers it around 100 ms). `lag.most(fa.at, start.at)` looks at every 10 ms beat due in that window. One beat 3 ms late makes `<= 0.002` false, so the 10 ms assert never runs. Nothing is skipped. In the same function a batch is still timed with main lag up to 20 ms (`MIDIMacTests.swift:408`), so a run can record 3 good batches and a green result while Start was held. `most` reports that 3 ms the same way at ordinary load (20–60) as it does for those batches. A quiet main (every beat ≤ 2 ms) still catches it; that is a narrower band than the one this test uses to decide the machine was quiet.

Fix: keep a fixed ceiling, and apply it for the same 20 ms bound as a batch. If `lag.most(fa.at, start.at) <= 0.020`, assert `start.at - fa.at < 0.040`. A main-queue async under a 20 ms stall still passes. A Start held for another batch (~50 ms) or a coalesced drain (~100 ms) does not. Do not add the measured lag to 10 ms.

**2. SHOULD-FIX — The send probe may be up to 20 ms late and the arm is still required to be within 2 ms, and a failing sample is not retried.** `host/apple/tests/ExactKitTests/MIDIMacTests.swift:145`

Scenario: the user-interactive stand-in is the arm's timer (`MIDIMacTests.swift:127`). Its worst beat in the window is 12 ms, so `lag <= 0.020` qualifies. The hold timer fires 12 ms late, the same delay. `XCTAssertLessThan(late, 2)` fails correct code, `timed` is set before the assert, and the other four attempts do not run. A coalesced arm (90–100 ms, probe near 0) still fails the 2 ms ceiling; this path is a false red, not a false green.

Fix: qualify on `lag <= 0.002`, same number as the ceiling, and leave the five retries for a window that is only a little late. A 90 ms send still fails any attempt that qualifies.

**3. NIT — The batch skip reason names main lag when the hear time was the reason.** `host/apple/tests/ExactKitTests/MIDIMacTests.swift:406`

Scenario: every batch was heard more than 50 ms after its stamp and main lag stayed under 20 ms. `worst` only tracks `late`, so the skip says main lagged a few milliseconds and the machine was too loaded.

Fix: keep the larger of main lag and handoff (or count how many batches each gate rejected) and say which gate failed.

Nothing else blocks. A lag is no longer added to a ceiling. A coalesced send whose probe stays within 20 ms until the note is still scored against 2 ms, including when a stall lands after `first.at` (that stall is outside `most(due - 0.250, first.at)`). A second hold with handoff ≤ 50 ms is due at 100 ms and `XCTAssertLessThan(..., 0.100)` rejects it; measured main lag does not widen that. Handoff of 80 ms (heard at 80, held to 130) does not qualify, so this loop does not pass it either. That delivery is past the lone-tick ceiling of 100 ms (`MIDIMacTests.swift:496`), which is the check that already fails a coalesced hub. Send retries cannot pass a 90 ms arm by shopping for a later attempt: a sample that qualifies and misses 2 ms sets `timed` and stops. `MainLag`'s handler is `[weak self]` (`MIDIMacTests.swift:85`) and keeps `self` for the rest of the callback, so a fire on the user-interactive queue after `stop()` does not touch a freed object. `observeHeard` copies the closure under `observeLock` and calls it after the lock is released (`MIDIModule.swift:157` and `MIDIModule.swift:167`); the test writes `handed` under the test lock.
