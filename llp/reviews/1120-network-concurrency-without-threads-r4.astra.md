# Design review: LLP 1120 r4, 2026-10-10 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `51be49ee86874687d41bb43ca4607498728c0eaaedfcd5c6e7e0f1bd476daddc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r5 (§0, round 4).

---

**The RFC is not ready for a ruling authorizing implementation as written.** R4 resolves several round-3 findings, but introduces an upload reservation deadlock and changes cancellation recovery. Completion delivery and release accounting still need explicit mechanisms.

Locations marked “RFC” refer to [llp/1120-network-concurrency-without-threads.rfc.md](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md).

1. **BLOCKER — Individually admissible uploads can deadlock each other.** [RFC:136](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:136), [RFC:225](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:225)

   D3 reserves response space before starting each reader. D5 then adds the file charge, making the reader wait if that addition cannot fit.

   Consider two independent uploads, each with a **12 MiB response ceiling and a 20 MiB file**. Ignoring their small request buffers, each response reservation is 24.125 MiB; each complete reservation is 44.125 MiB and therefore individually admissible. Both response reservations can be acquired before either reader reports its size. That leaves **15.75 MiB free**, so neither reader can acquire its additional 20 MiB. Neither has an HTTP exchange that could finish and release space. With no `exactTimeout`, both wait forever.

   The impossible-upload refusal fixes the previous round’s example, but not this new cycle. Specify a metadata phase followed by **atomic acquisition of response plus upload space**, or release provisional response reservations before waiting. Add a test with two gated metadata reads and no deadlines.

2. **MAJOR — Raising URLSession’s configured maximum does not resolve transport queuing.** [RFC:160](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:160)

   The per-lane origin counter, bypass of an origin-limited head, fallback to six, and continuing deadline across redirects are concrete improvements. However, “URLSession therefore never queues an HTTP/1.1 request below the core’s own limits” remains false. Apple explicitly permits a session to use a lower connection limit than configured. Setting 32 does not guarantee six—or 32—available connections. [Apple documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost?changes=_3).

   Astra’s round-3 M3 is consequently still open. The straightforward implementable ruling is that **all platform queuing after `resume` counts against `exactTimeout`**, including HTTP/1.1 queuing. Declare that broader change instead of limiting the exception to HTTP/2 stream capacity.

3. **MAJOR — The finisher still lacks a guaranteed path for prompt terminal publication.** [RFC:142](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:142), [RFC:171](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:171), [RFC:281](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:281)

   The specified finisher input is a *finished exchange*. The timer only flags and aborts. A reader blocked in filesystem work, a reader waiting for its upload reservation, or a Linux exchange blocked in the resolver need not produce that input. The resolver expressly cannot be interrupted: [rustls_http.rs:188](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:188).

   Also, the finisher disposes of opaque work. Dropping closure captures can run arbitrary, blocking destructors. Moving destruction onto the sole completion thread recreates the completion-progress problem even though `Now` runs elsewhere.

   Specify a bounded terminal notification that wakes the finisher independently of physical completion, records the winning event before delayed classification, and publishes exactly once. Keep opaque destruction off that publication path. Test a transport that withholds `end`, a blocked reader, and a blocking destructor while another request times out.

4. **MAJOR — Holding the ordered slot until a cancelled reader exits is an undeclared regression.** [RFC:151](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:151), [RFC:247](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:247)

   Today, `body::acquire` returns when cancellation or the deadline wins, while the reader retains its own permit until exit. The executor worker can then process subsequent ordered work. See [executor_body.rs:146](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_body.rs:146) and [executor_core.rs:1255](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1255).

   R4 instead makes a permanently stalled reader permanently block subsequent ordered HTTP, storage and handoffs—even after publishing its timeout. That does not preserve the existing recovery behavior. Apple cancellation also currently wakes the blocking consumer before `didComplete`: [darwin_http.mm:436](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:436).

   Preserve separate **execution completion** and **resource cleanup** lifetimes, retaining reader permits and upload charges through cleanup. Alternatively, explicitly request this stronger blocking policy as a semantic change. The handoff eligibility fix itself is correct; equating every cleanup lifetime with the old worker’s lifetime is not.

