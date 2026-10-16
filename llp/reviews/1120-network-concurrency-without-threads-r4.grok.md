# Design review: LLP 1120 r4, 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `51be49ee86874687d41bb43ca4607498728c0eaaedfcd5c6e7e0f1bd476daddc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r5 (§0, round 4).

---

## Round 3

| Finding | In r4 |
|---|---|
| Grok B1 — ordered handoff during a blocking job | Resolved. A handoff runs only while the blocking slot is free; the slot lasts until that job's physical end; a returned handoff does not hold it; the starter re-decides before parking. The three §4 tests match the three orders. |
| Both B — an upload that can never fit waits forever | Resolved as a failure. After `stat`, a charge above the lane budget is `Refused` at once and does not raise the fence. |
| Astra M2 — upload preflight | Resolved for where `stat` runs, for growth past the charged size, and for one executor buffer across 307/308. The budget wait still sits inside the deadline and the reader permit. |
| Astra M3 / Grok M4 / Grok M5 — deadlines and the per-origin limit | Resolved for per-lane counting, passing over a head at its origin's limit, restoring 6 after `http/1.1`, keeping already-resumed deadlines, and not restarting the deadline across hops. The HTTP/2 stream-limit wait is declared. "URLSession never queues" is still false. |
| Both M — release table | The table itself resolves success, cancel, redirect, pre-exchange failure, and a cancelled reader. One earlier sentence still describes the independent completion charge as the whole running charge. |
| Astra M5 / Grok M6 / Grok m7 — completions, `head`, streams | Resolved. A finisher publishes while the starter is in `Now`; `head` returns the disposition on the callback; SSE is framed there into the coalescing slot; the starter resumes streams. The long-`Now` effect is a declared change. |
| Astra M6 / Grok m8 — gates | C7 against a real A7, connection identity, a numeric "not worse", Linux worst-case, an RSS cap, and readers in the thread count are in. Preconnect's block order and the exclusion rule can still bless the wrong cause. |
| Astra m7 — WebSocket taxonomy | Resolved. Ceiling on start; binary and local overflow are `Refused`; a peer close, including 1009, is `Network`; abort and pre-open failure are separate. |
| Astra m8 / Grok m9 — one thread counter | Resolved as one counter, eager base threads, idle cost measured, and a declared process-wide stream refusal. The Linux total does not match its rows, and failed creation does not return every permit. |
| Grok n10 — timer | Resolved. The timer sets the flag and aborts; the finisher classifies. |

## Findings

**1. MAJOR — An upload that fits the budget but not the free bytes waits with the deadline armed and a reader permit held.** `llp/1120-network-concurrency-without-threads.rfc.md:169`, `:226-229`. `host/apple/src/executor_core.rs:799-809`, `:1231-1238`. `host/apple/src/executor_body.rs:11-18`, `:107-130`.

The starter moves the job to running and starts the reader before the file size is known. The reader stats under its permit and the deadline, then posts the size. If the whole charge fits the lane budget but not the bytes free now, D5 leaves that reader waiting, permit and deadline still running.

`next_job` today will not start a job until its reservation fits, and `Deadline::arm` runs only after that, inside `execute`. Time spent waiting for bytes is outside `exactTimeout`. `MAX_READERS` is 4 for the process and is held only while a read is actually running. Four uploads parked on a full lane now occupy all four permits, so the next `exactBodyFrom` is refused as a stalled disk while nobody is in the filesystem. Because the job has already left the queue, later work can take the free bytes it is waiting on and the request times out without being sent.

This is not in §3's declared changes. After `stat`, a charge that fits the budget but not the free bytes should go back to the head, release the reader permit, and stay unarmed until the full charge is taken. Arm when the read or the HTTP task actually starts.

**2. MAJOR — The ordered blocking slot stays taken until the reader thread exits, so a stalled read freezes the ordered lane after the timeout is published.** `llp/1120-network-concurrency-without-threads.rfc.md:150-151`, `:247`. `host/apple/src/executor_body.rs:12-18`, `:141-152`.

D3 holds the slot until physical end, and the release row for a reader cancelled by the deadline releases that slot at the reader's exit. `acquire` today returns the timeout to the worker as soon as the deadline is visible. The worker settles, loops, and takes the next ordered job. The reader is told to stop, keeps its permit until the thread leaves the syscall, and its late bytes are dropped.

