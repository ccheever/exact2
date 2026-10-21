# Design review: LLP 1120 r1, 2026-10-09 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the draft's worktree.
- **Method:** one brief (sha256 `8c521d8f925507af87fb856756c47a6a42cc0169743ea29f2b1521aece1e5718`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r2 (§0, round 1).

---

The Apple async direction is sound, but r1’s claim that every semantic stays unchanged is false. Two proposed rules directly conflict with existing behavior. Several other parts need a concrete lifetime and accounting contract before implementation.

This was a read-only review; I changed no files and ran no builds or tests.

1. **BLOCKER — The global cap breaks ordered-capacity isolation.**  
   [RFC:85](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:85)

   With 64 small independent requests held open, the proposed cap prevents an ordered release request from starting. If those requests await that release, this deadlocks. Separate URLSession pools cannot fix a scheduler-level cap. Including streams in the cap also breaks their ability to open beside held independent replies.

   Reserve ordered execution capacity and preserve separate stream admission. Extend the existing [ordered-releaser test](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_tests.rs:136) to exhaust the **new** cap. The current six-request version would miss this.

2. **BLOCKER — “At most one ordered job in flight” changes handoff semantics.**  
   [RFC:52](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:52), [RFC:109](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:109)

   Today, `OwnedWork::Later` hands work to its owner and immediately frees the executor to start the next ordered job. Its outstanding reply still occupies the ordered settlement sequence. The [existing test](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_tests.rs:559) explicitly requires job 11 to execute before job 10 replies, while their results settle 10 then 11.

   Preserve FIFO **dispatch**, serial blocking execution, and ordered **settlement**, with outstanding handoffs excluded from the execution-busy flag. Do not implement this as “no ordered `Running` entry exists.”

   Also, D3’s “only [ordered] lane can carry” opaque work is incorrect: driver faults produce `Work::Now` for independent requests, and native calls can hand off independently.

3. **MAJOR — One stream session changes long-lived stream concurrency.**  
   [RFC:99](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:99)

   Six long-lived SSE connections to an HTTP/1.1 origin can occupy the proposed session indefinitely; stream seven then waits despite the 16-stream admission bound. Today each stream has its own transport. Apple documents the default six-connection limit as **per session**. [Apple connection-limit documentation](https://developer.apple.com/documentation/foundation/urlsessionconfiguration/httpmaximumconnectionsperhost?changes=_3%2C_3)

   Retain bounded per-stream sessions initially, or explicitly design and test sufficient stream connection capacity. A separate stream session protects ordinary HTTP, but does not preserve isolation among streams.

   WebSockets also need an async socket interface: D1’s HTTP `start` does not replace blocking `SocketTransport::connect`/`MessageSource::next`. When sharing sessions, change [socket destruction](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/darwin_websocket.mm:501), which currently invalidates the entire session. Closing one socket must not cancel its siblings.

4. **MAJOR — The asynchronous transport contract is underspecified, and starting it under the core lock is unsafe.**  
   [RFC:25](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:25), [RFC:51](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:51)

   “Returns immediately” does not prohibit synchronous callbacks. A pre-aborted signal invokes its registration callback synchronously today; an inline failure calling `complete` would reenter the held state mutex. Reserve and select work under the lock, then invoke transports, handoffs and cancellation outside it.

   Specify request ownership after `start` returns, callback threading, exactly-once terminal delivery, and cancellation for each redirect hop. `data → Stop` alone does not explain rejecting headers or abandoning a redirect before receiving its body.

   Preserve declared-length rejection, HEAD/null-body exceptions, incremental body limits and header caps. Record overflow as the terminal **Refused** reason before cancelling; the resulting platform cancellation must not turn it into **Aborted**.

5. **MAJOR — Cancellation, retirement and deadlines need distinct logical and physical lifetimes.**  
   [RFC:57](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:57), [RFC:62](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:62)

   `cancel()` returns immediately and callbacks can still arrive before cancellation is acknowledged. Removing a ticket is not sufficient protection for a Rust sink reached through Objective-C callbacks. [Apple cancellation documentation](https://developer.apple.com/documentation/foundation/urlsessiontask/cancel%28%29?language=_8)

   Specify an owned exchange state, one terminal-outcome transition, hop identity, retained cancellation registrations, and cleanup after native completion. Late callbacks must safely release resources and answer required completion handlers. Retirement must disable wakes before cancellation and keep arbitrary work destruction off the UI thread.

   For `exactTimeout`, preserve one monotonic deadline across file preparation and redirects, starting when execution begins rather than while waiting in the core queue. Define abort-versus-timeout precedence by which terminal event wins. Remove completed timer entries so a fast workload with hour-long timeouts cannot grow the heap indefinitely. Timer callbacks must not block the sole timer thread behind arbitrary work.

6. **MAJOR — `exactBodyFrom` contradicts both the two-thread story and the memory argument.**  
   [RFC:56](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:56), [RFC:79](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:79)

   File reads deliberately use [separate, process-bounded reader threads](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_body.rs:78). A stalled read can outlive its timed-out request, retaining its reader slot until it actually exits. Reading directly on the sole work thread would lose that protection and could indefinitely stall subsequent work.

   Keep the four-reader bound and deliver read completion asynchronously.

   More importantly, admission charges the file **path**, not the eventual upload payload. With small response ceilings, 64 slow uploads could retain roughly **4 GiB of file bodies**, before copies. The four-reader limit does not prevent this: readers finish while their uploads remain active. This amplifies an existing accounting gap. Reserve upload bytes through their actual lifetime, or introduce an explicit aggregate upload budget.

7. **MAJOR — D5 silently changes completed-result accounting and misstates the count windows.**  
   [RFC:79](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:79)

   [Current `complete`](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:853) charges ordered completions their retained size, but independent completions retain their **full running reservation until drained**. “A completed one [is charged] what it retains” changes that behavior. Either preserve it or identify and evaluate the change explicitly.

   Likewise, “16 + 128 reads + 128 markers” is not three additive pools. The 128 real-ticket window includes effects; the sixteen includes queued/running reads and effects, excluding finished reads and markers.

   Finally, 1–64 MiB is the annotation-validation range, not the admissible independent range: [reservation accounting](/Users/admin/x2p2/apps/exact2-http/host/apple/src/executor_core.rs:1005) rejects even a 32 MiB ceiling because twice its ceiling plus allowances exceeds 64 MiB.

8. **MAJOR — Deferring a Linux reactor is reasonable; the proposed adapter is not adequately bounded.**  
   [RFC:67](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:67)

   Response bytes are a poor bound for thread stacks, descriptors, connection state and thread creation. The RFC also alternates between 128 concurrent adapter threads and a 64-exchange cap.

   “Short-lived” is particularly unsafe as an assumption: the existing [system resolver cannot be interrupted](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:188). Counting only work/timer threads no longer bounds retiring HTTP threads across repeated executor replacement.

   Keep the existing bounded blocking owners behind the async interface initially, preserving ordered capacity and permits until threads exit. If Linux or render measurements justify a reactor, use a maintained HTTP implementation such as Tokio/hyper; do not write another HTTP/TLS reactor.

   Include the render host in this decision: it [defaults to rustls even on macOS](/Users/admin/x2p2/apps/exact2-http/host/render/src/executor.rs:26).

9. **MAJOR — The migration tests are necessary but insufficient.**  
   [RFC:122](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:122)

   Preserve behavior assertions while adapting fixtures to a controllable asynchronous transport. Existing blocking `open` fakes cannot establish callback-race safety.

   Add coverage for inline failure, abort-before-start, redirect cancellation with late old-hop events, overflow-versus-cancel classification, completion-versus-deadline races, and repeated retirement with retained resources. Exercise full-cap ordered release, asynchronous handoffs, 16 held same-origin HTTP/1.1 streams, sibling WebSocket cancellation, and stalled file reads.

   Include the JS shared-load/re-ask integration tests and Linux/render tests, not just `executor_*_tests.rs`. Run the existing ignored real-platform tests explicitly. Validate HTTP/2 multiplexing against a controlled endpoint; “exactly one connection for N requests” is too strong as a universal URLSession guarantee.

10. **MAJOR — The measurement plan cannot yet support its acceptance claim.**  
    [RFC:135](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:135)

    With 4 MiB ceilings and unchanged accounting, build B’s sixteen workers can run only **seven** requests concurrently. That is a useful configuration comparison, but not a six-versus-sixteen-concurrent-request experiment. Record actual active requests and byte-budget stalls.

    Add matched-concurrency comparisons to separate scheduling gains from shared-session gains. Measure HTTP/1.1 as well as HTTP/2, mixed response sizes, completion backlogs, uploads and cancellation churn. Include physical-device startup and the no-network boot path.

    Eight interleaved runs are a pilot. Define a primary outcome, numerical regression tolerances and uncertainty; “no worse on any metric” is not a workable statistical acceptance rule. Measure peak threads/memory during loading, not only after launch.

    A/B can report task metrics: the current [Darwin delegate already receives them](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:218). Instrument all variants consistently. Comparing two HTTP/1.1 Linux implementations also cannot determine whether Linux HTTP/2 would help.

11. **MINOR — Preconnect is honestly described as an experiment, but its claimed invariants are overstated.**  
    [RFC:91](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:91)

    Correct: the Network.framework connection does not enter URLSession’s pool. Cross-framework DNS/TLS reuse remains a hypothesis. `isReusedConnection` measures persistent-connection reuse, not TLS resumption. [Apple metrics documentation](https://developer.apple.com/documentation/foundation/urlsessiontasktransactionmetrics/isreusedconnection)

    Sending no HTTP request does not make the warm-up free of observable network activity or first-frame cost. “First grant” is also not a prediction of demand and may be a wildcard.

    Use a concrete consumer origin, bounded lifetime, cancellation and no first-frame dependency. Judge total startup benefit and total connection cost, not merely shorter DNS/TLS spans on the subsequent task. Keep the default off unless that experiment wins.

12. **MINOR — Round-robin by `target` does not establish fairness by source.**  
    [RFC:54](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:54)

    [`RequestOut::target`](/Users/admin/x2p2/apps/exact2-http/runner/src/request.rs:658) names a resource or mutation, not its source. The runner normally has one current ticket per target; a source feeding many resources has many targets. The stated “100 reads from one source” example therefore does not establish the benefit of this grouping.

    Reordering is permitted on this lane, but fairness also needs a rule for large reservations that repeatedly fail to fit. Keep FIFO for the migration and evaluate fairness separately.

13. **MINOR — “Two threads per executor” needs a narrower scope, and session construction must remain lazy.**  
    [RFC:63](/Users/admin/x2p2/apps/exact2-http/llp/1120-network-concurrency-without-threads.rfc.md:63)

    Two is the proposed number of persistent executor-owned Apple threads. Foundation/GCD workers, file readers and module owners remain. Linux retains HTTP/stream readers, and its [WebSocket transport also creates a writer thread](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/websocket.rs:279). The current 112 limit likewise bounds executor workers, not every thread.

    Storage, opaque closures, destructors, filesystem operations, Linux DNS and response processing still consume blocking or substantial CPU time. Keep those out of the state lock and serial delegate callbacks.

    Explicitly preserve [lazy session construction](/Users/admin/x2p2/apps/exact2-http/vendor/ibex/crates/ibex2/src/transport/darwin.rs:306); creating three sessions during executor startup could charge unused networking initialization to first pixel.

The remaining §3 commitments are compatible with an event-driven implementation, with these qualifications:

| Semantic | Assessment |
|---|---|
| Refusals and fence | Preserve retained refusal positions, the separate 128-request/64 MiB fence, and host-controlled lifting after earlier settlement obligations disappear. |
| Read queue and re-asks | Preserve the amendment’s exact counters, marker-only completions, deduplicated waiting records, and placement after fenced work. Waiting markers must not consume exchange slots. |
| Forget | Queued safe reads disappear; running safe reads abort; undrained outcomes disappear. **Already-admitted queued writes still execute**, unlike forgotten fenced work. D3’s “once begun” wording should say this explicitly. |
| Grants and redirects | The proposed ownership split is right. Retain `storage::scope` validation before constructing scoped grants, every-hop admission, guarded headers, method/body transformations and cross-origin credential stripping. WebSockets retain their separate grant and redirect refusal. |
| Settlement and independence | Keep one result/refusal per pump, lane alternation and coalesced wakes. GET alone must never select independent execution. |

Per-task delegates themselves are a valid Apple choice at these deployment targets. Assign them before resuming tasks, retain exchange state safely, and keep callbacks short. Shared sessions enable multiplexing; they do not promise one handshake or one connection under every network/server condition. [Apple task-delegate documentation](https://developer.apple.com/documentation/foundation/urlsessiontask/delegate?changes=__6)

For §6:

| Question | Recommendation |
|---|---|
| Round-robin or FIFO? | **FIFO initially.** Separate the fairness experiment and establish the correct grouping. |
| Linux adapter or reactor? | **Bounded blocking adapter initially**, retaining existing capacity isolation and retirement accounting. A mature reactor only on Linux/render evidence. |
| Request-free warm-up acceptable? | **Yes as a bounded measurement experiment**, with a concrete origin and end-to-end acceptance criteria. No general grant-list warming by default. |
| Sessions per executor or process? | **Per executor for ordered and independent HTTP.** Preserve stream connection capacity separately. Process sharing adds cross-executor contention and lifetime complexity without evidence here. |

**Verdict: RIGHT WITH CHANGES.**
