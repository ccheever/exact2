# A resource reply over its CPU budget resends completed POSTs

**Status:** Closed
**Severity:** P2
**Systems:** `runner/src/runner/overrun.rs`, `runner/src/runner/settlement_tests.rs`
**Author:** Codex
**Date:** 2026-11-02
**Commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

When a resource's reply mapper exceeds its CPU budget, the runner retries the resource's entire answer, including requests whose server-side effects already completed. A resource that creates a job with POST can create three jobs from one ask. A slow reply mapper or temporary CPU contention is enough; the server need not fail.

`hold_overrun_reply` checks the resource, stream and retry count, but does not check whether the request or any preceding round had effects (`runner/src/runner/overrun.rs:156-203`). It clears the continuation and schedules a fresh answer. `reask` starts that answer again at lines 238-245. The existing test at `runner/src/runner/settlement_tests.rs:1006-1025` explicitly confirms a new POST after the first POST's reply overruns.

Reproduced on this Mac with a standalone Rust probe linked to this checkout's warm runner, plan and kernel rlibs. Its source returns `Request::post_json("https://example.test/create-job", …)` from `answer`, and `DataError::Overrun` from `parse`. After each emitted request, the probe supplies HTTP 201 and advances the runner clock. It emits POSTs at 0, 250 and 1250 ms, under tickets 1, 3 and 5, for one resource ask. The third mapper failure finally ends it. Probe: `/tmp/exact-runner-post-overrun-probe.rs`; no network requests were made. This confirms duplicate dispatches, without claiming an external service was modified.

Keep the received reply and retry its processing where possible. Otherwise fail after an effectful round unless the source explicitly promises replay safety. Track effects across earlier rounds too: a later GET's mapper can overrun after a preceding POST completed. Add coverage that an already completed unsafe request is emitted once.

Deduplication: searched current `issues/` and open and closed issues in the local `exact2` and `exact` checkouts for overrun, budget retries, POST retries and effect replay. No matching report found. The current POST retry test codifies this behavior; it does not establish that the operation is idempotent.

## Resolution

Requests retain whether any earlier round had effects. An overrun after such a round fails without re-asking the resource, including after a later GET or worker continuation.

## Verification

`an_overrun_post_reply_fails_without_replaying_the_request` passed for direct POST replies, POST-to-continuation and POST-to-GET chains. Existing safe-request retry tests passed.
