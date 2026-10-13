# Design review: LLP 1120 r7, 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `1b5cef594fb4f27a1efb4dd1185e5666df2c2d7b0af59a3383713f119a214994`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Taken in r7 without another round (§0, round 7): B1 as a proposed rule for the ruling (§6, question 8); m2 and n3 written in.

---

**R7 is not ready for an implementation-authorizing ruling unchanged.** The numbered round-6 findings now have substantive fixes, but one remaining upload-admission deadlock needs a policy correction before implementation. The other findings below can be settled in code review; they do not warrant another design round.

1. **BLOCKER — A parked upload can wait forever on buffers belonging to requests trapped behind it.** [RFC:218](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:218), [RFC:303](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:303), [RFC:313](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:313)

   Atomic acquisition fixes the earlier cycle between partial upload reservations. It does not guarantee that the head’s full charge will eventually become available.

   A concrete independent-lane sequence, allowing small request-record overhead:

   - An ordinary request X is running with a 1 MiB response ceiling, reserving approximately **2.125 MiB**.
   - Upload A has a **61 MiB file and 1 MiB response ceiling**. Its approximately **63.125 MiB** whole charge fits the 64 MiB lane, but not beside X. After sizing, A parks at the head and disarms its deadline.
   - Request B, a POST with a **3 MiB inline body**, is admitted behind A. Its waiting buffers fit admission and remain below the 4 MiB request cap.
   - X finishes physically and its outcome drains. B still occupies approximately 3 MiB, leaving less than 61 MiB available for A’s approximately 63.125 MiB charge.

   A cannot start. B cannot pass a head waiting for bytes. No running request or undrained outcome remains to release capacity, and A’s deadline is off. Every specified release wake can occur correctly without resolving this state.

   This follows the existing waiting-buffer admission and head-only selection rules in [executor_core.rs:389](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:389) and [executor_core.rs:799](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:799). Adding the file charge exposes it for uploads that previously could execute. It affects both transport paths.

   **Must change before implementation:** define a liveness rule that includes buffers held by blocked queued successors. The smallest policy preserving FIFO is to make an upload an execution-time `Refused` outcome when its whole charge plus those successor buffers exceeds the lane budget, rechecking after later admissions. Alternatively, protect sufficient capacity through admission or authorize another bypass rule. That choice changes observable behavior and belongs in Charlie’s ruling.

   Add this exact sequence to §4, with no timeout and with X completely drained and cleaned up. The existing two-upload test does not cover it.

2. **MINOR — Stream thread costs still use host names where the selected transport matters.** [RFC:257](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:257), [RFC:287](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:287)

   R7 correctly keys the **base set** to the HTTP transport. The stream paragraph still says “on Apple, none,” although the default render transport on macOS is rustls. Its SSE stream needs a blocking reader.

   WebSockets require a separate distinction: [`Host::with_transport`](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/host.rs:50) replaces HTTP while retaining the platform socket transport. Default macOS render therefore need not incur Linux’s two WebSocket threads.

   **Code-review correction:** charge actual executor-owned threads according to the HTTP and socket adapters selected. Preserve the stated process counter and admission policy. No new architectural ruling is needed.

3. **NIT — The counter-waiter paragraph contradicts the explicit stream-refusal policy.** [RFC:287](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:287), [RFC:289](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:289)

   Streams are explicitly refused when permits are unavailable, but the next paragraph names a stream as an example of a starter waiting for a permit.

   **Code-review correction:** remove that example. Ordered replacements wait; streams and readers refuse as their operative rules and tests require.

The round-6 disposition is:

