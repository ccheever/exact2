# Code review: MIDIMacTests' latency bounds and the main queue's own lag, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e1833ee20`.
- **Method:** one brief (sha256 `4b4c27fea6f482383666dab4b42d3ae5428adbbdc07cfb0adaf6eb3781b12039`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; residual risk that a window maximum loosens the ceiling (Astra's first finding).
- **Disposition:** covered by Astra's, taken in `20141b7db`.

Nothing is blocking. The new ceiling still rejects a coalesced drain and a second hold, and `MainLag`’s deadline, lifetime, and cancel are sound.

**Coalesced drain.** The clock test cannot see one: `wait` already spins the main run loop every 5 ms (`MIDIMacTests.swift:88-90`), so an ordinary `asyncAfter` is flushed on that wakeup. `MainLag`’s 10 ms strict timer (`MIDIMacTests.swift:76-83`) is the same kind of wakeup and does not make that worse. That is acceptable because the lone-tick test is the one that covers it, and it never arms `MainLag` during the idle 0.4 s (`MIDIMacTests.swift:419-429`). A probe wakeup there would run a coalesced drain on time and hide the regression. With the probe stopped, the old hub’s 193 ms still sits above `0.100 + late` whenever `late` is the drain’s own queue delay: the slack above the 50 ms batch is 50 ms, and coalescing adds about 100 ms. Their 193 ms vs 101 ms run is that case (`late` about 1 ms).

**A batch held a second time.** Wakeups cannot fire the drain before its deadline, so a 100 ms hold is not masked. For the batch that actually hit `most`, the measured hold is about `0.100 + Q` against `0.100 + late`. `XCTAssertLessThan` fails when `Q >= late` (`MIDIMacTests.swift:359`).

**`due` after a late beat** (`MIDIMacTests.swift:80-81`). One handler services every missed 10 ms beat; libdispatch then aims the next deadline at the next grid point strictly after now. `due = max(due + 0.010, now - 0.010)` does the same jump, up to 20 ms short of that grid. The following on-time beat can show that 20 ms, and it does not raise `most` above the stall already recorded, nor does the error accumulate. An on-time beat just does `due += 0.010`.

**`unowned self` and cancel** (`MIDIMacTests.swift:78-85`). The handler does not retain `MainLag`, and `MainLag` retains the timer, so there is no cycle. `stop()` runs on main, while the instance is still held, and `cancel` sets the canceled flag before it returns. A later invoke on main does not run the event handler. Both call sites cancel before the asserts, so a failing assert does not leave the 10 ms timer armed.

**Flakes.** Correct code is about 50 ms plus the queue delay. `most` is that delay’s high-water mark over a window that covers the batches, and the extra 50 ms absorbs one beat of sampling error. That is what stops the load-85 false failures (handlers 100–470 ms late against a fixed 100 ms). The trailing `wait(0.2)` (`MIDIMacTests.swift:347-348`) is doing real work: a handler due during the last batch can still be 200 ms late when `finished` becomes true, and it has to be allowed to record. Residual risk, not a broken bound: `late` is a window maximum, so a stall that overlaps the probe and not the drain loosens the ceiling. Under a sustained stall the drain is late by about the same amount, which is the case the 6/6 runs at load 35–89 exercised.
