# Design review: LLP 1120 r1, 2026-10-09 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `8c521d8f925507af87fb856756c47a6a42cc0169743ea29f2b1521aece1e5718`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r2 (§0, round 1).

---

## BLOCKER

**1. The ordered lane’s start rule runs more than one blocking job.** `start_ready` runs under the state lock and starts every job that passes today’s `next_job` test (`llp/1120-network-concurrency-without-threads.rfc.md:49-52`). `next_job` does not look at what is already running (`host/apple/src/executor_core.rs:799-826`). One ordered job is in flight today only because one worker blocks inside `execute` until the exchange or the `Now` closure returns.

An ordered ceiling is about 128 MiB (`executor_core.rs:1007-1008`, body 64 MiB, charged twice). The ordered budget is 512 MiB (`executor_core.rs:37`). Looping `next_job` starts several ordered exchanges. A later `POST` would leave the machine while an earlier ordered request is still open. Settlement would stay FIFO, because `drain` still waits for `ordered.front()`. Side effects would not. LLP 1041’s ordered lane is that FIFO (`llp/1041-graceful-overload.rfc.md:436-441`, `460-470`).

The same paragraph says the HTTP exchange waits on no thread, and a separate work thread runs `Now` closures (`rfc.md:52-56`). Those two mechanisms do not share a slot, so an ordered file read or storage continuation runs while an earlier ordered HTTP exchange is open.

`OwnedWork::Later` is the opposite case. The worker calls the handoff and immediately pulls the next job (`executor_core.rs:1075-1083`). `asynchronous_module_reply_keeps_io_available_and_ordered_results_in_order` requires the later `Now` closure to run while the earlier module still holds the reply (`host/apple/src/executor_tests.rs:559-603`). A literal “one job in flight” rule fails that test.

The rule to write down: at most one blocking ordered job (HTTP, `Now`, or `exactBodyFrom`) is started; a `Later` handoff does not hold that slot; `start_ready` must not loop `next_job` on lane 0.

**2. Per-task delegates follow redirects unless they refuse them.** “No automatic redirects” is not a session configuration. `new_session()` refuses them by completing `willPerformHTTPRedirection` with nil (`vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:226-230`). The RFC lists that refusal as configuration (`rfc.md:41`).

`NSURLSessionTask.delegate` (iOS 15 / macOS 12) receives that task’s callbacks instead of the session delegate. The method is optional. If the task delegate does not implement it, URLSession follows the redirect. The 3xx never returns to `FetchTask`, so the per-hop `boundary::admit` check (`vendor/ibex/crates/ibex2/src/stdlib/fetch.rs:405-420`) does not run. That is the redirect-laundering case `darwin_http.mm:7-14` exists to prevent. Each task delegate has to refuse the redirect itself, and it has to be set before `resume`, on a task created without a completion handler.

**3. `head` cannot cancel the body before it is delivered.** The sink returns `Flow` only from `data` (`rfc.md:27-29`). URLSession’s disposition is the `didReceiveResponse` completion handler, which runs before any body byte (`darwin_http.mm:136-168`). Today a declared `Content-Length` over the ceiling is cancelled there, and `Transport::open` requires that (`fetch.rs:350-354`). HEAD, 204, 205, and 304 are exempt because their length is not a body (`darwin_http.mm:149-159`).

A 3xx body must also be cancelled before the next hop. `streamed_redirect_keeps_admission_and_does_not_collect_redirect_body` sends a 100-byte redirect body with a 4-byte ceiling and still expects `DeniedRedirect`, not the size error (`fetch.rs:637-651`). `head` has to be able to return the same stop, synchronously, before the transport allows the body.

## MAJOR

**4. Opaque work is not confined to the ordered lane.** `exact-native:` is admitted with `exactIndependentHttp` (`js/src/prelude.js:1137`) and dispatched as `OwnedWork::Later` (`runner/src/runner/commit.rs:1152-1164`). `is_ordered` is false for that request (`runner/src/request.rs:176-190`). A driver fault is `Work::Now` on whatever lane the request already has (`runner/src/runner/faults.rs:205-220`). The RFC gives both to one work thread “in ordered-lane order (the only lane that can carry them)” (`rfc.md:55-56`). Independent native calls then have no starter, and an independent fault waits behind ordered storage. Handoffs on the independent lane have to be started by `start_ready` and return immediately, as the worker does now. Ordered `Now` shares the single blocking slot from finding 1. Independent `Now` must not take that slot.

