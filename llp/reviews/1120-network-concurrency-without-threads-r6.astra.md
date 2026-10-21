# Design review: LLP 1120 r6, 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `35e5cebb3c38e9986f32cb19f1c80965865719e280b14b3ce143412314552970`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r7 (§0, round 6).

---

**R6 is not ready for a ruling authorizing implementation as written.** Most round-5 findings now have concrete fixes. Three remaining mechanism issues can prevent progress or change cancellation semantics; two smaller inconsistencies need correction.

1. **MAJOR — The narrowed `forget` rule stops cancelling non-GET streams.** [RFC:182](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:182), [RFC:235](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:235)

   R6 defines cancellable operations as safe reads and says every other forgotten job merely becomes forgotten. That fixes ordinary writes, but loses the existing stream exception.

   Today **every stream receives an abort controller**, regardless of method ([executor_core.rs:420](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:420)), and `forget` fires it. POST-based SSE with `exactBodyFrom` is supported and tested ([executor_body_tests.rs:271](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_body_tests.rs:271)).

   Under the written rule, forgetting a quiet POST SSE stream leaves it running, potentially retaining its stream count, response reservation and upload indefinitely. Streams have no request timeout to rescue this.

   **Required change:** distinguish the cancellation categories explicitly. Forget cancels ordinary safe reads **and all streams**; ordinary writes retain the new noncancellation rule. Also distinguish retirement, which aborts all HTTP today ([executor_core.rs:690](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:690)). Add POST/file-backed SSE forget cases during preparation and after opening.

2. **MAJOR — Straggler exit can surrender an executor’s reserved ordered capacity permanently.** [RFC:245](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:245), [RFC:267](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:267)

   The new replacement rule does not preserve the base reservation through recovery:

   - The sole ordered adapter thread times out while resolving and becomes a straggler.
   - No subsequent HTTP job is ready, so no replacement is created.
   - The resolver returns; the straggler must exit and return its permit.
   - Another executor consumes that permit.
   - A later ordered HTTP request now has no adapter thread and cannot create one because the process counter is full.

   This executor has no remaining stuck operation, yet its ordered HTTP progress depends on unrelated executors releasing capacity. That contradicts the promised protection of its reserved adapter. The resolver scenario is real: cancellation cannot interrupt the current system lookup ([rustls_http.rs:188](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:188)).

   **Required change:** define how one ordered adapter remains reserved throughout a live executor’s lifetime. For example, a returning straggler can become idle when no replacement exists; otherwise a replacement takes over the base role. Specify replacement promotion and retirement, and test recovery with no queued successor followed by global exhaustion.

3. **MAJOR — The new final resource releases lack a complete wake mechanism.** [RFC:159](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:159), [RFC:173](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:173), [RFC:301](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:301)

   The later-of accounting rule correctly retains upload bytes beyond `didComplete` when the lent buffer survives. But the deallocator only drops the buffer’s `Arc`; its eventual release is absent from the starter’s wake list and the finisher’s three event types.

   A concrete case is a 50 MiB upload with a 4 MiB response ceiling. After timeout, drain, completion and metrics processing, the lent buffer still holds 50 MiB. A queued request with an 8 MiB ceiling needs over 16 MiB and cannot start with less than 14 MiB free. When the buffer finally disappears, none of the specified events wakes the starter. The queued request has no armed deadline.

   The process counter has a related omission: a starter waiting to replace its straggler must be notified when **another executor’s stream, reader or retiring thread** returns a permit. Its own adapter’s release notification is insufficient.

   **Required change:** specify an accounting-and-notification path for every final physical release, including the buffer deallocator and global permit returns. It must wake the affected waiters without requiring another admission, drain or transport callback. Test each release as the only remaining event capable of making progress.

4. **MINOR — The straggler limits and Linux thread gate disagree.** [RFC:245](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:245), [RFC:422](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:422), [RFC:478](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:478)

   One live adapter executor with its base ordered thread stuck and one replacement has **16 executor-owned threads**. With no streams, readers or retiring executor, §5’s formula permits only 15.

   The creation threshold also differs: D4 permits replacement while there are “at most two” stragglers, whereas the test says two stragglers already force waiting. The separate limit of two live replacements does not resolve that difference.

   **Required change:** state the maximum total ordered adapter threads and the precise replacement predicate, then include extra live ordered threads in the measurement bound.

   Also key the reservation table to the selected transport. Grok’s unnumbered round-5 question about macOS render with `EXACT_RENDER_TRANSPORT=platform` remains implicit ([executor.rs:32](/Users/admin/x2p2/apps/exact2-http/host/render/src/executor.rs:32)); its base and stream-permit costs should be explicit.