| Round-6 finding | R7 assessment |
|---|---|
| Astra M1: cancellation of non-GET streams and retirement | **Resolved.** D3 explicitly cancels every stream, distinguishes ordinary writes, and aborts writes on retirement. Preparation and open-stream tests are named. |
| Astra M2: preservation of ordered adapter capacity | **Resolved.** A returning straggler retains the ordered role when vacant; otherwise it exits. Recovery without a queued successor is tested. |
| Astra M3 / Grok B1: final releases and wakes | **Resolved.** The finisher applies releases before waking the starter; buffer deallocation is an event; global permit returns notify registered starters. Each release is tested as the sole progress event. |
| Grok M2: forgotten adapter-waiting writes | **Resolved.** Safe reads are removed; admitted writes remain marked and execute. |
| Grok M3: Linux upload deadline | **Resolved.** Adapter acceptance precedes sizing, and the adapter is retained through preparation. Parking returns it and disarms the deadline. |
| Astra m4 / Grok m6: straggler bound and thread arithmetic | **Resolved.** Three ordered adapter threads maximum, replacement only below three, and up to two extras included in §5. Base reservations follow the selected HTTP transport. |
| Astra m5 / Grok m4: file-backed stream retention and test | **Resolved.** Refused interested streams retain their charge through drain and physical cleanup; the simultaneous-admission test correctly expects both refusals. |
| Grok m5: sizing-to-reading permit race | **Resolved.** The same reader and permit continue when the charge fits immediately. |
| Grok N7 / N8: reader terminal outcome and comparison-local failure | **Resolved.** Both are explicit. |

These are mechanisms, not merely entries in §0. Finding 1 above is a separate remaining liveness hole.

**The rest of the execution model is sufficiently specified.** The starter serializes invocations, preserves the reservation through synchronous handoff replies, and re-decides after handoffs return. The finisher publishes terminal outcomes independently of transport cleanup and opaque work; the disposer keeps arbitrary destruction off that path.

The per-origin limit is per lane, with six initially, protocol-based lifting and restoration, and the stated independent-lane bypass. Post-`resume` platform waiting explicitly consumes the deadline. That is an honest best-effort policy: Apple permits a session to use fewer HTTP/1.1 connections than configured. [Apple documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost?changes=_3).

Deadline arming is now coherent across ordinary Apple HTTP, file preparation, adapter acceptance, redirects, and byte parking. The request-lock winner supplies one terminal transition, including reader failure. Logical publication and physical cleanup are appropriately separate; cancellation can still deliver callbacks before acknowledgment. [Apple cancellation documentation](https://developer.apple.com/documentation/foundation/urlsessiontask/cancel%28%29?language=_8).

The release rules preserve independent success **and failure** reservations through both drain and cleanup, defer ordered shrinking until physical cleanup, and retain upload charges through the final buffer release. Redirect replay, body-dropping redirects, and termination without another outstanding hop have explicit treatment.

The **32/8 caps** count resumed independent HTTP exchanges through physical end, excluding streams, handoffs and undrained completions. The **6/15 base sets**, **96/240 for sixteen executors**, and **up to 32 extra ordered adapter threads** reconcile. Exhaustion behavior is defined, subject to findings 2–3’s implementation clarifications. The 384 bound concerns executor-owned threads, not Foundation’s workers.

**§3’s counters, fence, re-asks, settlement ordering, cancellation categories and explicit independence are supported by D1–D8.** Its preservation claim nevertheless cannot cover upload progress until finding 1 is addressed. The additional refusal or scheduling exception chosen to fix that must join its declared changes.

**§5 can now decide the proposed change for the stated workload.** C7/A7 removes the seventh-worker explanation; connection identity and overlapping transactions establish that multiplexing occurred; C/A6 prevents accepting a shipping-baseline regression. Failed treatment runs cannot disappear through exclusions or control-triggered reruns. Preconnect is separated, randomized and qualified without pretending lookup duration proves an empty DNS cache. Linux has its own timing, memory, thread and render gates.

Those gates support an end-to-end win with the intended mechanism observed. They do not establish how much of the win comes from multiplexing versus other transport changes, or whether HTTP/2 would benefit Linux. Correctness remains a separate prerequisite: a fast Bluesky load cannot validate the upload liveness case.

Apart from finding 1’s policy decision, ownership details, atomic waiter registration, thread-creation failure cleanup and callback race handling can be settled and tested in code review.

Read-only review of the RFC, all twelve earlier reviews, LLP 1041 §8.4 and amendments, and the executor/fetch/transport sources. No files changed; no builds or runtime tests run. The counterexample’s charge arithmetic was checked directly.

**Verdict: RIGHT WITH CHANGES.**
