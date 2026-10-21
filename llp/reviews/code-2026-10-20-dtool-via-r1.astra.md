# Code review: tamber tool --via's dev loop probe tried again on a timeout, round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `6e17fe650f`.
- **Method:** one brief (sha256 `79a91a343efb1d00f7ad1c90155b86a0fc8d8bee58db22236390fccce65f601c`), shared by both reviewers, each blind to the other's review; commit 6e17fe650f. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX).
- **Disposition:** (1) taken in the round's fix commit (named in round 2's records): the refused-port case counts the fetches to it (one, with five tries allowed) instead of timing them; it fails with the timeout guard removed (checked by hand).

Nothing blocks in the implementation. One test should be fixed.

1. **SHOULD-FIX — The refused-port assertion does not prove “no retries” and can flake under load.** [scripts/tamber-tool-answers.test.mjs:35](/tmp/rv-dt-via/scripts/tamber-tool-answers.test.mjs:35). Five immediate connection refusals could all finish within one second, so removing the `TimeoutError` guard could leave this test green. Conversely, a worker descheduled for 1.1 seconds could fail after correctly making one attempt. Assert the fetch attempt count for a controlled refusal; keep the real-server timeout coverage.

The remaining checks look correct:

- **Retry classification and control flow:** [tamber-server.mjs:1308](/tmp/rv-dt-via/scripts/tamber-server.mjs:1308) retries only `TimeoutError`, the specified [`AbortSignal.timeout` reason](https://dom.spec.whatwg.org/#dom-abortsignal-timeout). Refusal and redirect errors escape to the outer catch and return `null`. HTTP errors produce a response and return `{ app: null }` without retrying. The `try` is a valid loop body; a successful assignment makes `got` truthy and ends iteration. The fifth timeout rethrows. Token-body parsing and hello failures are outside the retry loop.

- **Delay and epoch checks:** The target probes at [line 1353](/tmp/rv-dt-via/scripts/tamber-server.mjs:1353) and [line 1367](/tmp/rv-dt-via/scripts/tamber-server.mjs:1367) are mutually exclusive. Five timeouts cost nominally 10 seconds; success near the fifth deadline followed by a slow hello costs up to approximately 20 seconds. Reuse additionally includes the unchanged single TOOL_ORIGIN probe, up to 12 seconds, and existing asset checks. These bounded waits are acceptable here. A target returning a different epoch after retry still refuses reuse.

- **Other tests and registration:** The stand-in exercises real fetch cancellation, recovery on attempt two, default single-attempt failure, and exhaustion after three attempts. The 30/60-second test deadlines leave reasonable headroom. Registration at [async.mjs:114](/tmp/rv-dt-via/scripts/async.mjs:114) correctly includes the file in the parallel glue lane.

Reviewed HEAD `6e17fe650f4b` against its parent. No files changed; no builds or tests run.
