# Design review: LLP 1120 r3, 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `ab76db0e8c5868c62baa766f1afd7b7796c2c0275525c65600c29d8853b479ae`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r4 (§0, round 3).

---

**The RFC is not ready for a ruling authorizing implementation as written.** The direction remains sound, and several round-2 findings are resolved. The remaining problems include a definite wait-forever case and incomplete resource-lifetime rules.

1. **BLOCKER — An upload that can never fit waits forever.** [RFC:195](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:195)

   A 60 MiB file with a 4 MiB response ceiling requires **more than 68 MiB** under D5, against the independent lane’s 64 MiB budget. The file passes `MAX_BODY_FROM_BYTES`, but “waits for bytes” can never succeed, even with an otherwise empty lane. FIFO then blocks subsequent work. The deadline does not rescue it because the reader never starts.

   Distinguish **temporarily unavailable capacity** from **a reservation exceeding the entire budget**. Refuse the latter unsent, as today’s [`reservation`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1010) already does for response ceilings. Declare the resulting admissible upload bound and test this exact case.

2. **MAJOR — Upload preflight is neither deadline-safe nor sufficient to establish the charge.** [RFC:196](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:196)

   Moving `stat` onto the starter puts potentially blocking filesystem work **before** the file-read deadline arms. Today, the [`stat` and read](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_body.rs:204) run inside the bounded reader operation, while the executor can settle its timeout. A stalled metadata operation would now immobilize the lane’s sole starter.

   Separately, a file can grow or be replaced between `stat` and reading. Today’s reader enforces the absolute 64 MiB maximum, not the size originally observed. Charging the earlier size therefore does not close the upload gap.

   Specify asynchronous metadata acquisition under the reader permits and deadline; tie reading/allocation to the acquired byte reservation; and define handling of growth or replacement. Also specify upload ownership across 307/308 replay: transferring an owned `Request` to each transport must not silently introduce uncharged retained copies.

3. **MAJOR — The per-origin rule cannot guarantee that deadlines exclude transport queuing.** [RFC:131](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:131)

   Six is an upper bound, not guaranteed immediately available capacity: Apple explicitly permits URLSession to use a lower limit. A previous `h2` result also does not guarantee the protocol of subsequent connections. Moreover, HTTP/2 has its own peer-controlled concurrent-stream limit. Removing the six-request restriction after observing `h2` can still resume tasks that wait inside the transport. [Apple’s connection-limit documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost?changes=_2_4&language=objc), [HTTP/2 stream concurrency](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.1.2).

   Thus round-2 Grok M2 is **not resolved** by the written mechanism. Either provide an implementable activation/capacity mechanism or explicitly rule that `exactTimeout` includes platform queuing after `resume`. Moving arming to response headers would instead leave connection and header stalls outside the deadline.

   Redirect waits also need an explicit rule: once the request’s single deadline has armed, waiting for a subsequent origin must not restart or suspend it.

4. **MAJOR — Logical completion, physical cleanup, and retained-result accounting still lack a consistent release rule.** [RFC:148](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:148), [RFC:198](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:198)

   D3 releases the byte charge at physical `end`; D5 retains an independent completion’s running charge until drain. Both obligations matter. A successful response may end physically before drain; a timeout may drain before the cancelled exchange ends. Neither event alone permits releasing the whole reservation.

   The same missing distinction affects redirects. Each hop is an exchange, but upload ownership and the ordered job span the fetch. `Flow::Skip` cannot release upload bytes still needed by the next hop. Nor may the next independent hop resume past the cap while the cancelled previous hop still occupies a physical slot.

   Write a release table covering success, timeout/forget before physical end, redirects, and failures before any HTTP exchange exists. Include a cancelled file reader: its request must settle while its reader permit and retained bytes remain accounted for. Define separately when the ordered blocking slot returns. The terminal-transition list is substantially improved; this accounting mechanism is still unfinished.

5. **MAJOR — The starter/callback model leaves completion progress and stream buffering unresolved.** [RFC:120](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:120), [RFC:218](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:218), [RFC:226](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:226)

   Serializing starts on a lane starter fixes invocation ordering and the handoff-return loop. Making that starter the only completion processor introduces another dependency: one long independent `Now` or opaque destructor delays every independent HTTP completion. That is not “exactly as” today’s six independent workers. The RFC also promises prompt timeout publication without specifying a publication path that remains available during that blockage.

   Streams make the omission a memory problem. Callbacks only append bytes; parsing belongs to the starter, described as processing *finished exchanges*. SSE need not finish. Per-event ceilings do not bound accumulated unparsed bytes, and coalescing after parsing does not bound a queue before parsing. Today’s [`read_events`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_stream.rs:235) parses incrementally and publishes into the coalescing slot.

   Specify the incremental stream consumer, bounded buffering/backpressure, and terminal publication while a starter is occupied. Reconcile synchronous `head → Flow` decisions with D8’s callback restrictions. Any accepted loss of progress during independent opaque work must be declared as a change.

