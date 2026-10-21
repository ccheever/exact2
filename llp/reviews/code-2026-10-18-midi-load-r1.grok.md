# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `5dfdfff32`.
- **Method:** one brief (sha256 `5f9a8987f1dc220c03277895d924e72f17f5683267e8f996ac5bd49dc0bc7ae6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two should-fix on a first attempt (a user-interactive queue's lag added to the batch ceiling): summing lets a second hold pass; the queue is no proxy for the hub's handoff (Astra's run of the same brief failed: the model was at capacity).
- **Disposition:** the attempt dropped; `ace12a15d` records the hub's handoff instead.

Nothing here is a blocker. The production wait is unchanged, and a batch held well past a second period still fails. Two test-oracle problems are worth fixing: the new sum spends the second-hold margin, and the stand-in timer is not the arm thread's wakeup.

**1. SHOULD-FIX — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:375`**
Summing the two maxima onto `0.100` lets a batch held a second time pass.

`heard()` arms the drain with `max(0, 0.05 - (now - message.at))` (`MIDIModule.swift:177`). While the tick reaches the hub within 50 ms, that is an absolute deadline at `stamp + 0.05`: arm delay is subtracted out of the wait, not added after it. Delivery is then `stamp + 0.05 + main-queue lateness`. The old ceiling `0.100 + main` left one extra 50 ms, and `<` failed a clean second hold.

The load-95 miss fits that slack being used up, not a 2 ms arm delay. 111.5 ms from the oldest stamp, minus the 50 ms window, minus the 9.6 ms main reading, is about 52 ms still unexplained. The 100 ms bound already covered 50 ms of that; the run lost by 1.9 ms. A handoff under 50 ms cannot produce that 1.9 ms, because it does not move the deadline. A handoff past 50 ms can: the timer is armed already due, and delivery becomes `stamp + handoff + main`. That needs a handoff of about 102 ms, and the fair second-hold ceiling is `max(0.100, handoff + 0.050) + main`, not `0.100 + main + handoff`.

At load 30–90 the proxy is an otherwise idle user-interactive queue. Its strict 10 ms timer is typically a few milliseconds late over a 50–100 ms window (this commit's own residual at load 95 is 1.9 ms), not ~50 ms. It is above zero on a window that contains several beats. Then a normal second hold, delivered at `0.100 + main`, is under `0.100 + main + arm` and the assertion passes. If a run's proxy actually tracks a large stall, the hole grows by that whole reading and the second hold passes with about 50 ms to spare. The spacing check only requires batches more than 40 ms apart, so it does not catch this.

Fix: let the handoff extend the budget only by the part the 50 ms window does not already absorb, `0.100 + mainLate + max(0, handoff - 0.050)`, which is `max(0.100, handoff + 0.050) + mainLate`. A second hold then stays on the failing side of `<`.

**2. SHOULD-FIX — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:80`**
A strict timer on a private user-interactive serial queue is not a fair proxy for the delay that reaches `heard()`.

The arm thread is a dedicated `Thread` at `.userInteractive` blocked in `CFRunLoopRunInMode`, signaled with `CFRunLoopWakeUp` from CoreMIDI's realtime callback (`MIDIArm.swift:116`, `MIDIArm.swift:275`). The stamp in that callback is the packet time, taken before `MIDIReceivedEventList`. On a virtual source that hop goes through MIDIServer, another process, before the arm thread runs at all. The same thread also services the client: a wakeup that lands during `enumerate()` or another CoreMIDI call waits until that call returns. The proxy sees none of those waits. It sees GCD timer jitter on an idle queue in the user-interactive pool, which can be larger than this dedicated thread's wakeup when that pool is busy, and much smaller than MIDIServer or a blocked arm thread. Same QoS does not make them the same lateness.

The same test still requires Start within 10 ms (`MIDIMacTests.swift:380`). Start is `DispatchQueue.main.async` from the arm thread, so handoff delay is added in full. A path that is systematically ~100 ms late would fail that check too. One slow tick batch is still possible; this timer would not record a MIDIServer hiccup while it happened. The 6/6 passes at load 32–89 are at loads that did not reproduce the miss.

Fix: if the ceiling needs the real handoff, measure stamp-to-`heard()` (a test hook on the hub), and feed that into the formula in finding 1. Do not add this timer's lateness into the bound. A reading below 50 ms widens an allowance `heard()` did not move; a MIDIServer stall the timer missed leaves the flake in place.

`due` and `[unowned self]` are sound. After `resume()`, only the timer's serial queue reads or writes `due` (the init write happens before `resume()`). `beats` is the only state `most()` reads from the test thread, and both sides take the lock. `cancel()` keeps a handler that has not started from running; this test then does the Start checks and `wait(0.3)` before the locals die, which is longer than an in-flight 10 ms handler. The off-main handler can be inside `stop()`, unlike the old main-queue meter, but it only touches the locked list and `due`, and it is finished before that later wait returns.
