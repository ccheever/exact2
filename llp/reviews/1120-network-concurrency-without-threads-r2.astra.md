# Design review: LLP 1120 r2, 2026-10-09 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `587886bf89905233caa9b2e23806f4d64c58092725f16365fa016939237d97fb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r3 (§0, round 2).

---

r2 fixes several central round-1 mistakes, but **the claim that D1–D8 preserve all of §3 is still false as written**. The direction is sound; the following changes are needed.

This was a read-only review. The two cited round-1 review files are absent from this checkout, so the finding-by-finding audit below uses §0’s record.

1. **MAJOR — The stated running charge breaks handoffs again.** [RFC:141](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:141)

   Today, a `Later` handoff reserves **no response growth**: [`next_job`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:803) explicitly charges zero additional bytes. D5 instead charges every running job twice its response ceiling. Three outstanding ordered handoffs would consequently reserve over 384 MiB, preventing a fourth handoff—or an ordinary ordered HTTP request—from starting. Freeing the execution slot in D3 does not fix this byte-budget obstruction.

   State the handoff exception explicitly. Also preserve the `max(2 × ceiling, 32 KiB)` floor for other jobs. LLP 1041 separately acknowledges that handed-off outcomes can exceed the aggregate budget; that is another existing exception missing from §7. See [LLP 1041:689](/Users/admin/x2p2/apps/exact2-http/llp/1041-graceful-overload.rfc.md:689).

2. **MAJOR — The sixteen-ticket rule is still misstated.** [RFC:145](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:145)

   “Sixteen may be effects or unfinished reads” describes a cap that the amendment deliberately removed for reads. The sixteen is a predicate **when admitting an effect**. Plain ordered reads may continue entering the 128-real-ticket window even when sixteen unfinished reads already exist.

   The distinction is explicit in [`admit`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:379) and exercised by the [seventeenth-read test](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_again_tests.rs:81). Spell out the separate admission predicates. Otherwise implementing D5 literally contradicts §3’s read-queue promise.

3. **MAJOR — The process thread cap is not sufficiently defined to preserve lane isolation.** [RFC:125](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:125)

   The proposed counts are:

   | Executor configuration | Exact-owned threads |
   |---|---:|
   | Apple, excluding file readers and platform-managed threads | 3 |
   | Blocking adapter, fully populated, without streams | 36 |
   | Adapter plus sixteen SSE streams | 52 |
   | Adapter plus sixteen WebSocket streams and their writers | 68 |

   Thus sixteen fully provisioned adapter executors could mean **1,088 threads**, plus four file readers per process. This is a substantial policy decision, not merely restating today’s 112-worker limit.

   Specify the numeric cap, eager versus lazy pool creation, reservation policy, and exhaustion behavior. With lazy allocation against one shared cap, independent exchanges or streams can consume the permit needed to start an ordered release—reintroducing round 1’s coupling through a different resource.

   Reserve ordered progress capacity before admitting competing work. Explicitly charge the WebSocket writer until it exits; it currently spawns inside [the transport](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/websocket.rs:279). Platform-managed URLSession threads need separate measurement rather than being represented by the three-thread count.

4. **MAJOR — Releasing the state lock does not establish a safe, ordered execution context.** [RFC:88](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:88)

   Admission, drain, and completion can initiate starts from different contexts. D3 then directly lists `task.resume()` and “a handoff call,” while only `Now` work has an assigned thread.

   This leaves two concrete hazards:

   - Lazy session creation, request preparation, or an opaque `Later` closure can run on the UI thread or a delegate callback. Today the handoff executes on [a worker under panic containment](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1074).
   - Popping jobs FIFO under the lock does not ensure their out-of-lock invocations occur FIFO. A selected `Later` must retain a start reservation until its invocation returns, including when its `Reply` completes synchronously.

   Name the execution context and serialization mechanism for starts, handoffs, completion construction, file-root initialization, and disposal. Reconcile D2’s “parsing … on completion” with D8’s prohibition on callback parsing. The two work threads must also not become an unspecified place where arbitrary work blocks all HTTP completion processing.

5. **MAJOR — Logical completion and transport cleanup need separate, explicit rules.** [RFC:53](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:53), [RFC:109](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:109), [RFC:146](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:146)

   D1 makes each redirect hop an exchange, D3 calls an HTTP exchange a blocking job, and D5 caps exchanges. Specify that the **ordered job’s reservation spans the entire fetch**, including file preparation and every redirect. An intermediate hop ending must not admit the next ordered job.

   Also specify when the 32 permits and byte charges return after timeout, overflow, forget, and retirement. A logical terminal outcome can precede URLSession cancellation completion or a blocked resolver’s return. Releasing everything at logical completion defeats the physical-resource bound under churn; waiting for physical completion to publish every outcome defeats prompt deadline settlement. “Ignore except for releasing resources” does not resolve those distinct obligations.

   The claimed outcome precedence also needs correction. [Current code](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1273) accepts an `Ok` result first; on an error, a fired deadline wins. Caller cancellation prevents the deadline from winning when it precedes that deadline. Define terminal transitions for both event orders rather than an unconditional abort/timeout/overflow ranking.