5. **MINOR — The file-backed-stream failure row contradicts independent failure retention.** [RFC:295](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:295), [RFC:320](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:320)

   D5 says an independent completion “of any kind” retains its running charge until drain. The special sizing-refusal row instead releases the stream’s ceiling “at the stream’s end.” Those are different events; today an interested stream’s terminal outcome retains its charge until drain ([executor_core.rs:853](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:853)).

   The two-stream test also needs explicit scheduling ([RFC:417](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:417)). Two admitted 12 MiB ceilings reserve at least 48.25 MiB together. Neither 20 MiB file fits the remaining bytes. For one to start while the other fails after sizing, the fake must arrange the other’s refusal and drain before the successful sizing decision; otherwise both can legitimately fail.

   **Required change:** align the row with retained independent failures, and specify the test’s sizing/refusal/drain sequence. The immediate-refusal policy itself removes the old deadlock.

Using the finding numbers in the two round-5 records, the disposition is:

| Round-5 findings | R6 assessment |
|---|---|
| Grok B1: reader wake and parked-upload state | Resolved: reader results wake the starter; parking releases the slot, preserves the waiting charge and disarms the deadline. |
| Astra M1 / Grok m6: logical versus physical release | The lifetime rule is resolved. Its final-release notification needs finding 3 above. |
| Astra M2: file-backed-stream reservation cycle | Resolved by immediate post-sizing refusal; the release-row inconsistency remains in finding 5. |
| Astra M3: adapter availability after timeout | Eligibility and deadline timing are specified. Reservation recovery and replacement bounds remain in findings 2 and 4. |
| Grok M2: forgotten ordinary writes | Resolved for ordinary writes, with the new stream regression in finding 1. |
| Grok M3 / m7: uploads with no next hop or a body-dropping redirect | Resolved: terminal release when no hop remains, and the last body-carrying hop governs body-dropping redirects. |
| Astra M4/M5 / Grok M4: measurement gates | Resolved: shipping-baseline comparison, treatment-failure precedence, C-only cold qualification and explicit C+P/Linux coverage. |
| Astra m6 / Grok m5: upload deadlines | Resolved: one deadline unless parked; generation invalidation handles stale firings. |
| Grok m9: protocol locking and sizing reservation | Resolved: finisher updates lane state; sizing holds only request buffers. |
| Astra N7 / Grok m8/N10: stale test, metrics guarantee and Limits wording | Resolved. |
| Grok’s unnumbered render-platform reservation question | Still needs the explicit backend mapping noted in finding 4. |

**§3 is not yet true of D1–D8 as a whole.** Its admission predicates, read/re-ask windows, fence, FIFO settlement, handoff eligibility, explicit independence, grants and redirect rules are supported. The starter/finisher/disposer separation and first-terminal-transition rule are coherent. Cancellation coverage, release-driven progress and the stream-failure row prevent the full preservation claim.

The per-origin policy is implementable as stated: six until protocol evidence lifts it, separate lane sessions, and all post-`resume` platform waiting charged to the deadline. That appropriately accommodates Apple’s ability to lower the HTTP/1.1 connection limit. [Apple documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost). Adapter acceptance starts ordinary adapter deadlines; file sizing starts upload deadlines, with only the explicitly parked path receiving a fresh clock.

The independent caps also have a clear meaning: physical resumed HTTP exchanges, held through cleanup, excluding streams, handoffs and undrained completions. The base arithmetic—6/15 threads and 96/240 for sixteen executors—is correct. The unresolved issue is maintaining and measuring those reservations after stragglers appear.

**§5 now closes the round-5 false-positive paths.** C7/A7 removes the seventh-worker-only explanation, and overlapping transactions on a shared connection demonstrate the intended transport mechanism. C/A6 prevents approval of a shipping-baseline regression. Treatment failures cannot disappear through a control-triggered rerun. Preconnect’s qualification no longer rejects its intended DNS warming.

After correcting the Linux thread bound, these gates can support a ruling for the stated workload. Their evidence remains limited to that workload and the operational definition of “cold”; the revised text appropriately avoids claiming an empty DNS cache.

Read-only review of the RFC, all ten earlier reviews, LLP 1041 §8.4 and amendments, and the executor/fetch/transport sources. No files changed; no builds or runtime tests run.

**Verdict: RIGHT WITH CHANGES.**
