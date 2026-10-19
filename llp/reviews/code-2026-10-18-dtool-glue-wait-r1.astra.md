# Code review: the dev pages' hold wait for the input glue (LLP 1122 D8, stage 8 E), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `085931b670`.
- **Method:** one brief (sha256 `966ee4dd34b89ca05547718bf2efa4fd4863117997957f8c0257c52ece1a701a`), shared by both reviewers, blind to the other review; commit 085931b670. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium; smaller points).
- **Disposition:** taken in the next commit: a let-go needs no glue (with none loaded nothing is held), so a registration's `hold: null` no longer waits 60 s and holds the wasm page's `designing` queue (pins, inspect, live edits) behind it; the wasm wait's stale predicate is the frame's `current()` (a new stream or epoch ends it), as the JS page's; the tests check the glue's signal ends the wait as it comes (100 ms between two 250 ms looks) and their time bounds are looser. Not taken: the holding chain and the wasm copy are not driven by a test (both read only the helper's answer and their own `current()`, which the T4 gate's after-rebuild line runs end to end); the agents' 2 s waits are outside the assignment.

One issue blocks approval: the longer wait can stall the wasm design queue.

- **[P2] Handle clears before waiting for glue.** [host/web/dev.js:751](/tmp/rv-dt-glue/host/web/dev.js:751), also [inspect-dev.js:489](/tmp/rv-dt-glue/host/web-js/inspect-dev.js:489). Registration always sends a hold frame, including `hold: null`. With glue unavailable, even that clear waits 60 seconds. On wasm, pins, inspect commands, and live edits queue behind it on `designing`. An in-memory execution of the actual script reproduced a subsequent pin remaining blocked until **60,250 ms**. A newer clear cancels the previous wait but then starts its own. Process a current clear immediately, calling `hold` only if available; no installed glue means nothing is held.

Two smaller points:

- **Wasm does not cancel the wait on a stream change alone.** [host/web/dev.js:751](/tmp/rv-dt-glue/host/web/dev.js:751) checks only `holdLatest`. If `hello` changes the nonce/epoch before another hold arrives, the obsolete wait remains until readiness or timeout. The later `current()` guard correctly prevents applying or reporting it, but does not release the queue promptly. Include the stream and epoch checks in the wait’s stale predicate, as the JS implementation does.

- **The tests miss the caller behavior and have tight timing assertions.** [hold-wait.test.mjs:10](/tmp/rv-dt-glue/host/web-js/hold-wait.test.mjs:10) tests the helper, not either command queue. Its event case also passes through polling alone. The three-second timer with a four-second upper bound can fail under load. Controlled timers should cover event-only resolution, cleanup, hold→clear, reconnect, and the wasm caller.

The other requested checks looked sound: newer holds and clears do not resurrect superseded holds; JS stream/epoch cancellation works; wasm generation/plan guards prevent applying to another plan. Readiness is announced with a callable `hold`, once per factory invocation. Already-dispatched events, recreation, and polling fallback worked in probes; completed waits removed their timers and listeners.

The agents’ two-second waits remain unchanged at [agent.js:299](/tmp/rv-dt-glue/host/web-js/agent.js:299) and [glue.js:1162](/tmp/rv-dt-glue/host/web/glue.js:1162)—outside this assignment.

Validation: both new unit tests passed, plus in-memory probes of both callers and the actual glue factory. The browser test was not run because it creates a profile. No files changed.
