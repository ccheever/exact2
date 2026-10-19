# Code review: the dev pages' hold wait for the input glue (LLP 1122 D8, stage 8 E), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `384f4a71c3`.
- **Method:** one brief (sha256 `ca017d0f55e07a8632551446e90b656ff02b78107351cf841cc467a0cdd7ad78`), shared by both reviewers, blind to the other review; commit 94410eaa13. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (a smaller point).
- **Disposition:** nothing blocks (both). The smaller point, the signal's test depending on timing, taken in `59e8954d2a`: the signal is dispatched with no hold there, so only it can end the wait before the 10 s bound. Not taken (as in round 1): no test drives the holding chain or the wasm copy (the T4 gate's after-rebuild line runs a real hold end to end); the agents' 2 s waits are outside the assignment.

**Nothing blocks.** The round 1 correctness fixes are sound; I found no correctness regression in `94410eaa13`.

- **Let-go before glue loads:** [inspect-dev.js:487](/tmp/rv-dt-glue2/host/web-js/inspect-dev.js:487) and [dev.js:755](/tmp/rv-dt-glue2/host/web/dev.js:755) bypass the glue wait. A pending hold superseded by a clear exits on polling; the clear releases the wasm command queue without starting another wait.
- **Let-go with an existing hold:** both paths call the installed glue. Its synchronous `replaced` callback reports the old hold’s revision; the clear reports its own revision. Old-stream callbacks remain suppressed. Clears correctly bypass the `ended` and plan-mismatch exclusions; non-null holds retain those checks.
- **Stream cancellation:** [dev.js:751](/tmp/rv-dt-glue2/host/web/dev.js:751) now checks frame identity, stream and epoch throughout the wait. Generation changes, same-plan reholds and clears during swaps behaved correctly in probes.
- **Readiness signal:** [input-glue.js:351](/tmp/rv-dt-glue2/host/web/input-glue.js:351) still announces through a microtask after `hold` is installed. Missed signals, polling fallback and wait cleanup worked.

One smaller point remains:

- **[P3] The event test is still timing-dependent.** [hold-wait.test.mjs:27](/tmp/rv-dt-glue2/host/web-js/hold-wait.test.mjs:27): if the 100 ms callback runs at 200 ms, polling at 250 ms satisfies `late < 100` even with the event listener removed. I reproduced this with controlled timers. The test improves normal-case coverage, but controlled polling would prove event-only resolution reliably.

Leaving the agents’ two-second waits unchanged is consistent with scope. Deferring caller tests is a coverage tradeoff: T4’s rebuild gate does not specifically exercise hold→clear or stream-only cancellation.

Validation: all three helper tests passed, plus 29 in-memory probes using the source with DOM/runtime dependencies stubbed. The browser test was not run because it creates a Chrome profile. No files changed; no modifying git commands ran.
