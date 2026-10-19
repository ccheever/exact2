# Code review: Apple MIDI's hold and tick-batch timers strict (MIDIMacTests' coalesced timers), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `750819338`.
- **Method:** one brief (sha256 `7ad7e3f8aac99f74c734ee4de9b849a01786456528637702c8f510c41699b13e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking in the two commits; a should-fix (the hub's tick batch still coalesced) and a nit (the stale-drain test's sleeps).
- **Disposition:** both taken in the hub commit.

Nothing in these two commits is a blocker. The hold timer is safe, and the clock-test change does not hide the send regression.

**1. SHOULD-FIX — Clock-in is still batched on a coalesced main-queue timer — `host/apple/Sources/ExactKit/MIDIModule.swift:175`**

This is pre-existing, not a regression in these commits. It is the same coalescing the commits just measured. `heard` arms the 50 ms Timing Clock batch with `DispatchQueue.main.asyncAfter`. That API has no strict flag and no leeway; with an idle main it fires 100–150 ms late. `DispatchQueue.main.async` (the Start path) is a wakeup and stays prompt. The clock test never sees the late case: `wait` pumps the main run loop every 5 ms, so the timer is checked while the thread is already awake. Under App Nap or an occluded window, batches land at ~150 ms instead of 50 ms. The runner only issues hits out to 100 ms past delivery and assumes ~50 ms between deliveries, so followed-clock notes are issued late. The arm's strict hold timer then plays those late times on time.

Fix: arm a one-shot `DispatchSourceTimer` (`.strict`, leeway 0, userInteractive queue) only while a tick batch is waiting, and from its handler `DispatchQueue.main.async` the existing `drain(armed:)`. The stale-`armed` check already drops a fire that lost to a note. Do not call `drain` on the timer queue.

Leave the 3 s watchdog as a `CFRunLoopTimer`. It only re-enumerates ports. 150 ms on a 3 s poll does not move a send, and a repeating strict timer for the whole time an input is open would keep the process out of timer coalescing.

**2. NIT — The other clock test still paces with `Thread.sleep` — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:365`**

`testTicksAfterADrainAtOnceWaitTheirOwnFiftyMilliseconds` still sleeps 10 ms, then 20 ms, on a global queue. In this session those sleeps run 100–150 ms. The note then arrives after the first tick's 50 ms drain has already fired, so the race the test names (a drain armed before the note must not deliver the later ticks early) does not happen. The assertions are lower bounds, so the test still passes. `pause` is already in this file; the sender should use it.

The send path itself is not still coalesced. `MIDIHub.op` goes straight to `exact_midi_op` and the arm. The hold timer was the only timer on that path.

A stale `perform { schedule() }` is safe. The handler on the global queue only calls `perform` (`MIDIArm.swift:343`); `held`, `timer`, and CoreMIDI stay on the arm thread. `cancel` does not dequeue a block already handed to the run loop, so that block can run after a newer timer, or after `close` in the same pass. `schedule` is idempotent: it cancels whatever timer is current, hands only what is still in `held`, and arms again from a fresh `CACurrentMediaTime()`. A run after `close` sees `held` already cleared (`MIDIArm.swift:382`) and returns at the `guard` without `hand` and without arming another timer. A block queued only after `CFRunLoopStop` never runs; `[weak self]` means it does not keep the arm alive. `close` cancels, clears, disposes, sets `closed`, then stops the run loop, all on the arm thread, so none of that interleaves with `schedule`.

`[weak self]` is the right capture. The nested `self?.schedule()` refers to that weak optional, not a strong `self` (there is no `guard let self`). The timer does not retain the arm. After `deinit`, the handler does nothing.

The timer is armed only while something is held. `schedule` returns before creating one when `held` is empty, it is one-shot, and `send`, `retract`, `settle`, and `close` all cancel it. One precise wakeup per held deadline is the energy cost of handing over on time.

The test change does not hide the product bug. `testASendArrivesOnItsTimeAndARetractionSendsNothing` is untouched and still requires the note within 2 ms; that is the test that failed 5/5 on the old arm and passed 5/5 on the new one. The clock test failed because its own sender slept ~100–150 ms a tick, so 48 ticks never reached Start inside 5 s. A device clock is not `Thread.sleep`. Pacing it with a strict timer is the right fixture, and the batch-count, spacing, hold, and Start assertions are still there.