**5. `exactBodyFrom` loses its stall isolation.** The file is read on a thread of its own, up to `MAX_READERS` (4) per process, and the worker settles on the deadline without waiting for a stalled disk (`host/apple/src/executor_body.rs:10-19`, `93-95`, `107-140`). The RFC reads the file on the one work thread (`rfc.md:57`). A stalled read then holds the ordered blocking slot until the syscall returns, the deadline cannot settle mid-read, and a fifth stuck read no longer refuses. Independent uploads with file bodies also serialize on that thread. Keep the reader thread, the chunked cancel, and the process cap.

**6. The new in-flight cap couples the lanes.** D5 adds a global cap of 64 exchanges per executor and says LLP 1041 already names it (`rfc.md:85`). §8.4 does not. Admission stays 128 independent (`llp/1041-graceful-overload.rfc.md:443-454`). The cap is therefore new, and it is specified as global. Sixty-four tiny independent exchanges would stop the ordered exchange from starting, which is the capacity isolation D2 says separate sessions preserve (`rfc.md:40-41`). Today the ordered worker and its transport are unaffected by how full the independent lane is. Exempt the ordered exchange, or count the cap on the independent lane only, and make “over the cap” wait for a slot (LLP 1054.000 R3), not a refusal. Say which.

**7. D7 drops stream properties that are not “a session of their own”.**

- An SSE body is uncapped at the transport; the ceiling is per event (`host/apple/src/executor_stream.rs:184-185`). Routing streams through D1’s `Flow::Stop` ceiling ends a long stream at `maxResponseBytes` of total bytes.
- Each WebSocket has its own ephemeral session, refuses redirects (a handshake redirect fails; it is not followed), and sets `maximumMessageSize` to the ceiling plus one so an oversize frame is refused before it is buffered (`vendor/ibex/crates/ibex2/src/transport/darwin_websocket.mm:1-14`, `196-212`). One shared stream session with the HTTP task delegate follows a redirect and will buffer the frame unless `maximumMessageSize` is set. Sixteen sockets on that session also share one HTTP/1.1 per-host cap. Today each socket has its own connection.
- Streams still need the grant check, scoped grants, and `exactBodyFrom` (`executor_stream.rs:55-76`). D7 does not say they go through `FetchTask`. WebSocket redirects must fail. SSE redirects must go through `FetchTask`.

**8. Delegate lifetime, the state lock, and the serial queue are unspecified.** Callbacks for every task on a lane share that session’s serial delegate queue. A `data` callback that appends, parses SSE, or takes the executor lock stalls every other exchange on the lane, and it deadlocks if the lock holder waits for the queue. `start_ready` is specified to run under the state lock (`rfc.md:49-51`); `task.resume()` and a Linux `thread::spawn` have to happen after the lock is released. The render host’s wake already runs under that lock and only signals (`host/render/src/executor.rs:51-54`).

`invalidateAndCancel` still delivers `didCompleteWithError` and `didBecomeInvalidWithError`. Those callbacks have to hold `Arc` of the shared state until invalidation finishes, see a retired or missing exchange, and return. `complete` does the check (`executor_core.rs:829-836`); the RFC says so (`rfc.md:62`) and does not say who keeps the `Arc`. Dropping the delegate with the `Core` is a use-after-free. `URLSessionTask.cancel` is asynchronous: `data` after `Flow::Stop` has to be ignored. Classify the completion in the existing order: caller abort, then deadline `Timeout`, then over-limit `Refused`, then network (`executor_core.rs:1273-1289`).

Session objects also have to stay lazy. The first `NSURLSession` in the process is deliberately not built at construction, off the presenter, because it costs tens of milliseconds on the boot floor (`vendor/ibex/crates/ibex2/src/transport/darwin.rs:80-82`, `305-312`; `executor_core.rs:202-205`). Three sessions created in `Executor::start` put that cost back on first frame.