5. **MAJOR — The release table contradicts retained-failure accounting and omits terminal paths.** [RFC:232](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:232), [RFC:239](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:239)

   Today, independent completions retain their running charge until drain **for failures as well as successful responses**: [executor_core.rs:853](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:853). The table instead releases timeout/overflow charges at physical end and pre-exchange failure charges at publication. “Nothing is retained” is also inaccurate: an interested ticket retains its failure message and completion record.

   Success and redirect accounting are substantially improved, but ordinary transport errors, oversized-header failures and redirect-policy/admission failures still lack complete rows.

   Distinguish forgotten results from retained failures. Define accounting for both orders of drain and physical completion, preserving the independent reservation until both obligations end—or declare and evaluate a changed policy. Cover every terminal reason, including failures between redirect hops.

6. **MAJOR — The measurement gates still permit ambiguous attribution.** [RFC:355](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:355), [RFC:357](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:357), [RFC:368](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:368)

   The real A7/C7 comparison and overlapping transactions identified by connection endpoints are good repairs. The seventh-worker confound is closed.

   Two problems remain. Repeated `C, C+P, C+P, C` blocks balance run position, but do not establish equal DNS/TLS starting conditions: the final C follows two preconnect runs. Rebooting *between* blocks does not remove carryover *within* them. C+P also appears in the general interleaved-arm list. Specify a separate preconnect experiment with explicit treatment of carryover and verified initial conditions; do not assert that rebooting establishes equality.

   Separately, “every run … completes all 19” conflicts with excluding failed runs. Excluding slower treatment runs that time out can improve both the median and worst successful time. Treatment-caused failures must fail or render the gate inconclusive, rather than disappear from its denominator. Define exclusions and replacement runs in advance.

7. **MINOR — The Linux thread gate does not match D4’s permitted population.** [RFC:389](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:389)

   One executor with one file reader and no streams legitimately owns **16 threads**, but “reservation plus streams in use” allows only 15. A WebSocket contributes two threads, not one stream-sized increment.

   Express the bound using base threads, SSE threads, both WebSocket threads, file readers and surviving retired threads, all under 384. Also give “warm pooled-render page time not worse” an operational statistic and tolerance.

8. **NIT — Two signatures of the ruling are stale.** [RFC:107](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:107), [RFC:398](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:398)

   Add the ceiling argument to `start_socket`, as D7 requires. Change the FIFO ruling question to include D3’s explicit origin-limit exception.

The round-3 disposition is:

| Round-3 finding | R4 assessment |
|---|---|
| Astra B1 / Grok B2: impossible upload | Resolved for an individually impossible charge; new reservation cycle in finding 1. |
| Grok B1: handoff behind blocking work | Eligibility fixed; cleanup lifetime introduces finding 4. |
| Astra M2: upload preflight, growth, replay | Reader-side metadata, bounded growth and shared replay buffer are specified; reservation sequencing remains unsafe. |
| Astra M3 / Grok M5: deadline activation and fallback | Redirect deadline and fallback rules fixed; platform-queue guarantee remains false. |
| Grok M4: per-lane origin isolation and bypass | Resolved. |
| Astra M4 / Grok M3: release accounting | Success and redirect rules improved; findings 4–5 remain. |
| Astra M5 / Grok M6 and m7: completion progress, callbacks, streams | Synchronous header decisions, starter-owned starts and incremental SSE framing resolved; finisher progress remains incomplete. |
| Astra M6 / Grok m8: measurement | Matched concurrency, connection identity, numerical Apple comparison and Linux RSS/worst-case clauses resolved; findings 6–7 remain. |
| Astra m7: WebSocket taxonomy | Resolved substantively. |
| Astra m8 / Grok m9: thread reservations and exhaustion | Resolved: one counter, eager 5/15-thread bases, partial-failure unwind, retained permits, idle measurement and declared stream refusal. |
| Grok n10: timer ownership | Classification ownership resolved; the notification mechanism is still missing. |

**§3 is not yet true of D1–D8 as a whole.** Its admission predicates, settlement order, fence, re-ask placement, explicit independence and grant/redirect rules are supported. The 32/8 caps now clearly count resumed independent HTTP exchanges, exclude handoffs and streams, and retain slots through hop cleanup. D4’s reservation arithmetic is consistent. The remaining failures concern upload progress, timeout publication, ordered recovery and retained-result accounting.

Read-only review; no files changed and no builds or runtime tests run. I checked the upload counterexample’s reservation arithmetic directly.

**Verdict: RIGHT WITH CHANGES.**
