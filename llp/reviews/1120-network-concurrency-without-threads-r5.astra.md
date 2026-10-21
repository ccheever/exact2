# Design review: LLP 1120 r5, 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `ecb0631869531f61b1294be56ef90c883a97a73e77b9a2cc40fd32a3b52b201c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r6 (§0, round 5).

---

**The RFC is not ready for a ruling authorizing implementation as written.** Most round-4 fixes now have concrete mechanisms. Resource accounting, file-backed streams, and the measurement gates still need changes.

References marked “RFC” below refer to [llp/1120-network-concurrency-without-threads.rfc.md](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md).

1. **MAJOR — Draining a timeout can still release bytes that physical cleanup owns.** [RFC:285](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:285)

   The finisher now publishes before transport completion, but the failure row releases the independent response reservation at **drain alone**. Nothing specifies that the partially collected response and retained request buffers have been destroyed before publication. D3 instead says the cancelled task retains its resources until physical end.

   Following the table literally permits successive timed-out cohorts to drain and reuse the same byte budget while their cancelled exchanges retain buffers. The 32-exchange cap does not preserve the 64 MiB budget: 32 exchanges retaining 4 MiB each already hold 128 MiB. Ordered failures shrink to their retained outcome even earlier, without an independent-style exchange cap.

   Preserve the reservation until **both** the retained-result and physical-resource obligations end, or explicitly specify earlier destruction of those buffers and account for what remains. Test drain-before-cleanup and cleanup-before-drain. Also apply D2’s “last upload-buffer release, whichever is later” rule to failure, forget and retirement rows—not only success. **Round-4 Astra M5 remains partially open.**

2. **MAJOR — File-backed SSE streams do not have a consistent atomic-reservation rule.** [RFC:256](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:256), [RFC:264](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:264)

   Streams retain their response reservation at admission, while the new sizing phase requires only request buffers to be reserved. D7 expressly supports `exactBodyFrom` on SSE. The RFC does not reconcile these rules or specify how a sizing/byte-waiting stream relates to the ordinary lane queue.

   Retaining the admission reservations reproduces round 4’s cycle: two streams with 12 MiB ceilings reserve approximately 48.25 MiB together. Each carrying a 20 MiB upload fits individually, but neither upload fits the remaining 15.75 MiB. Streams have no request deadline to rescue them. Their existing admission charge comes from [`executor_core.rs:369`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:369).

   Specify an explicit file-backed-stream admission/sizing exception, including when response reservations return, retention of the stream count, and scheduling while awaiting the complete charge. Add the two-upload test for SSE as well as ordinary fetches. The ordinary-upload deadlock is resolved; the complete supported surface is not yet closed.

3. **MAJOR — Releasing the ordered logical slot leaves Linux’s next HTTP start undefined.** [RFC:175](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:175), [RFC:235](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:235)

   An ordered request can time out while its sole adapter thread remains stuck in the resolver. The finisher releases the ordered blocking slot, but that does not make the adapter available.

   The next HTTP request therefore needs another eligibility condition or an adapter queue. The RFC does not choose which, or state when that request’s deadline arms. Arming on adapter submission charges executor-capacity waiting; arming when the thread actually accepts work excludes it. URLSession’s declared post-`resume` policy does not settle this internal adapter question.

   Specify physical adapter availability separately from the logical ordered slot, including cancellation while waiting. Test a timed-out, blocked resolver followed by another ordered fetch and a handoff. Reserving the thread protects against **other executors**, but cannot make this executor’s occupied thread available.

4. **MAJOR — The Apple gate can approve a regression against the shipping executor.** [RFC:410](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:410)

   C7/A7 closes the seventh-worker attribution problem, and C/C7 evaluates the larger cap. Neither comparison requires C to perform acceptably against **today’s A6**.

   For example, these hypothetical median/worst times pass the written timing gates:

   | Arm | Median | Worst | IQR used by gate |
   |---|---:|---:|---:|
   | A6 | 600 ms | 700 ms | — |
   | A7 | 800 ms | 1,000 ms | 50 ms |
   | C7 | 700 ms | 900 ms | 20 ms |
   | C | 710 ms | 900 ms | — |

   Multiplexing evidence and successful completion of all reads can also hold. C still regresses against the shipped baseline.

   Keep the matched-concurrency mechanism gate, and add an explicit C-versus-A6 acceptance condition.

