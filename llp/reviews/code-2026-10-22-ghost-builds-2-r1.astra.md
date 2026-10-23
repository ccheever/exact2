# Code review: the ghost builds' follow-up (res close backstops, a test per door), round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `619360e3c0`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one SHOULD-FIX (door test timeouts) and two NITs, answered in 5741c64e03.

No BLOCKER findings.

1. **SHOULD-FIX — [tamber-remote.test.mjs:246](/tmp/rv-dt-ghost2/scripts/tamber-remote.test.mjs:246), [tamber-canvas.test.mjs:990](/tmp/rv-dt-ghost2/scripts/tamber-canvas.test.mjs:990), [tamber-relay.test.mjs:659](/tmp/rv-dt-ghost2/scripts/tamber-relay.test.mjs:659):** These tests retain [Bun’s default five-second timeout](https://bun.com/docs/test/writing-tests#timeouts), but allow five seconds for disconnect detection **after setup**. Give the outer tests enough time for setup, polling and cleanup; otherwise the runner can interrupt their intended failure path.

2. **NIT — [tamber-remote.test.mjs:233](/tmp/rv-dt-ghost2/scripts/tamber-remote.test.mjs:233), [tamber-canvas.test.mjs:979](/tmp/rv-dt-ghost2/scripts/tamber-canvas.test.mjs:979):** `once('data')` assumes the first chunk contains the expected SSE text. Accumulate through a complete frame before asserting, avoiding failures from chunk fragmentation.

3. **NIT — [dev-page-streams.test.mjs:77](/tmp/rv-dt-ghost2/host/web-js/dev-page-streams.test.mjs:77):** This launches the JS loop, so the resident `/__dev` guard remains uncovered. Add response-only disconnect coverage before/during/after the relay hello.

The guards look correct, including assignment after `await`. The preload stays confined to the test child. I found no remaining unbounded SSE handler relying solely on request close, or an early-response-close regression in the changed GET paths.

Read-only review; no builds or tests run.