`stat` and a 1 MiB read are not interrupted. Under the new row, that syscall keeps the only ordered slot after the caller has already been given `Timeout`, so the next ordered fetch, handoff, and storage step wait on a disk the outcome no longer depends on. §3 states "until its physical end" as preservation of today's one blocking job. For HTTP that is the transport's end, and that part is right. For `exactBodyFrom` it is a change, and it is not in the declared list.

Publish the timeout and free the ordered slot when the request settles. Keep the reader permit and the upload charge until the reader exits; the table already does that for those two.

**3. MAJOR — The deadline rule says URLSession never queues an HTTP/1.1 request below the core's limits. The platform does not offer that guarantee.** `llp/1120-network-concurrency-without-threads.rfc.md:160-166`.

Setting the independent session's `HTTPMaximumConnectionsPerHost` to 32, and resuming at most 6 HTTP/1.1 exchanges per origin per lane, does close the old bug where the core resumed more tasks than the session's default of 6. Apple's documentation for that property says a session may use a lower limit than the one configured, depending on the connection to the Internet, and that HTTP/2 and later ignore the property. The core has already called `resume` on those tasks, which is when D3 arms `exactTimeout`. Any task the system holds back still burns the deadline.

The HTTP/2 peer stream limit is handled honestly: the wait is a declared change and counts against `exactTimeout`. The system-lowered HTTP/1.1 limit needs the same sentence. "Never queues" should go. Round-3 Astra M3 asked for an activation signal or an explicit ruling that platform queuing is inside the deadline. r4 gives the ruling only for the peer's stream limit. The §5 campaign runs under the link conditioner's LTE profile, which is the kind of path the lower-limit sentence is about.

**4. MAJOR — Two gates can accept the wrong cause.** `llp/1120-network-concurrency-without-threads.rfc.md:354-355`, `:357-368`.

The session-change comparison itself is in good shape: C7 against an A7 built with seven independent workers, median by more than A7's IQR, worst not worse, overlapping transactions on one connection per origin identified by local and remote endpoints, and the same shape for Linux with a worst-case clause and an 8 MiB RSS cap.

Preconnect is not. Every block is `C, C+P, C+P, C`, and the reboot sits in front of `C`. Cold runs do not flush system DNS. `C+P` therefore never starts from the rebooted cache, and half of `C` does. D6's acceptance requires `domainLookup` and `secureConnection` to fall. A warm DNS cache produces that fall. The same paragraph says the arms are one interleaved set of five and that `C`/`C+P` are a separate rebooted block design, so a measurer also has to invent whether a `C+P` run or a reboot lands inside the C7/A7 sequence.

The cohort rule disagrees with itself. It requires every run in every arm to finish all 19 feed reads, then says a run with a refusal or a missing read is excluded and not counted as fast. Under LTE the runs that fail are the slow ones. Dropping them moves the median of whoever is left. One excluded run should fail that arm's gate.

**5. MINOR — The completion sentence and the release table account the upload differently.** `llp/1120-network-concurrency-without-threads.rfc.md:232`, `:243`. `host/apple/src/executor_core.rs:853-857`.

The table releases upload bytes at physical end on success and keeps the independent response charge until `drain`. The charge paragraph says an independent completion keeps its running charge until drained, which is what `complete` does today with `run.charge`. Once the file is part of that charge, following the paragraph holds the upload until `drain` and following the table releases it at physical end. The table is the one that keeps the response budget. Say the retained completion charge is the running charge minus the upload bytes.

**6. MINOR — The Linux base reservation does not match its rows, and creation failure does not return every permit.** `llp/1120-network-concurrency-without-threads.rfc.md:193-206`. `host/apple/src/executor_core.rs:241-247`, `:276-284`, `:716-720`.

Starters 2, finishers 2, timer 1, ordered adapter 1, and independent adapters 8 sum to 14. The total cell says 15, and sixteen executors are then "240". Reserving 15 and creating the rows leaks one permit per executor for the life of the process. On a failed `spawn`, permits return as each created thread exits. Permits taken for threads that were never created have no exit. That is also how `LIVE_WORKERS` plus `WorkerSlot` behaves today; this RFC is the place that was supposed to close it. Readers take a permit from the same counter and the text never says what a reader does when the counter is exhausted while the limit of 4 is not.

**7. MINOR — The charged upload buffer is not the buffer URLSession will hold.** `llp/1120-network-concurrency-without-threads.rfc.md:231`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:347`.

One executor buffer shared across 307/308 hops does close an uncharged replay copy. The transport being replaced sets `HTTPBody` with `dataWithBytes:length:`, which copies for the life of the task. That second buffer is the size of the file and is outside D5's charge. The task has to borrow the charged buffer, or the charge has to include the copy the session actually retains.

**8. MINOR — One lane session means one delegate queue for every exchange on that lane.** `llp/1120-network-concurrency-without-threads.rfc.md:116`, `:274-281`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:270-275`, `:188-189`.