**9. Linux threads fall out of the process cap, and the adapter’s pool is unspecified.** D3 redefines `MAX_WORKERS` as two threads times sixteen executors (`rfc.md:63`). D4 then adds one thread per exchange, up to 128, plus one per stream (`rfc.md:67`, `104`). LLP 1041’s cap covers live and retiring workers until they exit (`llp/1041-graceful-overload.rfc.md:494-497`; `executor_core.rs:15-21`, `716-720`). Adapter threads, stream threads, and the four `exact-body-from` readers are outside the new cap. A replacement storm can pass the old 112.

The adapter also has to lease from one `RustlsHttpTransport` owned by the executor. Agents are exclusive per in-flight response and reused only when returned to that pool (`vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:58-59`). A new transport per thread drops keep-alive. The render host pools executors so the next page reuses connections; a fresh TLS handshake dominated RealWorld (`host/render/src/lib.rs:413-417`). That host does not use `NSURLSession` unless `EXACT_RENDER_TRANSPORT=platform` (`host/render/src/executor.rs:23-31`), so on the default path it gets this adapter, including on macOS.

D4c is a sound way to land the shared core without a new runtime. It does not deliver the HTTP/2 win this RFC is for. D4b (a hand-written HTTP/1.1 reactor) does not either, so it is not the thing to switch to when multiplexing is the question. There is no tokio or hyper in this workspace; that part of the objection is true. The size objection is not measured. A later switch, if any, is D4a, after a binary-size spike.

**10. The redirect list drops the 301/302 rewrite.** `FetchTask` is “the same steps”, and the steps it names include `303 → GET` (`rfc.md:31`). The code also turns 301 and 302 of a non-GET/HEAD into GET and drops the body (`fetch.rs:469-475`). An implementer who checks the RFC’s list resubmits the POST body to the next URL. 307 and 308 keep the method. The deadline covers the whole chain, not each hop (`host/apple/src/executor_timeout_tests.rs:86-87`).

**11. The migration’s “unchanged tests” exception is too small, and the measurement cannot decide what it claims.**

Existing `executor_*_tests.rs` encode the handoff overlap, the timeout classification, body-from stalls, and stream close. ibex2 encodes redirect bodies, declared-length refusal, cookies, and cache. The RFC exempts only tests that name worker counts (`rfc.md:123-127`). Name the invariant tests. Add the ones that do not exist: ordered HTTP stays one-at-a-time while a `Later` handoff is outstanding; a task delegate that forgets `willPerformHTTPRedirection` is a failing test; retirement delivers a late `didComplete` into a live `Arc` and a dropped `Core`; `exactBodyFrom` settles while its reader is still blocked.

On measurement (`rfc.md:133-153`):

- Build B cannot show “16 workers”. A 4 MiB independent ceiling is charged twice plus header allowance, and the lane holds 64 MiB, so about seven run (`rfc.md:53`; `llp/1041-graceful-overload.rfc.md:444-448`). Sixteen workers and six workers start almost the same number of the clone’s reads. B does not separate thread count from multiplexing.
- “No worse than B on any metric” with 8 simulator runs and no median-versus-worst rule lets `fling-discover` noise veto the change, or a single lucky run pass it. The 6-worker amendment reported median and worst (`llp/1041-graceful-overload.rfc.md:449-451`).
- The useful Apple question is A versus C under LTE: cold `zero_to_post`, the span of the 19 feed reads, and `URLSessionTaskMetrics` showing one `h2`/`h3` connection (`isReusedConnection`, `networkProtocolName`, DNS and TLS durations). That can decide the device change. The mechanism check has to be part of acceptance, not only the time.
- Linux A versus the adapter cannot show that HTTP/2 matters on Linux, which is the stated trigger for leaving D4c (`rfc.md:74`). There is no HTTP/2 arm and no binary-size number.

## MINOR