6. **MAJOR — The measurement plan fixes the seventh-worker confound, but still permits misattribution.** [RFC:289](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:289), [RFC:299](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:299)

   **A7 really having seven workers is the correct repair.** Making C7/A7 decisive and giving Linux its own gate address the central round-2 measurement findings.

   However, always running the preconnect block after its C control confounds preconnect with run order and persistent DNS/TLS warmth. Falling DNS/TLS durations do not distinguish those causes. Isolating the block prevents contamination of other arms; it does not establish causality within this comparison. Require independently established equivalent initial conditions or a counterbalanced design with an explicit treatment of carryover.

   The connection evidence also needs an operational definition. `isReusedConnection` identifies reuse, not connection identity; several existing connections can all report reuse and the same protocol. Require observed overlapping `h2`/`h3` transactions and a defined connection-count measurement, using connection endpoints or controlled-server identifiers where appropriate. [Apple’s metric definition](https://developer.apple.com/documentation/foundation/urlsessiontasktransactionmetrics/isreusedconnection).

   Finally, define “C is not worse than C7 on the same rule” numerically and require equivalent successful request cohorts, so earlier refusals or missing reads cannot improve the primary outcome.

7. **MINOR — The WebSocket interface still cannot express the existing terminal taxonomy.** [RFC:90](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:90)

   `opened`, `text`, and `closed(code, reason)` omit a typed local failure. Today, binary messages and local size overflow produce **Refused**, whereas a peer close produces **Network**; a peer can itself send close code 1009. Those cases cannot safely be distinguished using only a close code and reason. See [`read_socket`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_stream.rs:125).

   Add the ceiling to the start contract and define typed error/refusal delivery, including failure before opening and abort. Round-2 Grok m8 is only partially resolved.

8. **MINOR — Thread reservation is concrete; thread creation and unused cost remain unspecified.** [RFC:163](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:163), [RFC:318](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:318)

   The arithmetic is correct: **3 Apple / 12 adapter permits per executor**, and **16 × 12 + 96 + 4 = 292**. Reserving ordered capacity at startup and retaining permits until thread exit fixes the central exhaustion/isolation finding.

   State whether reserved adapter threads are created eagerly or lazily and how partial OS thread-creation failure unwinds. Restore the requested unused-executor/no-network measurement; load-time peak threads and RSS do not measure that cost. Clarify that the 384 ceiling encompasses all named pools, including retiring threads.

The round-2 disposition is:

| Round-2 finding | r3 assessment |
|---|---|
| Astra M1 / Grok m7: handoff charges and floor | Resolved. |
| Astra M2: read/effect predicates | Resolved. |
| Astra M3 / Grok M3: thread reservations | Central mechanism resolved; finding 8 remains. |
| Astra M4 / Grok M1: start context and handoff loop | Dispatch mechanism resolved; completion progress needs finding 5. |
| Astra M5: terminal transitions and cleanup | Terminal policy resolved; release/publication remain incomplete. |
| Astra M6 / Grok M4: uploads and exchange counts | Cap membership is explicit; upload and lifetime mechanisms remain incomplete. |
| Astra M7 / Grok M5: experiments | A7 and Linux gates repaired; findings 6 and 8 remain. |
| Astra m8: SSE branch and pull consumers | Both explicit fixes resolved; asynchronous stream buffering remains unwritten. |
| Astra m9 / m10: header change and preservation wording | Requested corrections resolved. |
| Grok M2: deadline activation | Unresolved. |
| Grok m6: redirect skip and declared-length precedence | Resolved as outcome policy; cleanup still needs finding 4. |
| Grok m8 / n9: socket interface / session delegate | Socket partial; session delegate resolved. |

**§3 is therefore not yet true of D1–D8 as a whole.** The ordered dispatch/handoff rules, admission predicates, fence, re-ask machinery, settlement pumping, explicit independence, and grant/redirect policy are now compatible with preservation. Upload admission, deadline activation, completion progress, stream buffering, and release accounting still require the corrections above.

Read-only review; no files changed and no builds or tests run.

**Verdict: RIGHT WITH CHANGES.**