`new_session` passes `delegateQueue: nil`, and that is a serial queue. Today a session is leased to one response, so a callback stall is that response's. A lane session shares the queue. D8 allows the callback to append up to the ceiling and to frame SSE. The current `didReceiveData` also waits on a 64 KiB handoff buffer on that queue. Copied forward, that wait holds every other independent `didComplete`. Callbacks have to return without waiting, and the queue choice has to be written down.

**9. MINOR — The protocol report can be dropped on the way out of `didComplete`.** `llp/1120-network-concurrency-without-threads.rfc.md:123`, `:163`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:208-224`.

The origin limit moves when a finished exchange reports `networkProtocolName`. The current callback removes the exchange from its map before metrics are delivered, and metrics are often delivered after `didComplete`. A lookup that misses leaves the origin at 6 permanently, so the HTTP/2 lift never happens. The report has to be applied when metrics follow completion. Names other than `h2`, `h3`, and `http/1.1` need a stated side; leaving them on the limit of 6 is the safe one.

**10. MINOR — A yes to §6 question 1 contradicts the mechanism.** `llp/1120-network-concurrency-without-threads.rfc.md:157-158`, `:309`, `:398`.

Question 1 asks whether independent-lane order stays FIFO. D3 and the declared-change list already pass over a head whose origin is at its limit so later origins start. The question to rule is that exception.

**11. NIT — The prose bounds are a little larger than `reservation`.** `llp/1120-network-concurrency-without-threads.rfc.md:228`, `:233`. `host/apple/src/executor_core.rs:1005-1012`.

"About 64 MiB less 2× the ceiling" and "about 32 MiB" omit the request buffers and the `2 × 64 KiB` header allowance. A 32 MiB independent ceiling does not fit, and `reservation` already refuses a limit the lane can never hold. The check that matters is the whole charge against `BYTES[lane]`.

**12. NIT — Opaque drops moved onto the only completion thread.** `llp/1120-network-concurrency-without-threads.rfc.md:145`, `:284`.

The finisher is what keeps a long `Now` from delaying timeouts. It is also where forgotten and retired opaque work is destroyed. A slow destructor there delays the lane's completions. That is the same concentration the finisher was added to avoid, and it is not in the declared list.

## What holds

Ordered dispatch matches the worker loop: one blocking job, a handoff only while that slot is free, FIFO settlement through `ordered`, one completion per pump, lane alternation, and `counts[0] == 0` as idle. D5's predicates match `admit` (`executor_core.rs:379-386`), including reads past sixteen unfinished ones and the separate re-ask window. Forget, the fence, marker placement, grants, and redirect policy match `fetch.rs` and §3. The in-flight cap is resumed independent HTTP exchanges, 32 and 8, released at physical end; handoffs, `Now`, undrained completions, and streams stay out of it. Per-lane origin limits and the pass-over are written tightly enough to implement, including the other lane and another origin in the §4 test. The terminal list matches `execute`'s success, earlier caller abort, deadline, over-limit, and `fetch_failure` order. Upload bytes that cannot fit the budget fail closed. The release table is the right shape for slot, response charge, upload, and ordered slot once finding 5's sentence agrees with it. The Linux adapter's per-lane `RustlsHttpTransport` can actually run its eight exchanges: each lease owns an agent. The ordered adapter permit cannot be taken by another executor.

§3 is true of that queue, the fence, forget, retirement's "do not join on the UI thread", grants, explicit independence, and the changes it lists. It is not true of `exactTimeout`, or of ordered progress while an `exactBodyFrom` reader is still blocked, until findings 1–3 are in the mechanism.

An implementer would still have to invent the park-between-stat-and-charge state (finding 1), the delegate queue, the non-copying upload handoff, and the metrics lifetime. The redirect continuation is determined by the text: `head` decides on the callback, the next hop starts on the starter, and only after the previous hop's physical end. Protocol is learned when an exchange finishes, so the 7th HTTP/1.1-turned-h2 request starts after the first completion rather than at ALPN. That is specified, and the measurement will see the stagger.

## Verdict

**RIGHT WITH CHANGES.** The direction is the right one, and both round-3 blockers are fixed in the mechanism rather than only named. Findings 1–4 have to land before a ruling can treat §3 as preserved or treat §5 as deciding the change.