5. **MAJOR — Preconnect’s “verified cold” check measures the state after treatment.** [RFC:402](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:402)

   In C+P, the first independent task runs after preconnect has had an opportunity to warm DNS/TLS—the intended treatment. Its lookup duration therefore cannot establish the **initial** state. A positive lookup duration also does not prove an empty DNS cache; Apple defines it as the duration of the lookup operation, not a cache-cold indicator. [Apple metrics documentation](https://developer.apple.com/documentation/foundation/urlsessiontasktransactionmetrics).

   Restarting the experiment based on this post-treatment observation can reject the very warming behavior being tested. Rebooting every run and randomizing order are improvements, but this qualification rule does not establish equivalent starting conditions.

   Establish initial conditions independently of the treatment, or narrow the experiment’s claim and leave cold-cache benefit inconclusive. Explicitly extend the successful-cohort/failure rule to C+P and the Linux comparison; its current operative treatment list names only C7 and C.

6. **MINOR — D3 and D5 specify different upload deadlines.** [RFC:204](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:204), [RFC:268](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:268)

   D3 and §3 declare a fresh deadline **when the request waited for bytes**. D5 also gives the immediately fitting second phase “a fresh deadline.”

   With a 100 ms timeout, 90 ms of sizing and 20 ms of reading, those rules produce different outcomes even without a budget wait. Choose one rule, align the declaration, and specify invalidation of the sizing timer when disarming or rearming so an old timer cannot terminate the second phase.

7. **NIT — A test and two guarantees remain stale.** [RFC:375](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:375), [RFC:145](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:145), [RFC:345](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:345)

   The handoff-behind-blocking-job test still requires waiting for **physical end**, contradicting D3’s terminal-transition release. Change it to exercise terminal-before-cleanup explicitly.

   The one-second metrics retention fallback does not support “always applied” without a stated delivery guarantee. Describe the fallback conservatively. The Limits bullet should also name the broader platform-queue change, rather than only HTTP/2 stream limits.

The round-4 disposition is:

| Round-4 findings | R5 assessment |
|---|---|
| Astra B1 / Grok M1: upload reservation cycle and armed budget wait | Ordinary-fetch mechanism fixed; stream exception and deadline inconsistency remain above. |
| Astra M2 / Grok M3: platform queuing | Resolved: all post-`resume` platform waiting is explicitly charged to the deadline. |
| Astra M3 / Grok n12: publication and opaque destruction | Resolved mechanically: direct terminal events, a separate finisher, and a separate disposer. |
| Astra M4 / Grok M2: stalled reader holds ordered execution | Resolved by terminal-transition release while retaining the reader permit/upload charge. The adapter follow-on needs finding 3. |
| Astra M5 / Grok m5: retained failures and release table | Failure retention is corrected; the two independent release obligations remain incomplete. |
| Astra M6 / Grok M4: measurement | Main-arm failure exclusions fixed; preconnect qualification remains unsound. |
| Astra m7 / Grok m6: thread arithmetic, exhaustion and Linux gate | Resolved: 6/15 bases, atomic reservation, partial-spawn unwind, reader refusal, and the corrected thread formula. |
| Grok m7 / m8: upload copy and delegate queue | Concrete mechanisms supplied; upload-release rows still need alignment. |
| Grok m9: late metrics | Late delivery is addressed within the stated retention window; “always” overclaims it. |
| Astra n8 / Grok m10 / n11: signature, FIFO question, bounds | Resolved. |

**§3 is not yet true of D1–D8 as a whole.** The admission counters, read/re-ask windows, fence, settlement order, handoff eligibility, explicit independence, grants and redirect rules are supported. The starter/finisher separation and first-terminal-event policy are sound. The independent caps clearly count physical HTTP exchanges and exclude handoffs, streams and undrained completions.

The thread numbers also reconcile: sixteen Apple bases consume 96 permits; sixteen adapter bases consume 240. Streams, readers and surviving retired threads share the 384 limit, with explicit exhaustion behavior. Those numbers are a coherent policy choice, pending measurement.

The remaining implementation decisions are substantive: cleanup accounting, stream-upload reservation sequencing, adapter availability, and upload timer transitions. The gates also need the corrections above before they can decide whether to ship the change.

Read-only review of the RFC, all eight earlier reviews, LLP 1041 §8.4 and amendments, and the relevant executor/ibex2 sources. No files changed; no builds or runtime tests run. Counterexample arithmetic was evaluated directly.

**Verdict: RIGHT WITH CHANGES.**