**12. Independent completions keep the running charge, not the retained size.** `complete` uses `retained()` on the ordered lane and `run.charge` on the independent lane (`executor_core.rs:853-858`). The amendment’s “completed-results test is kept as it was” (`llp/1041-graceful-overload.rfc.md:776`). D5’s bullet “a completed one what it retains” (`rfc.md:82`) would shrink independent reservations and admit more work. Say “as `complete` does”.

**13. Preconnect is honest about the pool and thin about everything around it.** There is no public URLSession preconnect; WebKit’s `_preconnect` is private. A Network.framework connection that is then closed is not in the session’s pool, and `isReusedConnection` can stay false while DNS and a TLS ticket still get warmer. That part (`rfc.md:91-95`) is accurate. Also true, and unsaid: URLSession will use the system proxy and VPN, and a raw `nw_connection` to the origin often will not, so the warm-up can heat a path the session will not take. The origin still sees a ClientHello. “The first `net.fetch` origin” misses the clone’s other hosts. The boot rule cited in the header is about app JS before first pixel (`rules/RULES.md:54-57`), not a ban on host I/O; LLP 1016’s rule is that the host runs requests a source handed back (`llp/1016-async-data-settlement.rfc.md:20-22`). A handshake the source did not ask for is outside that.

**14. Round-robin is a second scheduling change, keyed by the resource name.** `RequestOut::target` is the resource or mutation name (`runner/src/request.rs:661-665`), not the origin. One source’s `Promise.all` often shares a target, so it does not address the 19-read launch. `next_job` will not pass a front job that does not fit (`executor_core.rs:808-810`); skipping it to start another target’s small read changes who waits for bytes.

## NIT

“Twice its ceiling” is the prose. The charge is request bytes plus twice the ceiling plus twice the 64 KiB header allowance (`executor_core.rs:1005-1008`). “Seven” for 4 MiB ceilings is the right neighborhood. HTTP/2 and HTTP/3 both ignore `HTTPMaximumConnectionsPerHost`; the documented default is 6, so that sentence in D2 is fine. Keeping `fetch_stream` and `FetchTask` as two copies will drift; running the redirect tests against both, as D1 says, is the mitigation.

## §6

1. **Keep FIFO on the independent lane.** Reordering is allowed by the opt-in (`llp/1041-graceful-overload.rfc.md:436-441`). The host does not have a measured fairness failure, and round-robin by resource name does not fix a single source’s fan-out. It also changes which job waits for bytes.

2. **Adapter now, for this change.** A reactor from the start is D4a or nothing. D4b does not speak HTTP/2, so it cannot be the answer to the question that would retire the adapter. Do not treat “revisit on evidence” as decided until the plan includes a Linux HTTP/2 build or a size spike. The render host follows the adapter even on macOS.

3. **A request-free warm-up is acceptable as a measurement flag that defaults off and is deleted if the numbers are flat.** It is not acceptable to land on `Executor::start` on the strength of this RFC. It does not enter URLSession’s pool; it may warm the wrong path; the origin can see it. Name a threshold (DNS time and `secureConnection` duration, not `isReusedConnection`) and wait until the warm-up handshake has finished before reading the first task.

4. **One session per lane per executor.** Retirement is `invalidateAndCancel` on that executor’s three objects, and a pooled render executor must keep its connections across settled renders (`host/render/src/lib.rs:413-417`) without sharing them with another grant set or a successor after reboot. A process-wide session saves a handshake and needs a generation check on every callback. That lifetime is the cost D2 correctly declines.

## Verdict

**RIGHT WITH CHANGES.** Amend the RFC before implementation. The ordered start rule, the task delegate’s redirect refusal, and a synchronous cancel on `head` have to be specified so that §3 is true of the mechanism, not only of the intention. Separate the independent-lane cap from the ordered exchange, keep `exactBodyFrom`’s reader threads, and give independent `Later` and `Now` a starter that is not the ordered work thread. Treat D4c as the non-Apple plan, with the pool owned by the executor and the process thread cap restated. Reduce the measurement to A versus C under LTE plus connection metrics, and do not use sixteen workers or an unmeasured Linux reactor as the deciding arms.
