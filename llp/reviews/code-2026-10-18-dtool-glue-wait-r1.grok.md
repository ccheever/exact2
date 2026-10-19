# Code review: the dev pages' hold wait for the input glue (LLP 1122 D8, stage 8 E), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `085931b670`.
- **Method:** one brief (sha256 `966ee4dd34b89ca05547718bf2efa4fd4863117997957f8c0257c52ece1a701a`), shared by both reviewers, blind to the other review; commit 085931b670. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks.
- **Disposition:** taken in the next commit: a let-go needs no glue (with none loaded nothing is held), so a registration's `hold: null` no longer waits 60 s and holds the wasm page's `designing` queue (pins, inspect, live edits) behind it; the wasm wait's stale predicate is the frame's `current()` (a new stream or epoch ends it), as the JS page's; the tests check the glue's signal ends the wait as it comes (100 ms between two 250 ms looks) and their time bounds are looser. Not taken: the holding chain and the wasm copy are not driven by a test (both read only the helper's answer and their own `current()`, which the T4 gate's after-rebuild line runs end to end); the agents' 2 s waits are outside the assignment.

No findings. Nothing blocks.

A later hold or let-go is not left behind the 60-second bound, and it is not applied ahead of the frame that was already running. The glue signal, a missed signal, and a generation change all settle on the function that is actually installed.

**Chain.** `hold` in `host/web-js/inspect-dev.js` sets `holdLatest` and appends `holdNow` onto `holding` before it returns (`inspect-dev.js:457-468`). The dev page does the same with `channel.holdLatest` (`dev.js:844-850`) and the `designing` queue (`dev.js:815-822`). Suppose a replayed hold starts `glueHold` while `exact.hold` is still missing, and a let-go arrives a second later. The let-go becomes `holdLatest` immediately. The earlier `glueHold` sees that on its 250 ms poll (`inspect-dev.js:479`, `dev.js:744`) and resolves. `holdNow` then drops it because `current()` is false (`inspect-dev.js:490`, `dev.js:755-756`). The let-go runs next. Once `exact.hold` exists, that next call returns on the first line of `glueHold` (`inspect-dev.js:476`, `dev.js:741`), so only the frame that is still the latest spends the long wait.

**Newer frame, stream, epoch.** On the JS page the poll predicate is `!current()` (`inspect-dev.js:485-489`): a newer frame, a new epoch, or a new stream ends the wait, and the frame is dropped unsaid. On the wasm page the poll is only `channel.holdLatest !== frame` (`dev.js:751`). Epoch and stream are enforced by `current()` after the wait (`dev.js:755-756`), so a stale hold is still not applied. A hello that bumps `epoch` (`dev.js:851-854`) without a new hold frame leaves that wait on `designing` until `exact-hold` or the 60 s bound. A hold or let-go that does arrive updates `holdLatest` and the poll ends it.

**Missed event and a new generation.** `createInputHandlers` queues `exact-hold` and then assigns `exact.hold` (`input-glue.js:351-352`). The microtask runs after that assignment, and after the rest of the function, so a listener already waiting reads a function. A listener that arrives after the event takes the same function from the opening check in `glueHold`. A wasm generation calls `holdQuiet` and keeps the installed function; it does not build a second glue or drop `exact.hold`. The 250 ms poll also treats a present `exact.hold` as ready, so a signal that nobody heard still resolves.

**Timers and listeners.** `end` clears the interval and removes its own listener before it resolves (`inspect-dev.js:478`, `dev.js:743`). The event, the poll, staleness, and the deadline all go through `end`. A second resolve is a no-op. The early return when `exact.hold` is already there registers neither.

**Agents.** The 2 s loops are unchanged: `host/web-js/agent.js:298-300` and `host/web/glue.js:1162`.

The tests cover a late signal, an already-installed hold, the deadline, a stale wait, and a single `exact-hold` that observes a function (`hold-wait.test.mjs`, `hold.test.mjs:39` and `:78-79`). They do not drive the `holding` chain or the wasm copy. The late-signal test also passes on the poll alone, because the hold is assigned in the same turn as the event and the upper bound is 4 s.