6. **MAJOR — The acknowledged upload gap becomes materially larger.** [RFC:260](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:260)

   Four reader threads bound simultaneous **reads**, not bodies retained by HTTP requests after reading finishes. With small response ceilings, the new independent cap permits **32 × 64 MiB = 2 GiB of upload payload**, before copies and allocation overhead. Today the six independent workers bound that portion to 384 MiB.

   Recording this as an existing gap does not account for removing its previous concurrency bound. Either reserve upload bytes before increasing concurrency, preserve an equivalent bound for this path, or explicitly accept and measure the enlarged envelope. The four-reader mechanism alone does not make this safe on a phone.

7. **MAJOR — The matched-concurrency experiment is not matched, and the Linux arm omits its central cost.** [RFC:227](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:227), [RFC:249](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:249)

   Giving A an in-flight cap of seven still leaves it with six workers. C7 can execute seven requests; A7 cannot. Either use seven workers in A7 or compare A6/C6, and verify actual concurrency from the recorded timeline.

   The LTE primary outcome, interleaving, protocol metrics, controlled HTTP/2 endpoint, and cancel churn are useful improvements. But “request and response marks only” cannot decide whether the Linux adapter’s increased thread capacity is worthwhile. Include Linux thread/RSS measurements, unused-executor cost, warm pooled-render behavior, and tiny-ceiling saturation at 32. Define its acceptance criterion separately.

   Define “cold” precisely, particularly for C+P: process-cold runs do not necessarily reset system DNS/TLS state, and preconnect can affect subsequent interleaved runs.

8. **MINOR — Complete the stream and remaining-consumer contract.** [RFC:161](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:161), [RFC:66](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:66)

   D7 should explicitly preserve the existing branch: only a successful `text/event-stream` response enters the event parser; other responses produce one ordinary, body-bounded outcome. A 404 JSON response must not become an empty event stream. That behavior exists in [`executor_stream::open`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_stream.rs:191).

   If deleting the blocking form includes ibex’s generic fetch consumers, explain bounded delivery to demand-driven readers. [`read_response`](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/task.rs:841) currently pulls chunks. A push interface with only continue/stop needs an explicit buffering/backpressure arrangement; blocking the shared delegate queue would stall sibling requests. Otherwise narrow the migration’s scope.

9. **MINOR — Checking headers on every hop is a semantic change.** [RFC:48](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:48)

   Today the executor applies `check_headers` **after** `fetch_stream` has followed redirects ([executor_core.rs:1268](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1268)). D1 checks every hop. A redirect with more than 1,024 headers that previously led to an acceptable final response now fails.

   This may be a desirable change, but declare it and test it. Also retain or explicitly change the failure classification: today excessive headers become `Network`, whereas body overflow becomes `Refused`.

10. **MINOR — Several literal preservation statements overclaim.** [RFC:185](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:185), [RFC:191](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:191), [RFC:206](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:206)

    Retirement cannot abort arbitrary executing closures or retract handed-off work; it aborts cancellable operations and suppresses delivery. Independence is selected by explicit scheduling metadata, including Rust constructors and native-call paths—not exclusively the TypeScript `exactIndependentHttp` property.

    “Every assertion is kept” also needs qualification: [the existing overlap test](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_tests.rs:210) explicitly asserts six concurrent requests. Preserve semantic assertions while replacing assertions about the mechanism being removed.

The §0 audit is therefore:

| Round-1 finding | r2 assessment |
|---|---|
| Ordered start rule | Correct rule stated; charges and start serialization remain unresolved (#1, #4). |
| Global in-flight cap | Fixed at the exchange-cap level; process permits can recreate coupling (#3). |
| Redirect task delegates | Resolved: each task explicitly refuses platform redirects. |
| Stop at headers | Interface supports it; terminal/cleanup accounting remains incomplete (#5). |
| Opaque work on both lanes | `Now` routing fixed; handoff execution context incomplete (#4). |
| Bounded file readers | Retained correctly; enlarged upload retention remains unaddressed (#6). |
| Separate stream sessions | Correct isolation and WebSocket direction; stream contract needs completion (#8). |
| Callback safety, laziness, lifetimes | Improved substantially, but not mechanically closed (#4, #5). |
| Accounting | Independent undrained charges and approximate 32 MiB ceiling corrected; other errors remain (#1, #2). |
| Linux | Shared transport leasing is sound; process reservation and measurement incomplete (#3, #7). |
| Redirect rules and chain deadline | Redirect transformations match current code; deadline terminal rules need precision (#5). |
| FIFO fairness | Resolved as a policy choice. |
| Preconnect | Coherent as a default-off experiment; cold-state controls need definition (#7). |
| Measurement | Much improved, but not sufficient as currently written (#7). |

The Apple session separation is well founded: Apple documents the HTTP/1.1 connection limit as per session, with a default of six. [Apple documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost?changes=_3)

For §3, the preserved `State` machinery supports the pump limit, lane alternation, coalesced wakes, `counts[0] == 0`, refusal positions and fence, marker deduplication and placement, ordinary request cap, and queued-write treatment. D1–D2 support per-hop grants and incremental response limits. The ordered/read-admission claims fail against D5, while deadline, cancellation, and retirement preservation still depend on unspecified transitions. **Thirty-two is a reasonable experimental independent-task ceiling, but its acquisition/release rules and interaction with physical resources must be settled before it constitutes a bound.**

**Verdict: RIGHT WITH CHANGES.**
