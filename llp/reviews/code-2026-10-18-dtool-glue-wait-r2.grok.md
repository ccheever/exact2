# Code review: the dev pages' hold wait for the input glue (LLP 1122 D8, stage 8 E), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `384f4a71c3`.
- **Method:** one brief (sha256 `ca017d0f55e07a8632551446e90b656ff02b78107351cf841cc467a0cdd7ad78`), shared by both reviewers, blind to the other review; commit 94410eaa13. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (a smaller point).
- **Disposition:** nothing blocks (both). The smaller point, the signal's test depending on timing, taken in `59e8954d2a`: the signal is dispatched with no hold there, so only it can end the wait before the 10 s bound. Not taken (as in round 1): no test drives the holding chain or the wasm copy (the T4 gate's after-rebuild line runs a real hold end to end); the agents' 2 s waits are outside the assignment.

Nothing blocks.

The three round-1 points this commit takes are fixed, and the let-go reorder still clears, drops, and reports the way those paths did before the wait moved.

**A let-go before the glue loads.** A registration with nothing held sends `{kind: "hold", hold: null}` (`scripts/tamber-builds.mjs` `replayHold` with `always`). On the wasm page that frame skips `glueHold`: `host/web/dev.js:755` waits only when `frame.hold` is set. It still has to be the latest frame on this stream (`dev.js:757`), then `planDigest` (`dev.js:758`). The digest await is the same one a pin already takes through `exact.ready`, and the server only emits this replay after that ready has resolved, so the `designing` queue is not sitting on the 60-second glue wait. At `dev.js:765` it calls `holds?.(null)` and reports `{kind: "held", state: null, revision}` with **this** frame's revision. With no glue, the call does nothing and that report is the one the server keeps (`holdSent` matches, state null). The JS page does this at `host/web-js/inspect-dev.js:487`, before `pressFeedback` and `glueHold`.

**A let-go of a hold the glue is showing.** Both queues stay serial (`holding`, `designing`), so the clear runs after the earlier hold has applied. `exact.hold(null)` is the glue's release (`host/web/input-glue.js:352-355`: a null state lets go with reason `replaced`). The previous frame's `dropped` callback reports the **old** revision and `replaced`; the server drops that because `holdSent` is already the clear's revision (`tamber-builds.mjs:420`). The clear's own report then carries the clear's revision and no reason.

**`ended` and plan.** Those guards stay on a real hold. Wasm: `dev.js:757` is `h && ended.has(revision)`, `dev.js:762` is `h && (swaps || moved)`. JS: `inspect-dev.js:497` is still after the early return. A null hold never took either guard. `sendHold` attaches `plan` only when `hold` is set, so a clear has no plan to match. A real hold that is stale, ended, or for another plan is still dropped unsaid, and its revision is the one in that report when it does report.

**Stream and epoch.** The wasm wait's stale predicate is now `() => !current()` (`dev.js:755`). `current` (`dev.js:751`) is the latest frame, `at.stream === channel.nonce`, and `at.epoch === epoch`. A hello bumps nonce and epoch (`dev.js:853-856`) without a new hold frame; the wait ends on the next 250 ms look and the frame is dropped unsaid. That is the same predicate the JS page already passed into `glueHold`.

**The signal test.** `host/web-js/hold-wait.test.mjs:22-30` sets the hold and dispatches `exact-hold` at 100 ms, inside the 250 ms look, and requires resolution within 100 ms of that dispatch. A poll-only wake at the first look is about 150 ms after `said` and fails that bound. The other ceilings are 8 s and 5 s (`:19`, `:39`, `:44`). `input-glue.js:350-352` still queues `exact-hold` and then assigns `exact.hold`, so a listener sees a function.

**Not taken.** There is still no test that drives the `holding` chain or the wasm copy. For a real hold that is the right scope: both callers wait, then re-read `exact.hold` and gate on `current()`, and the T4 after-rebuild line is the end-to-end hold. The new let-go branches are inline in the callers and never call `glueHold`; the scenarios above are that logic, and it is straight-line. The agents' 2-second loops are unchanged at `host/web-js/agent.js:298-300` and `host/web/glue.js:1162`, outside this assignment.

One smaller point: the new signal test's `< 100` bound includes the delay between `dispatchEvent` and the promise's `then`. The other bounds were opened because a loaded machine slipped a one-second window; this one can fail on that machine while the event path is fine, and a timer that fires in the last 100 ms before a look can also pass on the poll alone. A ceiling under 250 ms still separates the event from the look.
