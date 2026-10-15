# LLP 1120: Network concurrency without a thread per request

**Type:** RFC
**Status:** Draft r7, 2026-10-12, with round 7's findings written in (§0). **Approved by Charlie, 2026-10-10**, to be implemented once the design rounds converge (rounds up to 7 allowed). Rounds 1–7 were reviewed blind by GPT-6 Astra (max) and Grok 4.7 (xhigh); every verdict was RIGHT WITH CHANGES (`llp/reviews/1120-network-concurrency-without-threads{,-r2,-r3,-r4,-r5,-r6,-r7}.{astra,grok}.md`). r7 takes every round-6 finding, and round 7's are written in without another round, the last allowed (§0). One needs a ruling (§6, question 8). Stage 1 is built behind a switch, off (§4); §5's Apple gate, measured 2026-10-12, failed, so the switch stays off. The October 13 adoption below uses upstream Ibex services without enabling the production switch. Charlie explicitly authorized the remaining correctness and rollout follow-up on October 13 ("ok do all those things"); §4 records the implementation. Independent Codex and Grok code reviews cleared the stage-1 correctness follow-up and its Ibex pin adoption at Exact `56f107fa4`; the broader stages below remain open. A passing measured §5 gate remains required before default activation.
**Systems:** the native executor (`host/apple/src/executor_core.rs`, `executor_stream.rs`, `executor_body.rs`), compiled into Apple, Linux and the render host; ibex2's fetch layer (`vendor/ibex/crates/ibex2/src/stdlib/fetch.rs`) and transports (`transport/darwin.rs`, `darwin_http.mm`, `darwin_websocket.*`, `websocket.rs`, `rustls_http.rs`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-10, r6 and r7 2026-10-12
**Implementer:** x2-perf3, once ruled
**Related:** LLP 1041 §8.4 and its 2026-10-09 amendments (lanes, admission, the read queue, the fence; six independent workers since 1b31848f5); LLP 1016 D2/D6; LLP 1016.000 D3/D4 and LLP 1069.004 (streams, WebSockets); LLP 1054.000 R3 (a job waits for bytes rather than being refused); LLP 1067 D3 and `js/src/native.rs` (`native.later` runs on a thread of its own); LLP 1103 D1 (driver faults); LLP 1108 D6 R2 (`exactBodyFrom`); LLP 1109 D3; LLP 0057 §3, LLP 0068 §5.

## 0. The design rounds

**Round 1** (r1 → r2): r1's mechanism didn't keep LLP 1041's semantics. r2 took every finding:
- one blocking ordered job at a time, with handoffs not holding the slot;
- an in-flight cap on the independent lane only;
- task delegates that refuse redirects, and a stop at headers;
- work starters for both lanes;
- bounded `exactBodyFrom` readers;
- a session per stream;
- callback and lock rules, and lazy sessions;
- corrected accounting;
- the Linux adapter;
- FIFO order;
- preconnect as a default-off flag;
- A vs C under a throttled link.

**Round 2** (r2 → r3): both reviewers confirmed the round-1 fixes and the direction, and found mechanism still unwritten. Each finding is taken:
- **Starter thread** (Grok M1, Astra M4). D3 now names the execution context of every start: a **lane starter** per lane. A handoff's return re-enters the decision, so a queued run behind it moves. A selected job keeps its start reservation until its invocation returns. Delegate callbacks only append and enqueue (D8).
- **Handoffs charge no response bytes** (Astra M1), as `next_job` does today. The charge formula now has the `max(2 × ceiling, 32 KiB)` floor (Grok m7). D5.
- **The sixteen** is an admission predicate for effects, not a cap on reads (Astra M2). D5 states each predicate as `admit` does.
- **Deadlines begin when the exchange can really go out** (Grok M2). The core never lets URLSession queue a request behind its per-host limit: the per-origin limit is the session's 6 until an exchange on that origin reports HTTP/2 or HTTP/3. Terminal transitions are defined for each order of events (Astra M5). D3.
- **The process thread cap is a number, and progress threads are reserved** (both). D4.
- **What the 32 counts** (Grok M4): resumed HTTP exchanges, from start to their transport's end. Logical and physical release are separate (Astra M5). D5.
- **Upload bytes are charged for the exchange's lifetime** (both; the lead's ruling, 2026-10-10). D5. §7's gap closes for the async path.
- **Measurement gating** (both): C7 against a real 7-worker A7 decides, with mechanism metrics required. Linux gets its own rule, and "cold" is defined. §5.
- **Minors:**
  - a 3xx stop isn't terminal, and the declared-length refusal comes first (Grok m6);
  - header checks on every hop are a declared change, classified as today (Astra m9);
  - SSE parsing only for a 2xx `text/event-stream` (Astra m8);
  - the WebSocket's asynchronous interface is spelled out (Grok m8);
  - §3's overclaims are corrected (Astra m10);
  - ibex2's pull-based consumers keep the blocking form (Astra m8);
  - session invalidation is the session delegate's (Grok n9).

**Round 3** (r3 → r4): both reviewers confirmed the round-2 fixes and the direction. Each finding is taken:
- **Ordered handoffs behind a blocking job** (Grok B1). The ordered starter invokes a handoff only while no blocking ordered job is in flight. The blocking slot lasts until that job's physical end (D3).
- **An upload that can never fit** (both B). A request whose charge with its file exceeds the lane's whole budget is failed at selection, not left waiting at the head (D5).
- **Upload preflight** (Astra M2). The size is taken by the reader, under its permit and the deadline. Reading is bounded by the reserved size, and growth fails the request. One copy of the body is shared across 307/308 hops (D3, D5).
- **Deadlines and transport queuing** (Astra M3, Grok M4/M5). URLSession never queues an HTTP/1.1 request below the core's limits: the independent session's `HTTPMaximumConnectionsPerHost` is the lane cap. The per-origin limit of 6 is the core's own, per lane, and a head at its origin's limit doesn't block later origins. The remaining queue, an HTTP/2 peer's stream limit, is a declared change (D3).
- **A release table** (both M). The slot, the response charge, upload bytes and the ordered blocking slot each have their release event on every path (D5).
- **Completion progress and stream buffering** (Astra M5, Grok M6/m7).
  - Callbacks do bounded CPU work: `head` decisions, SSE framing into the coalescing slot. They do no I/O or opaque work.
  - A per-lane finisher builds outcomes, so a long `Now` on the starter doesn't stall completions.
  - Session creation, protocol updates and stream starts run on the starter (D3, D7, D8).
- **Measurement** (Astra M6, Grok m8): preconnect counterbalanced; connection identity from a controlled server and endpoints; operational "not worse" rules; Linux worst-case, RSS and reader clauses (§5).
- **Minors:**
  - typed WebSocket failures (Astra m7);
  - one thread counter covering all pools, eager creation, partial-failure unwind, and the unused-executor measurement (Astra m8, Grok m9);
  - streams keep a per-executor 16; a process-wide exhaustion is a declared new refusal (Grok m9);
  - the timer only flags and aborts (Grok n10).

**Round 4** (r4 → r5): both confirmed the round-3 fixes. Each finding is taken:
- **Uploads that deadlock each other, or wait with the deadline armed** (Astra B1, Grok M1). Sizing is its own phase: a stat under a reader permit, with no reservation held. The job then returns to the head of its lane with its size known, unarmed, and starts again only when its **whole** charge, response plus file, can be taken at once (D3, D5).
- **"URLSession never queues" is false** (both M). Apple may lower a session's limit. Ruled instead: **any platform queuing after `resume` counts against `exactTimeout`**, a declared change. The core's per-origin limits stay as best effort (D3).
- **Prompt terminal publication** (Astra M3, Grok n12). The timer and aborts post a terminal event straight to the finisher, which publishes exactly once, whether or not the transport, a reader or a resolver has ended. Opaque destruction moves to a separate **disposer** thread (D3).
- **The ordered slot and a stalled reader** (both M). The ordered slot is released when the request reaches its terminal transition, as today's worker moves on. A reader keeps its permit and upload charge, and a cancelled task its connection and in-flight slot, until their physical end (D3, D5).
- **The release table** (Astra M5, Grok m5). Independent completions of every kind, success or failure, keep their running charge less upload bytes until drained, as `complete` does. Forgotten outcomes release at physical end. Rows are added for transport errors, header failures and failures between hops (D5).
- **Measurement** (both M).
  - Preconnect is a separate experiment with a simulator reboot before **every** run, and a verified cold DNS state (the first task's `domainLookup` > 0 in C).
  - A treatment-arm failure fails its gate, and a control failure makes the comparison inconclusive. There are no exclusions or replacements (§5).
- **Minors:**
  - thread arithmetic fixed: Apple 6, Linux 15, the disposer included; permits are taken per created thread after an atomic capacity check, and the unspawned are returned at once; a reader refused when the counter is exhausted (D4);
  - the upload buffer is lent to URLSession without a copy (D2);
  - the delegate queue is named, and callbacks never wait (D2, D8);
  - the protocol report survives metrics arriving after `didComplete` (D2);
  - the Linux thread gate is a formula (§5);
  - `start_socket` takes the ceiling (D1);
  - the FIFO question names the origin exception (§6);
  - prose bounds defer to `reservation` (D5).

**Round 5** (r5 → r6): both confirmed most round-4 fixes and the direction. Each finding is taken:
- **An upload's reader result wakes the starter** (Grok B1). A reader posts its result (a size, the bytes, or a failure) to the lane's starter, which is on the wake list. A parked upload has its own release-table row: no slot, its deadline disarmed, its waiting charge kept, the in-flight slot not taken. An ordered one stays the lane's head, as a head short of bytes does today, and its bytes come from earlier outcomes draining (D3, D5).
- **Every charge ends at the later of its logical and physical release** (Astra M1, Grok m6). A timed-out or failed exchange's running charge is held until **both** its drain (or, ordered, its shrink to what it retains) **and** its physical end. Upload bytes end at the later of the last outstanding hop's physical end and the lent buffer's last release, on every row. Tests for drain-before-cleanup and cleanup-before-drain (D5).
- **Upload bytes on a hop that never starts** (Grok M3), and **a body-dropping redirect** (Grok m7). With no hop outstanding (a refused redirect, a timeout while waiting for the cap), the upload is released at the terminal transition. A 303, or a 301/302 of a non-GET/HEAD, ends the upload charge with the hop that last sent it (D5).
- **File-backed streams** (Astra M2). A stream never waits for bytes: it is refused at admission when its ceiling doesn't fit, as today. An `exactBodyFrom` stream is sized after admission and takes its file's charge at once or is refused, a declared change (D5).
- **`forget` of an in-flight write is not an abort** (Grok M2). Only a cancellable operation (a safe read) gets the terminal event. A forgotten write, `Now` or storage runs to its own end and keeps the ordered slot, and `complete` drops its outcome, as today (D3).
- **Linux's ordered adapter after a timeout** (Astra M3). Physical adapter availability is separate from the logical ordered slot. An ordered HTTP job starts only on an idle adapter thread. A thread still busy after its request's terminal transition becomes a straggler, and the starter replaces it from the process counter, up to two stragglers per executor. Otherwise the job waits in the core, its deadline not armed (D4).
- **One upload deadline rule** (Astra m6, Grok m5). One deadline from sizing through the exchange. It is disarmed only if the job parks for bytes; the parked request's timer generation is bumped so a stale firing is ignored, and it re-arms when the job starts again (D3).
- **The per-origin table is changed by the finisher under the state lock** (Grok m9). A callback records the protocol in its exchange; a late metrics report is posted to the finisher as an event. The sizing phase holds only the request buffers, in the starter's general rule as in D5.
- **Measurement** (Astra M4/M5, Grok M4).
  - A new gate: **C is not worse than today's A6**, median and worst.
  - A treatment failure fails its gate and stops the campaign, whatever the controls did. A control failure with every treatment run complete reruns the whole sequence once. A second control failure is reported inconclusive.
  - Preconnect's cold check is made on the C runs only, where no treatment touches DNS. A positive `domainLookup` is reported as evidence, not as proof of an empty cache. The failure rule covers C+P and the Linux comparison (§5).
- **Minors:** the handoff-behind-a-blocking-job test exercises terminal-before-cleanup (§4); the metrics fallback is described as what it is (D2); §3's Limits bullet names all platform queuing (§3).

**Round 6** (r6 → r7): both confirmed the round-5 fixes and the gates. Each finding is taken:
- **Every release wakes the starter after it is applied** (Grok B1, Astra M3). Callbacks, the timer, the buffer deallocator, and a reader's failure or exit only post to the finisher (a reader's size or bytes go to the starter, which is waiting on them). The finisher applies each release under the state lock: a charge's final release, the ordered slot at a terminal transition, a per-origin count, a reader permit, the lent buffer. Only then does it wake the starter. A permit returned to the process counter wakes every starter waiting on the counter. Tests make each release the only event that can make progress (D3, D4).
- **`forget` cancels streams** (Astra M1). Forget cancels safe reads and every stream, whatever its method, as today every stream has an abort controller. Retirement aborts every HTTP exchange and stream, as today. Writes alone keep r6's rule (D3).
- **A write waiting for an adapter thread is a queued write** (Grok M2). `forget` drops only a safe read from that wait; anything else is marked and runs when a thread accepts it (D4).
- **The ordered adapter stays reserved through recovery** (Astra M2). The base ordered permit belongs to the lane's ordered role, not to a thread. A returning straggler takes the role back if no other ordered thread holds it, and otherwise exits. The lane never has more than three ordered adapter threads: one holding the role and at most two stragglers. A replacement is created only when a job is ready, no thread holds the role, and the lane has fewer than three (D4).
- **A Linux file upload's one deadline arms at adapter acceptance** (Grok M3). On the adapter, an `exactBodyFrom` job's sizing starts only once an adapter thread has accepted it. The thread is held through sizing and reading, under the one deadline. A park returns the thread with the deadline disarmed (D4, D5).
- **The sizing reader reads** (Grok m5). It posts the size and waits for the starter's decision. If the charge is taken, the same reader, under the same permit, reads; otherwise it exits. Phase 2 never asks for a second permit (D5).
- **The Linux thread bound counts the extras** (Astra m4, Grok m6): at most two extra ordered threads per executor, in the arithmetic and in §5's formula. **The base set is keyed to the transport**, not the host: a platform-session transport (Apple, and the render host with `EXACT_RENDER_TRANSPORT=platform` on Apple) has the 6, and the adapter has the 15 (D4).
- **File-backed streams** (Astra m5, Grok m4). An interested stream refused after sizing keeps its charge until drain and physical end, as any independent failure. The test expects both streams refused when both were admitted before either was sized. The stream's read and resume are written out (D5, §4).
- **Minors:** a reader's failure is a numbered terminal transition (D3); a treatment failure stops its own comparison, not the other experiments (§5).

**Round 7** (r7, written in; no further round). Both confirmed every round-6 fix as mechanism. Each finding is taken:
- **A parked upload's liveness against the requests queued behind it** (Astra B1). A parked head can't start while successors it blocks hold waiting buffers that leave too little room. **Proposed rule, for the ruling (§6, question 8):** at the park, a whole charge that doesn't fit beside the waiting charges of every job queued behind it is refused at once (`Refused`, "the file and the requests queued behind it exceed the lane's byte budget"). From then on, while a job is parked, admission to its lane counts the parked job's outstanding charge as taken, so a later request that would crowd it out is refused at admission ("native executor admission limit reached", as today when bytes don't fit). Everything that then holds the rest of the budget is running or undrained and ends without the parked job, except an open stream, which can hold its charge as long as it is open, as it can for an ordinary head waiting for bytes today (LLP 1054.000 R3) (D5).
- **The adapter re-start after a park arms at acceptance** (Grok M1). A parked job on the adapter starts again only when, in one decision under the state lock, its whole charge can be taken **and** an adapter thread can accept it. Only then is the fresh deadline armed. Otherwise it stays parked, charge untaken and deadline disarmed (D4, D5).
- **Stream thread costs follow the selected transports** (Astra m2), not the host: an SSE stream costs a thread under the HTTP adapter (Linux, and render by default on every platform), a WebSocket two under the rustls socket transport and none under the platform's. The counter's waiters are ordered replacements only; streams and readers refuse (Astra n3) (D4).
- **NITs:** §0's round-6 reader wording (Grok n2); the fifth reader's refusal is today's diagnostic, quoted (Grok n3).

## 1. Why

Native HTTP concurrency equals the number of worker threads. Each worker owns a transport and blocks on one exchange from request to last body byte. With two independent workers, the Bluesky clone's `Promise.all` of 19 feed reads came back two at a time. Six (1b31848f5) cut runtime zero to first post from 822 to 662 ms median on the simulator. A thread count is the wrong knob:

- **Each worker's transport is its own pool.** On Apple, each `DarwinTransport` owns up to four `NSURLSession`s, each leased to one response. Sessions share no connections. So six busy workers make up to six TCP and TLS handshakes to one origin on a cold launch, and HTTP/2 multiplexing never happens.
- **Threads are a cost, not a budget.** Each executor has seven workers, plus a thread per timed request (`Deadline::arm`) and one per open stream.
- **The real bounds are already bytes and counts** (§8.4).

**Proposal:** HTTP exchanges go to the platform's asynchronous client. On Apple that's one `NSURLSession` per lane per executor, HTTP/2 where the origin speaks it. Concurrency is bounded by byte budgets and counts.

## 2. Design

### D1. ibex2: an asynchronous transport and a fetch state machine

**Interface.** A transport adds `start(Request, AbortSignal, Box<dyn Sink>) -> Exchange`. The `Request` is owned by the transport from then on.

- `start` never calls the sink before returning. A failure known at once is delivered asynchronously, so a caller holding a lock can't re-enter it.
- The sink's methods are called on one serial context per exchange, never concurrently:
  - `head(status, headers, declared_length) -> Flow`, before any body byte;
  - `data(&[u8]) -> Flow`;
  - `end(Result<(), HostError>)`, exactly once.

**`head` decisions, in this order:**
1. A declared length over the ceiling is refused. It stops the exchange, and the terminal outcome is `over_limit` (→ Refused). This holds for any status that has a body, 3xx included; HEAD, 204, 205 and 304 are exempt, as `darwin_http.mm:149-159`.
2. Headers over 64 KiB or 1024 entries stop the exchange.
   - **This is a declared change:** today `check_headers` runs only on the final response, after redirects (`executor_core.rs:1268`). r3 checks every hop, so a hop with oversized headers now fails the fetch.
   - The outcome stays `Network`, as today's "HTTP headers exceed limit" is.
3. A 3xx that `FetchTask` will follow returns `Flow::Skip`. That stops the body **without a terminal outcome**: the hop's `end` is ignored, and `FetchTask` admits and starts the next hop. `streamed_redirect_keeps_admission_and_does_not_collect_redirect_body` keeps its meaning (a refused next hop is `DeniedRedirect`, not the redirect body's size).

**`data`** enforces the ceiling incrementally, as `body_limit()` does. Over it, the exchange stops and the terminal outcome is `over_limit`.

**After a stop**, any `data` or `end` the platform still delivers releases resources and changes nothing. The terminal outcome is the one recorded at the stop, never the platform's cancellation error.

**`FetchTask`** is today's `fetch_stream` loop as a state machine. On each hop:
- `boundary::admit` on the hop's origin, with `DeniedRedirect` naming the target;
- guarded headers and `MAX_REDIRECTS`;
- 303, and 301/302 of a non-GET/HEAD, become GET with no body; 307/308 keep method and body;
- a cross-origin hop drops `authorization`, `cookie` and `proxy-authorization`;
- a 3xx with no `Location` is returned as is;
- `RedirectMode` is honoured.

Each hop is its own exchange with its own cancellation registration. A late event from an earlier hop is ignored by hop identity.

**The blocking `Transport::open`** stays for ibex2's pull-based consumers (`task.rs`'s `read_response` pulls chunks on demand). The executor stops using it. Both forms run `fetch.rs`'s redirect and grant tests.

**The asynchronous WebSocket.** It is `start_socket(url, ceiling, AbortSignal, Box<dyn SocketSink>) -> Socket`, with:
- `opened()`;
- `text(String) -> Flow`, one message at a time. The per-message ceiling is enforced by the transport (`maximumMessageSize` = ceiling + 1 on Apple). A binary frame or an oversize message ends it.
- `closed(Closed)`, exactly once. `Closed` is typed (D7): a local refusal, a peer close with its code and reason, a transport failure, or an abort.

Exact sends nothing, so there's no send queue. A handshake redirect fails, as today.

### D2. Apple: a session per lane, per executor, created lazily

- **Sessions.** The ordered and independent sessions are created by the lane's starter on first use, never at `Executor::start`, because the first `NSURLSession` costs tens of milliseconds (`darwin.rs:80-82`). Configuration is `new_session()`'s: ephemeral, no cookies, no URL cache, `reloadIgnoringLocalCacheData`. One addition: the independent session's `HTTPMaximumConnectionsPerHost` is the lane's in-flight cap, 32; the ordered session keeps the default, 6 (D3).
- **Isolation.** Separate sessions keep LLP 1041's isolation, because `HTTPMaximumConnectionsPerHost` is per session.
- **Task delegates.** Each task has its own delegate, set before `resume` on a task made without a completion handler. It implements:
  - `willPerformHTTPRedirection` → nil, always;
  - `didReceiveResponse` → `head`, then the disposition;
  - `didReceiveData` → `data`;
  - `didCompleteWithError` → `end`;
  - `didFinishCollectingMetrics` → the measurement record (§5) and the origin's protocol (D3).
- **Delegate queue.** Each lane session has its own serial `NSOperationQueue` (`maxConcurrentOperationCount = 1`) as its delegate queue. Every exchange on that lane shares it, so callbacks never wait on anything: no hand-off buffer, no lock held across I/O (D8). Each callback holds an `Arc` of its exchange.
- **The upload body is lent, not copied.** The request's `HTTPBody` is `NSData` made with `dataWithBytesNoCopy:length:deallocator:` over the charged buffer (D5). Its deallocator drops the buffer's `Arc`, so the bytes URLSession holds are the bytes charged. The upload charge ends when D5's release table says, never before the buffer's last release.
- **Protocol reports.** `didFinishCollectingMetrics` can arrive after `didComplete`. The exchange's record stays reachable until both have been delivered, or for 1 s after `didComplete`. Metrics later than that are dropped, and the origin keeps the limit it has: an origin never reported as `h2` or `h3` stays at the HTTP/1.1 limit, the safe default. The callback records the protocol in the exchange; the lane's per-origin table is changed only by the finisher (D3). `networkProtocolName` values other than `h2` and `h3` keep the origin at the HTTP/1.1 limit.
- **Session lifetime.** The session's own delegate (an object per session) observes `didBecomeInvalidWithError` and holds the executor's shared state until then. Retirement disables wakes, then calls `invalidateAndCancel`. Each task's `didComplete` still arrives, finds its exchange retired, releases it and returns.
- **Scoped grants** are a scoped `GrantSet` checked by `FetchTask`, after `exact_data::storage::scope` validation as today.

### D3. The core: who starts what, and when

`State` and its admission rules are unchanged (D5, §3). Each lane has two threads:
- a **starter**, the only place that lane's work starts;
- a **finisher**, the only place that lane's outcomes are built and published.

One **disposer** per executor destroys forgotten and retired opaque work, so a slow destructor delays neither starts nor publication.

**The starter.**
- Admission, `drain`, `forget`, `resume_ordered`, a handoff's return, a reader's result (a size, the bytes, or a failure), and the finisher after it has applied a release (below) each wake it. A permit returned to the process counter wakes it when it is waiting on the counter (D4).
- It decides under the state lock which jobs to start and moves each into `running` with its running charge. An `exactBodyFrom` job's sizing phase holds only its request buffers (D5).
- It releases the lock, then for each job: creates its session if this is the session's first use, resumes its HTTP task, calls its handoff, runs its `Now` closure or storage operation, or starts its `exactBodyFrom` reader.
- A job's start reservation is held from selection until its invocation returns, including when a handoff's `Reply` completes synchronously inside the call.
- It also starts streams (D7), so no stream task is resumed on the host's pump.
- Handoffs run on it (the `native.later` contract, LLP 1067 D3), as do `Now` closures and storage. **Declared change:** a long independent `Now` (only driver faults, LLP 1103 D1) delays the lane's *starts* until it returns, where today it occupies one of six workers. Completions keep flowing through the finisher.

**The finisher.** Its queue takes these events:
- a **finished exchange** from a callback;
- a **terminal event** posted directly: by the timer when a deadline fires, by `forget` or a caller's abort of a cancellable operation (a safe read or a stream; see **Forget** below), or by a reader's failure;
- a **physical end**: a transport's `didComplete` after the outcome was already published, an adapter thread's return, or a reader's exit;
- a **buffer release**, posted by the lent buffer's deallocator (D2), which may run on any thread and does nothing else;
- a **protocol report**: an exchange's protocol from its metrics, posted by the callback that recorded it, including one that arrives after `didComplete`.

**Release, then wake.** For each event, the finisher applies under the state lock every release it makes due: an outcome published (`complete`), the ordered slot at a terminal transition, a charge's final release (the later of logical and physical, D5), an in-flight slot, a per-origin count, upload bytes, a protocol report into the per-origin table. Then, with the lock released, it wakes the starter. No callback wakes the starter itself, so the starter never sees a release before it is applied. `drain` applies its release under the same lock before it wakes the starter.

Each request has one terminal transition, recorded under the request's own lock by whichever event wins (the rule below). The finisher publishes it exactly once (`complete`). It doesn't wait for the transport's `end`, a reader's exit, or a resolver's return, so a timeout settles on time even when the underlying operation is stuck.

It builds outcomes: the final response's header check, bounding, `Response` construction. It never runs a closure, a handoff, storage or a destructor of opaque work.

**Ordered lane:**
- At most **one blocking ordered job** is in flight: an HTTP fetch (across every redirect hop and its file read), a `Now` closure, or storage.
- The **blocking slot is released at the job's terminal transition**, as today's worker moves on when its blocking call returns with an outcome. That's the request's success, failure or timeout, or the closure's return.
  - A cancelled task keeps its connection and its resources until its physical end (D5), but not the ordered slot.
  - A stalled reader keeps its permit and upload charge until it exits, but not the slot.
  - The ordered session's `HTTPMaximumConnectionsPerHost` stays at its default (6), so a cancelled task still holding a connection doesn't block the next ordered request's connection.
  - The next ordered job, a handoff included, waits for the slot.
  - A forgotten write, `Now` or storage isn't cancelled, so it keeps the slot until its own end (**Forget** below).
- **A parked upload** (D5: sized, but its whole charge doesn't fit the bytes free now) holds no slot. It stays the lane's head, and nothing behind it starts, as a head short of bytes doesn't pass today (`next_job`). Its bytes come from earlier outcomes draining and earlier exchanges ending, which don't wait on it.
- **A handoff is invoked only while no blocking ordered job is in flight.** A handoff that has returned doesn't hold the slot, so the next job may start. When it returns, the starter re-decides before parking, so a run of handoffs followed by a blocking job reaches that job. A handoff queued *behind* a blocking job waits for it, as on today's single worker.
- Execution and settlement are FIFO, the latter through `ordered`, unchanged.

**Independent lane.**
- Jobs start in FIFO order while each fits `BYTES[1]` and the in-flight cap (D5).
- **One exception to strict FIFO:** a head HTTP job whose origin is at its per-origin limit (below) is passed over, and later jobs to other origins may start. LLP 1041 allows this lane to reorder. A head that doesn't fit the *bytes* is not passed over, as `next_job` today.
- A handoff (`exact-native:` calls) is called and returns.

**Transport queuing and the deadline.**
- **Declared change: any platform queuing after `resume` counts against `exactTimeout`.** URLSession may hold a resumed task back:
  - behind a per-host connection limit it lowered itself, which Apple allows;
  - behind an HTTP/2 or HTTP/3 peer's concurrent-stream limit.

  Today, a worker's request doesn't start until a worker is free, so its deadline never sees queuing. With async, the deadline runs from `resume` and includes it.
- **The core keeps that queuing rare, as best effort:**
  - The independent session's `HTTPMaximumConnectionsPerHost` is 32, the lane cap.
  - The core resumes at most 6 HTTP/1.1 exchanges per origin per lane, lifted once an exchange on that origin reports `h2` or `h3`, and restored when one reports `http/1.1`. The finisher applies each report (above).
  - Typical HTTP/2 stream limits (100) exceed the cap.
- **§5 measures it:** the gap between `resume` and the first transaction's start in task metrics, under "LTE".

**Deadlines.**
- One monotonic deadline per request, armed when its execution starts: the HTTP task's `resume`; for an `exactBodyFrom` request, the sizing reader's start; on the Linux adapter, when an adapter thread accepts the exchange (D4).
- **An `exactBodyFrom` request has two phases** (D5):
  - **Sizing:** a stat under a reader permit, armed with the request's deadline, as today's `acquire` arms it around the stat.
  - **If its whole charge fits the bytes free now,** the same deadline carries on through the read and the exchange, as today's one `Deadline::arm` covers stat, read and exchange.
  - **If it doesn't,** the job parks at the head of its lane (D5), and the deadline is **disarmed**: its heap entry is removed and the request's timer generation is bumped, so a firing already in flight finds a stale generation and does nothing. When the job starts again with its whole charge taken, the deadline arms afresh, at the new generation, for the read and the exchange.
  - **Declared change:** only a request that parked between its phases gets a fresh deadline for the second. The wait for bytes never counts, as a wait for a worker doesn't today.
- It covers the file read, every hop and the body. Waiting to resume a later hop doesn't restart or suspend it.
- The timer thread's heap entry is removed at the request's terminal transition. When the timer fires, it sets the request's deadline flag, aborts it, and posts the terminal event to the finisher. It does nothing else.
- **Terminal transitions**, one per request, by which happens first:
  1. The response completes successfully. The outcome is the response, even if the deadline fires afterwards.
  2. The caller aborts a cancellable operation (forget of a safe read or a stream, a newer answer, retirement) before the deadline fires. The outcome is Aborted, and a later firing is ignored (`executor_core.rs:1310-1316`). A forgotten write is not aborted; its transition is one of the others.
  3. The deadline fires first. The outcome is `Timeout`, and the task is cancelled. Its cancellation and any later abort are ignored.
  4. A stop is recorded: Refused for over-limit, Network for oversized headers. Its cancellation `end` is ignored.
  5. A reader fails (a stat or read error, a reader refused for a permit, a file grown past its charge): the outcome is the reader's, as `executor_body.rs` classifies it today, and D5's refusals.
  6. Otherwise the transport's error, classified as `fetch_failure` does.
- The idle timeout stays deadline + 1 s. A deadline that can't be armed is refused.

**Forget**, as `forget` does today (`executor_core.rs:512-561`): a queued safe read is never sent; a running one is cancelled, through the terminal event, and `complete` drops its outcome. **Every stream**, whatever its method (a POST SSE with `exactBodyFrom` included), is cancelled while preparing or open, as today each stream has an abort controller (`executor_core.rs:420`). Every other forgotten job is marked forgotten and nothing else. A queued write still executes. A running write, `Now`, storage or handoff runs to its own terminal transition, keeping the ordered slot until then, and `complete` drops the outcome. LLP 1041 §8.4: a write still runs, and keeps its slot until it ends.

**Retirement** aborts every HTTP exchange and stream, writes included, as today's shared abort does (`executor_core.rs:690-706`), and suppresses delivery. A closure or handoff already running runs to its end.

### D4. Linux and the render host: the same core over a bounded blocking adapter

**The adapter.** Off Apple, and in the render host by default on every platform (`host/render/src/executor.rs:23-31`), `start` comes from an adapter over ureq/rustls. The render host with `EXACT_RENDER_TRANSPORT=platform` on Apple (`executor.rs:32-40`) uses D2's sessions instead, and its executor has the platform transport's base set (below):
- The executor owns one `RustlsHttpTransport` per lane, so keep-alive and the render pool's warm connections survive (`host/render/src/lib.rs:413-417`).
- Each exchange runs on an adapter thread from the lane's pool, until `end`.
- The adapter's independent in-flight cap is **8**, not Apple's 32. Linux has no HTTP/2 to multiplex, and with typical ceilings the byte budget binds at seven anyway. The per-origin limit of 6 applies there too.
- **Physical availability is separate from the logical slot.** An HTTP job starts only when one of its lane's adapter threads accepts it; its deadline arms at acceptance. An `exactBodyFrom` job is accepted before its sizing: the thread is held through sizing and reading, under the one deadline, and then runs the exchange (D5). A park for bytes returns the thread, with the deadline disarmed as D3 says.
- **A parked upload re-starting** (D5) takes its charge and an accepting thread in one decision, and arms its fresh deadline then; if no thread can accept, it stays parked, deadline disarmed.
- **A job waiting for a thread** is in the core: not started, no deadline. It is a queued job to `forget`: a safe read is removed and never sent; a write, `Now`, storage or handoff is marked and runs when a thread accepts it.
  - Independent: the eight threads are the in-flight cap, each held to physical end, so a thread stuck in the resolver holds its slot until it returns.
  - Ordered: the slot ends at the terminal transition (D3), but the thread may still be blocked, typically in the resolver. The **ordered role**, and the base set's permit for it, belong to the lane, not to a thread. A thread still blocked after its request's terminal transition gives up the role and becomes a **straggler**, keeping its own permit.
    - **Replacement:** when an ordered HTTP job is ready, no thread holds the role, and the lane has fewer than **three** ordered adapter threads, the starter creates one from the process counter and gives it the role. Otherwise, the job waits.
    - **A returning straggler** takes the role back if no thread holds it, keeping its permit; otherwise it exits and returns its permit. So a live executor always keeps one ordered adapter permit, and at most two extra.
    - A straggler's return, and a permit returned to the counter, wake the starter (D3).
  - Handoffs, `Now` and storage need no adapter thread and keep D3's ordered rules.

**One process thread counter, `MAX_THREADS` = 384.** It counts every executor-owned thread from creation until it exits. Retiring threads keep their permits.

- **Each executor reserves its base set at `start`, all or nothing**, and creates those threads then:

  | | Platform transport (Apple; render with `platform` on Apple) | Adapter (Linux; render by default) |
  |---|---:|---:|
  | Lane starters | 2 | 2 |
  | Lane finishers | 2 | 2 |
  | Timer | 1 | 1 |
  | Disposer | 1 | 1 |
  | Ordered adapter thread | — | 1 |
  | Independent adapter threads | — | 8 |
  | **Base set** | **6** | **15** |

  - **Start is all or nothing.** The executor first checks atomically that the counter has room for its whole base set and reserves it. It then creates each thread, which takes over one permit.
  - If any creation fails, the permits of threads never created are returned at once, the threads already made are told to exit (each returns its permit as it exits), and the executor is disabled and refuses work, as today when `LIVE_WORKERS` is full. No permit is left without a thread.
  - Adapter threads are created eagerly with the executor and parked. An idle executor costs its base set, measured in §5.
- **Streams take permits from the same counter when they open,** by the transports selected, not the host: under the HTTP adapter (Linux, and the render host by default on every platform), one for SSE; under the rustls socket transport (Linux), two for a WebSocket (reader and writer, `websocket.rs:279`); under the platform's sessions and sockets (Apple; the render host keeps the platform socket even with the HTTP adapter, `Host::with_transport`), none, because a stream is a task. Each executor still admits 16 streams (`STREAMS`), as today. **Declared change:** a stream whose permits the process counter can't supply is refused at open, a new process-wide refusal.
- **`exactBodyFrom` readers** take a permit each and keep their own process-wide limit of 4. A reader that can't get a permit, at the limit or with the counter exhausted, is refused as today's fifth reader is (`exactBodyFrom: 4 file reads are still running (a stalled disk?); nothing was sent`, `executor_body.rs:127`).
- **The counter wakes its waiters.** A starter that needs a permit it can't get for an ordered replacement registers on the counter (streams and readers don't wait: they are refused); every permit returned, by any executor's thread, wakes the registered starters, which re-decide.
- **Arithmetic:** sixteen live Linux executors reserve 240 (Apple, 96), and their ordered stragglers can add at most 2 each, 32 more. Their streams can take up to 16 × 32 = 512, but rarely more than a few. Readers take 4. The 384 is the bound, so a process with many streams and retiring executors refuses streams before it starves an executor's reserved progress threads.
- **Resolvers.** A thread stuck in the resolver keeps its permit until it returns. An executor's reserved threads are its own, so another executor's stuck threads can't take an ordered release's adapter. Replacements for this executor's own stragglers come from the counter, never from the base set, and at most two are alive at a time.

**Alternatives:** a hand-written HTTP/1.1 reactor is rejected (no HTTP/2, our own framing and TLS bugs). tokio + hyper + h2 is adopted only if §5's Linux arm justifies HTTP/2 there, after a binary-size spike, behind the same `start`.

### D5. Byte budgets and counts: the caps, stated as `admit` and `next_job` compute them

**Admission predicates**, unchanged and exactly as `admit` (`executor_core.rs:367-390`):
- An independent request is admitted while `counts[1] - streams < 128`.
- An ordered **read** (a plain GET/HEAD without work) is admitted while `counts[0] - agains < 128`.
- An ordered **effect** (a write, storage, a continuation, a native call, work) is admitted while `counts[0] - light < 16` **and** `counts[0] - agains < 128`. The sixteen counts everything admitted except re-ask markers and finished reads, so reads may keep entering the 128 when sixteen are unfinished (`executor_again_tests.rs:81`).
- Re-ask markers have their own window of 128.
- A stream is admitted while `streams < 16`, charged its ceiling at once.
- Bytes are admitted while the charge fits the lane's budget, 512 MiB ordered and 64 MiB independent.

**Charges:**
- Waiting: the request's buffers.
- Running HTTP: request buffers + `max(2 × ceiling, 32 KiB)` + 2 × 64 KiB, as `reservation` computes.
- Running `Later` handoff: **no response bytes** (`next_job`'s `handed_off` → 0). It is charged what it retains when it completes.
- **Upload bytes (new, the lead's ruling 2026-10-10).** An `exactBodyFrom` request is charged its file's size, in addition, for the life of its fetch.
  - **Sizing phase.** The job starts with **no reservation** but its request buffers (its waiting charge). A reader (permit, deadline armed; on the adapter, after a thread accepted the job, D4) takes the size (`FsOp::Stat`, as `executor_body.rs:204-215`), posts it to the starter, and **waits for the decision**.
  - **Then the starter decides, all at once:**
    - If the whole charge, as `reservation` computes it plus the file, **exceeds the lane's entire budget**, the request fails: `Refused`, "the file and response ceiling exceed the lane's byte budget". It's an execution outcome, so an ordered one doesn't raise the fence.
    - If its whole charge doesn't fit beside the waiting charges of every job queued behind it in its lane, it fails at once (`Refused`, "the file and the requests queued behind it exceed the lane's byte budget"): those jobs can't run before it, so their buffers would never come back to it. *(Proposed; §6, question 8.)*
    - If it fits the budget but not the bytes free now, the job **parks** at the **head** of its lane with its size recorded: deadline disarmed (D3), the reader told to exit, no slot, no permit, no adapter thread, its waiting charge kept and nothing more. It starts again when the rest of its charge (running charge plus file, less the waiting charge it holds) can be taken atomically, and, on the adapter, in the same decision, an adapter thread accepts it (D4); only then is its fresh deadline armed. No two uploads can each hold part of what the other needs. **While it is parked, admission to its lane counts its outstanding charge as taken**, so a request admitted later can't crowd it out; one that doesn't fit is refused at admission, as today when the bytes don't fit. *(Proposed; §6, question 8.)*
    - Otherwise the rest is taken, and the second phase starts: the reader that sized it, under the same permit (after a park, a new reader, and on the adapter a newly accepting thread; the same deadline, or a fresh one after a park), reads **at most the charged size**, posts the bytes to the starter, and exits. A refusal tells the reader to exit. The starter then resumes the HTTP task. A file grown or replaced past the charged size fails the request (`Refused`, "the file changed while it was read").
  - **A file-backed stream** (`exactBodyFrom` on SSE, D7) never parks: a stream never waits for bytes. It is admitted, as any stream, with its ceiling charged at once or refused (`executor_core.rs:369`). Its reader then sizes the file, and the starter takes the file's charge at once, or the stream fails (`Refused`, "the stream's file does not fit the lane's free bytes"; over the whole budget, D5's first refusal). When the charge is taken, the reader reads the file, posts the bytes, and the starter resumes the stream's task. **Declared change:** a file-backed stream can fail under byte pressure after admission. It keeps the stream count throughout, and its file charge ends as an upload's does.
  - **The body** is one owned buffer, shared by reference across 307/308 hops and lent to URLSession without a copy (D2).
- Completed, as `complete` does:
  - An ordered completion keeps what it retains.
  - An independent completion **of any kind, success or failure, keeps its running charge less its upload bytes until drained.**
  - A forgotten one keeps nothing past its physical end.
- What ceiling and upload fit is whatever `reservation`, plus the file, finds within `BYTES[lane]`. The prelude's 1..=64 MiB is only the annotation's validation range.

**Every charge ends at the later of its logical and physical release.** A charge's logical release is the table's: drain for an independent outcome; for an ordered one, its shrink to what the outcome retains, then drain. Its physical release is the physical end of whatever still holds those bytes: a cancelled task's partial response and buffers, a reader. Until both have happened, the charge stays. So a timed-out cohort that has been drained can't lend its bytes to the next cohort while its cancelled tasks still hold them. An ordered outcome shrinks to what it retains only after the physical end.

**Upload bytes end** at the later of (a) the physical end of the last hop that sends the buffer, or the terminal transition if no such hop is outstanding, and (b) the lent buffer's last release (D2). A 303, or a 301/302 of a non-GET/HEAD, drops the body, so the hop before it is the last that sends the buffer. A redirect refused at its grant check, or a timeout while the next hop waits for the in-flight cap, leaves no hop outstanding.

**The in-flight cap** counts **resumed HTTP exchanges** on the independent lane: 32 on Apple, 8 on the adapter.
- Not counted: handoffs (`native.later` calls are bounded by the 128, as today), `Now` work, undrained completions, and streams.
- Over the cap, a job waits, never refused. The ordered lane has its single blocking slot and no cap.

**Release table.** Each reservation has one release event per path. "Interested" means the ticket is still held (not forgotten).

| Path | In-flight slot | Response charge | Upload bytes | Ordered blocking slot |
|---|---|---|---|---|
| Success, interested | physical end | later of physical end and: independent, **drain**; ordered, what it retains until drain | the upload rule above | terminal transition |
| Failure after an exchange began (transport error, oversized headers, timeout, over-limit), interested | physical end (the cancelled task's last callback) | later of physical end and: independent, **drain** (the running charge less upload, as `complete`); ordered, what it retains until drain | the upload rule above | terminal transition |
| A redirect refused at its grant check (`DeniedRedirect`), or a timeout while the next hop waits for the cap, interested | the old hop's physical end | as the row above | the upload rule above: no hop outstanding | terminal transition |
| A safe read forgotten or aborted by the caller | physical end | physical end | the upload rule above | terminal transition |
| A write, `Now`, storage or handoff forgotten while running | physical end | physical end | the upload rule above | its own terminal transition |
| A redirect hop (`Flow::Skip`) | the old hop's physical end, before the next hop's slot is taken | carried to the next hop | carried to the next hop if it sends the body; else the upload rule above | held across hops |
| A parked upload (sized; its whole charge doesn't fit the free bytes) | not taken | its waiting charge only | none taken yet | none (it stays the head) |
| Failure before any exchange (grant refusal, an upload that can't fit, a sizing or reading failure), interested | never taken | independent: **drain**; ordered: what it retains, until drain | the reader's exit (sizing holds none) | terminal transition |
| A reader still blocked after its request's terminal transition | never taken | as the row above, and not before the reader's exit | the reader's exit | terminal transition |
| A file-backed stream refused after sizing, interested | never taken | later of the reader's exit and **drain**, as any independent failure | none taken | — |
| Retirement | each task's physical end, after `invalidateAndCancel` | with the executor | each task's or reader's physical end, and the buffer's last release | with the executor |

Physical end is the last transport callback (`didComplete` on Apple, the adapter thread's return on Linux), the closure's return, or the reader thread's exit.

### D6. Preconnect: a measurement flag, default off

URLSession has no public preconnect into its pool, so a request-free warm-up is outside the pool by construction.

- **What the flag does:** at `Executor::start`, a Network.framework TLS connection to one **named consumer origin** (the manifest names it; for the clone, `public.api.bsky.app`). It closes after the handshake or 2 s. No HTTP is sent, and first frame doesn't wait on it.
- **What it can't do:** enter URLSession's pool; promise URLSession's network path (a raw connection may skip the system proxy or VPN); or hide from the origin, which sees a ClientHello.
- **Acceptance:** cold `zero_to_post` under "LTE" improves by more than C's IQR, **and** the first independent task's `domainLookup` and `secureConnection` durations fall. Otherwise the flag is deleted.

### D7. Streams keep a session each

- **SSE.** Each SSE stream has its own session, as each has its own transport today. It runs as an asynchronous task through `FetchTask` (grants, scoped grants, `exactBodyFrom`, redirects under the grant check).
  - The stream is started by the lane's **starter** (D3).
  - **Framing runs in the data callback.** It is bounded CPU work: the callback splits lines into events, each bounded by the per-event ceiling, and publishes each through `message()` into the per-ticket coalescing slot (LLP 1016.000 D4) under a brief state lock. So unparsed bytes never exceed one event's ceiling, as `read_events` (`executor_stream.rs:235`) works today.
  - Only a **2xx `text/event-stream`** response enters the event parser. Any other response is one ordinary, body-bounded outcome, as `executor_stream::open` (`executor_stream.rs:191`). A 404 JSON body doesn't become an empty stream.
  - The ceiling is **per event** (`executor_stream.rs:184-185`), and coalescing is unchanged (LLP 1016.000 D4).
- **WebSockets** keep a session each and use D1's asynchronous socket interface, with redirect refusal, `maximumMessageSize` = ceiling + 1, text only and today's close rules. Closing one invalidates only its own session.
- **The socket's terminal taxonomy** is today's (`read_socket`, `executor_stream.rs:125`):
  - the ceiling is passed to `start_socket`;
  - `closed` carries a typed reason: `Refused` for a binary message or a local over-ceiling message, `Network` for a peer close (with its code, 1009 included) or a transport failure, `Aborted` for a local abort, and a failure before opening as the handshake's error.
- On Linux, streams keep their threads, taking permits from D4's counter.

### D8. What runs where

- **Delegate callbacks and the Linux adapter's reads** do bounded CPU work only:
  - `head`'s decisions (declared length, header caps, `Flow::Skip`), which return the disposition synchronously, so no body byte arrives before a stop;
  - appending bytes, bounded by the ceiling;
  - SSE framing (D7);
  - recording the protocol from metrics, under the exchange's own lock.

  A finished exchange is enqueued to the lane's finisher. Callbacks never wait: they do no I/O, run no closure, block on no buffer, and take the state lock only briefly for `message()`.
- **The state lock** guards decisions and bookkeeping only: no transport call, closure, storage, file I/O, destructor of opaque work, DNS or outcome construction.
- **The starter** creates sessions, resumes tasks, starts streams and readers, and runs handoffs, `Now` and storage.
- **The finisher** builds and publishes outcomes.
- **The disposer** destroys opaque work.

## 3. What stays as LLP 1041 says

Each line is a test today. The semantic assertions are kept and run against the new core on a controllable asynchronous fake transport and on the real ones. Assertions about the removed mechanism become their equivalents; for example, the six-concurrent test (`executor_tests.rs:210`) becomes "the cap's number concurrent".

- **Ordered:**
  - one blocking job at a time, its slot released at its terminal transition; FIFO execution;
  - a handoff runs only while no blocking job is in flight, frees the slot when it returns, and settles in place;
  - FIFO settlement, one completion or refusal per pump, lane alternation, coalesced wakes;
  - `counts[0] == 0` is idle.
- **Refusals and the fence:** retained refusal positions; the fence's own 128 tickets / 64 MiB; host-controlled `resume_ordered`.
- **Read queue and re-asks:** the amendment's counters and predicates (D5), marker-only completions, de-duplicated waiting records, placement after fenced work. Markers start no exchange.
- **Forget:** queued safe reads never sent; running safe reads and every stream cancelled; undrained outcomes dropped; admitted queued writes still execute, including one waiting for an adapter thread; a running write keeps its slot until it ends (D3, D4).
- **Retirement:**
  - every HTTP exchange and stream is aborted, writes included, and delivery of everything else is suppressed;
  - a closure already running or a handoff already made runs to its end, as today;
  - nothing is joined on the UI thread, and wakes stop.
- **Limits:** `exactTimeout` as D3, with the declared change that any platform queuing after `resume` counts against it (a per-host limit Apple lowers, an HTTP/2 or HTTP/3 stream limit); `exactBodyFrom` as D3 and D5, with uploads charged; ceilings while receiving; the 4 MiB request cap; header caps, with the declared per-hop change (D1).
- **Grants and redirects** as D1.
- **Declared changes, collected:**
  - header checks on every hop (D1);
  - any platform queuing after `resume` counts against the deadline (D3);
  - a request that parks for bytes between its upload's sizing and reading gets a fresh deadline for the second phase; otherwise one deadline covers both (D3);
  - a file-backed stream whose file can't be charged at once is refused after admission (D5);
  - a long independent `Now` delays the lane's starts (D3);
  - a process-wide stream refusal (D4);
  - the independent lane passes over a head at its origin's limit (D3);
  - an upload that can never fit is refused, and growth while reading fails (D5);
  - *(proposed, question 8)* an upload whose whole charge can't fit beside the requests queued behind it is refused at its park, and while an upload is parked, admission to its lane counts its outstanding charge as taken (D5).
- **Independence is never inferred:** only explicit independent scheduling metadata selects the lane (the TypeScript `exactIndependentHttp`, Rust constructors, the native-call path). GET alone never does.

## 4. Migration and tests

**As built, stage 1 (2026-10-12).** Stage 1a is ibex2's asynchronous fetch: `fetch_async.rs` (`Sink`, `Flow`, `Upload`, `Exchange` with an explicit activation, `FetchTask`, and a `BlockingAdapter` over any blocking `Transport`) and Apple's `DarwinLane` (`darwin_async.rs`, `darwin_task.mm`). Stage 1b is the core, `host/apple/src/executor_async.rs`, under the existing admission, queues, settlement and fence of `executor_core.rs`. Where it differs from the text above, until the stages that finish it:
- **A switch.** `EXACT_HTTP_ASYNC=1`, at build or in the process's environment, runs the machine; without it the workers run, unchanged. It stays off until §5's gate passes. Only `Executor::start` (Apple's) takes it; the render host's `start_on` keeps the workers.
- **Threads.** The six machine threads are reserved at `start` from `LIVE_WORKERS` and created at the first request. The October 13 adoption also reserves the bounded portable adapter's one ordered and six independent workers (the latter follows `EXACT_HTTP_WORKERS`), created on their first transport reservation and charged until accepted work returns. Linux/render activation and the broader `MAX_THREADS` rollout remain stage 2.
- **Streams** keep their own threads and transports (stage 3), so the file-backed stream rule of D5 waits for them.
- **Scoped grants** use the lane's transport with the scope's grants.
- **A redirect's next hop** waits only for the in-flight cap; it counts toward its origin's HTTP/1.1 count when it starts.
- **Correctness follow-up (2026-10-13; implemented and independently reviewed).** This closes the still-listed stage-1b findings from Astra and Grok's October 12 round 4. The production switch remains off; these repairs do not waive §5 or authorize blind retry of writes.
  - **Upload progress:** one unsized file per lane is selected until its size is decided or its reader ends. Every successor therefore stays in the queue counted by the park-refusal test. A parked upload reserves its future bytes wherever it is in the queue, including behind an origin-blocked head.
  - **Upload ownership:** the read result becomes the charged `Upload` before its first exchange is queued. A timed-out start awaiting disposal retains that charge until the last buffer owner releases it and the sending hops physically end. Upstream's last-release callback must run after the allocation is freed; the follow-up Ibex pin supplies that ordering fix.
  - **Retirement and panics:** the active reader owns its shared-retirement registration independently of the disposable file job. Reader and starter physical holds have unwind guards; failure to spawn a reader and unwinding sizing/reading or preparation/activation panics publish a terminal outcome and leave the lane usable. The native app profiles use `panic=abort`: an aborting panic terminates that process and cannot be caught by these guards. The tracked sink owns a prepared hop's physical hold even when its transport refuses or unwinds.
  - **Destruction:** stale redirect starts, duplicate outcomes, forgotten outcomes, and a parked file job's retired transport/abort handles go to the disposer. Posting an event releases the finisher-channel lock before handing over or dropping the event. No opaque stale capture is destroyed under the state lock.
  - **Regression evidence:** `executor_async_tests.rs` covers blocked disposal with a charged queued upload, retirement after file-job disposal, injected reader spawn refusal and sizing/reading panics, transport preparation/activation panics in both lanes, a sending hop surviving buffer release, and parking behind an origin-blocked head. `executor_async_file.rs` checks the two-upload sizing deadlock boundary and stale-hop/duplicate-outcome disposal off the state lock. The executor-core run passes 92 tests with 9 explicitly manual cases ignored; its nested async-core replay passes 71 with 9 ignored. The fault hooks are per-machine test-only hooks; production scheduling has no new test dispatch path.
- **The executor's suite** runs again on the machine (`the_suite_passes_on_the_asynchronous_core`), and `executor_async_tests.rs` adds the cap, the per-origin pass-over and its h2 lift, the ordered slot at a timeout, the later-of release, a forgotten write, and question 8's park and refusal.


Real-file upload tests in the worker and async suites share one test guard,
matching the process-wide four-reader limit. Otherwise unrelated tests can
consume that capacity and turn an expected timeout into an admission refusal.
Each test keeps its own concurrent-reader assertions; other tests run in parallel.


There are no shims; each step deletes what it replaces in the executor.

1. **ibex2.**
   - Add `start`/`Sink`/`Exchange`, `Flow::Skip`, `FetchTask`, Darwin task delegates and per-session delegates, and the asynchronous socket.
   - Port the redirect, grant, declared-length, cookie/cache and reuse tests to the asynchronous form; the blocking form keeps them too.
   - New tests:
     - a task delegate without `willPerformHTTPRedirection` fails;
     - declared length refused before a 3xx is followed;
     - a 3xx body skipped while the next hop's refusal is `DeniedRedirect`;
     - late events after a stop and after a hop;
     - over-limit vs cancel classification;
     - oversized headers on an intermediate hop (the declared change).
   - Add the Linux/render adapter over one executor-owned `RustlsHttpTransport` per lane.
2. **The core.** Lane starters, the timer, reservations and charges as D3–D5. Every semantic assertion in `executor_*_tests.rs` is kept, with fixtures moved to a controllable asynchronous fake. New tests:
   - an ordered HTTP fetch stays alone across its redirects and file read while a `Later` handoff is outstanding;
   - a queued run of handoffs then a blocking job, with the blocking job starting without another event;
   - a handoff queued behind a blocking job waits for that job's terminal transition, not its physical end: the job times out while its task withholds `didComplete`, and the handoff starts at once;
   - the independent cap full while an ordered release still starts and completes;
   - three outstanding ordered handoffs don't block an ordered HTTP job (no response bytes charged);
   - an independent `Now` doesn't wait behind ordered storage;
   - a synchronous handoff `Reply` inside the call;
   - inline failure and abort-before-start;
   - each terminal-transition order in D3;
   - more HTTP/1.1 exchanges to one origin than 6: the seventh waits in the core with no deadline running, a later job to another origin starts, and the ordered lane's request to the same origin is unaffected; after the origin reports `h2`, the lane cap governs; after a later `http/1.1` report, the limit is 6 again;
   - upload bytes charged and released at physical end; an upload whose charge exceeds the lane budget fails at once and doesn't stall the lane; two gated uploads that each fit but not together, with no deadlines, both completing in turn; a file grown past its charged size fails; one buffer across a 307, lent without copy;
   - a 1 KiB `exactBodyFrom` that fits at once: the reader's result wakes the starter and the body is sent under one deadline; the same upload parked behind undrained bytes, with a stale timer firing after the park ignored, then starting with a fresh deadline when the bytes drain; an ordered upload parked at the head, with nothing behind it starting until it does;
   - two file-backed SSE streams (12 MiB ceilings, 20 MiB files) both admitted before either is sized: together they hold 48.25 MiB, so both are refused after sizing, promptly, and their charges return at drain; one admitted, sized and charged before the other's admission: the second is refused at admission;
   - `forget` of a POST SSE stream with `exactBodyFrom`, while sizing and after opening: cancelled, its charge, count and upload released;
   - each release as the only event that can make progress, the starter otherwise idle: a timed-out exchange's `didComplete` after its drain; the lent buffer's deallocator after `didComplete`; a straggler's return; another executor's thread returning a permit to a full counter; the ordered slot at a timeout, with a handoff queued behind it;
   - four uploads sizing at once with every reader permit taken: each proceeds to read on its own reader;
   - drain-before-cleanup and cleanup-before-drain for a timed-out independent exchange: the charge returns only after both; the ordered equivalent shrinks to its retained outcome only after physical end;
   - a redirect refused at its grant check and a timeout while the next hop waits for the cap, each releasing the carried upload at the terminal transition; a 303 after a POST ending the upload charge with the first hop; the lent buffer outliving `didComplete` on a cancel, with the charge held until its release;
   - `forget` of a running ordered POST: the next ordered job doesn't start until the POST ends, and the outcome is dropped; `forget` of a running safe read: cancelled, Aborted, the slot released;
   - a protocol report arriving after `didComplete` updating the origin table through the finisher;
   - on the adapter, an ordered fetch timing out while its thread is blocked in the resolver, then another ordered fetch and a handoff: the fetch starts on a replacement thread with its deadline armed at acceptance; with three ordered threads alive or the counter exhausted, it waits, unarmed, and `forget` removes it if it is a safe read and keeps it if it is a POST; the straggler returning with no successor queued takes the role back, and an ordered fetch after a full counter still starts on it;
   - on the adapter, an `exactBodyFrom` POST whose deadline doesn't run while it waits for a thread: accepted, sized, read and sent under one deadline; the same POST parked, its bytes freed while no thread can accept: no deadline fires, and when a thread accepts, one fresh deadline covers the read and the exchange;
   - Astra's round-7 sequence, no timeouts: X running (1 MiB ceiling), upload A (61 MiB file, 1 MiB ceiling) parks behind it, POST B (3 MiB inline body) arrives behind A and is refused at admission, X ends and drains, A starts; and the same with B admitted before A's park: A is refused at its park;
   - every row of D5's release table;
   - a long independent `Now` while completions and a timeout still publish through the finisher;
   - a transport that withholds `end`, a blocked reader and a blocking destructor, while another request's timeout publishes on time;
   - an ordered request whose reader stalls: its timeout publishes and the next ordered job starts;
   - metrics delivered after `didComplete` still lifting the origin limit;
   - executor start failing partway through creating its base set;
   - retirement with in-flight tasks and late `didComplete`;
   - `exactBodyFrom` settling while its reader is blocked;
   - repeated executor replacement within `MAX_THREADS`, and stuck adapter threads not taking another executor's reserved threads.
3. **Streams.** SSE as tasks with per-event ceilings and the non-event-stream branch. WebSockets on the asynchronous socket. Tests: 16 held SSE streams to one HTTP/1.1 origin; sibling WebSocket close; a stream refused when the process counter is exhausted.
4. **Integration.** The JS shared-load and re-ask integration tests, the Linux and render suites, and the ignored real-platform tests run explicitly. HTTP/2 multiplexing is checked against a controlled local HTTP/2 endpoint ("N requests ride few connections", not a universal promise).
5. **Preconnect** (D6) behind its flag, decided by §5.
6. **The Completion Storm** 128-lane drive on macOS, Linux and the web.

## 5. Measurement plan

**Definitions.**
- **"Cold"** is `measure.py --only cold`: process terminated, the app's HTTP cache and `app:/cache` emptied, and system DNS not flushed (the simulator can't without root). Order effects are controlled by interleaving.
- **Preconnect is a separate experiment, not one of the interleaved arms.** There are 12 C and 12 C+P runs in a randomized order fixed in advance, with a **simulator reboot before every run**. The reboot, the same in both arms, is the initial-state control. The cold check is made on the **C runs only**, where nothing touches DNS before the first task: each must report `domainLookup` > 0 and no reused connection. That is evidence of a cold start, not proof of an empty cache (Apple's `domainLookup` is the lookup's duration). If a C run fails it, the experiment's cold-cache claim is reported as inconclusive; the run is not replaced. On C+P a short lookup is the treatment's intended effect, so it isn't checked there. The failure rule (gate 5) applies.

**Arms.** Interleaved, 12 runs each, the Bluesky clone signed out, bsky4's `tools/perf/measure.py`, under the host's Network Link Conditioner "LTE":
- **A7:** today's executor rebuilt with 7 independent workers.
- **C7:** async, with the independent in-flight cap = 7.
- **A6:** today, 6 workers.
- **C:** async, cap 32.

**Gate for the Apple change** (primary outcome: cold `zero_to_post`):
1. C7's median beats A7's by more than A7's IQR, and C7's worst isn't worse than A7's worst. This isolates the shared session and HTTP/2 from "more requests at once".
2. **The mechanism metrics show it.** Connection identity comes from the transaction metrics' local and remote endpoints (`localAddress`/`localPort`) per request, and on the controlled local HTTP/2 server from its connection ids. In C7, the feed reads' transactions overlap in time on **one connection per origin** with `h2`/`h3`. In A7, they ride several. Connections opened per launch are reported for every arm.
3. **C is not worse than C7:** C's median is not worse than C7's by more than C7's IQR, and C's worst isn't worse than C7's worst.
4. **C is not worse than what ships:** C's median and worst are each no higher than A6's. Gates 1–3 can hold while C regresses against today's executor; this one can't.
5. **Failures are outcomes, not exclusions.** Every run must complete all 19 feed reads. This applies to the interleaved arms, the preconnect experiment and the Linux comparison alike.
   - A failed run in a treatment arm (C7, C, C+P, the Linux adapter) **fails that arm's gate** and stops its own comparison, whatever the control arms did. The interleaved arms, the preconnect experiment and the Linux comparison are separate comparisons: a C+P or Linux failure doesn't stop the Apple gates.
   - A failed run in a control arm (A7, A6, the C runs of the preconnect experiment, today's Linux executor), with every treatment run complete, makes the comparison **inconclusive**. The whole sequence is rerun once, and both sequences are reported. A control failure in the rerun is reported as inconclusive to Charlie.
   - No individual run is replaced or excluded.

**Recorded in every arm, not gating:**
- `connect_to_post`;
- the span of the 19 feed reads;
- active exchanges over time and byte-budget stalls;
- peak threads and resident memory during the data load;
- main and owner CPU during the load;
- `fling-discover` page latency and late frames;
- unthrottled ("LAN") runs for the simulator floor;
- the **idle-executor cost**: threads and resident memory of a launched app that makes no request (no network), per arm.

**Also:**
- an HTTP/1.1-only local origin, to exercise the per-origin start limit and ordered isolation;
- mixed response sizes, a cancel-churn run, and an upload run;
- one physical-device cold start, if a device is available;
- signed-in Following by bsky4's worker.

**Gate for the Linux adapter**, against today's six-worker Linux executor, 12 interleaved runs:
- cold load median not worse by more than A's IQR, and worst not worse;
- the Completion Storm drive passing LLP 1041's checks;
- peak executor-owned threads at or under the bound: 15 × live executors + extra ordered adapter threads (at most 2 per executor) + SSE streams + 2 × WebSockets + readers running + threads of retiring executors not yet exited, all under 384;
- RSS growth over A's at most 8 MiB per executor, with idle and loaded runs both reported;
- warm pooled-render page time: median not worse than A's by more than A's IQR over 12 pages;
- tiny-ceiling saturation at the cap of 8, with no ordered stall.

This decides the adapter, not HTTP/2 on Linux; that would need D4's spike first.

**As measured, the Apple gate (2026-10-12): failed; `EXACT_HTTP_ASYNC` stays off.** Three LTE windows of the four interleaved arms, 12 cold launches each (`measure.py --only cold`, the clone signed out at its c5e6e50, simulator x2-perf2). LTE was pf/dummynet pipes on the resolved API addresses (anchor `com.apple/exact-lte`, the lead's root-owned scripts), standing in for the Network Link Conditioner; every exchange with a recorded remote reached a shaped address. The arms are the stage-1c builds (`EXACT_HTTP_WORKERS`, `EXACT_HTTP_IN_FLIGHT`, 0612b2087); the C arms also set the switch at launch, and from window 2 on every block's connection log (`IBEX2_HTTP_METRICS`, one line per exchange, written when its metrics arrive at completion) shows the transport its arm names.
- **The windows.** Window 1 is clean except C7, which ran the workers (a cached build lost its baked switch; the check above was added after). Window 2 is **contaminated**: another agent ran simulator flings, three iOS builds and an all-processes Time Profiler for all of it (05:07:30–05:17:23Z). Window 3 (06:18–06:26Z) is clean, the machine idle but for one transient system process, and is the only clean C7. The clean data are windows 1 and 3.
- **`zero_to_post`, ms, median (IQR, worst):**

  | window | A7 | C7 | A6 | C |
  |---|---|---|---|---|
  | 1 | 1452 (137, 1715) | (ran the workers) | 1508 (248, 2351) | 1481 (194, 2002) |
  | 2, contaminated | 1537 (198, 3337) | 1498 (103, 1775) | 1543 (188, 1984) | 1521 (678, 16505), 2 failed |
  | 3 | 1630 (284, 2810) | 1620 (110, 1943) | 1624 (128, 1955) | 1602 (78, 1932) |

  From window 1 to window 3 the medians rose 178 ms (A7), 116 (A6) and 121 (C); the cause isn't known. Within window 3 only A7's first block is high (1770, against 1613 and 1634).
- **Verdicts, window 3 (window 1 where it can say):**
  1. **Fails.** C7 beats A7 by 10 ms, against A7's IQR of 284; C7's worst (1943) is better than A7's (2810). Sharing a session and HTTP/2 doesn't buy a measurable launch on this path.
  2. **Not shown.** In windows 2 and 3, every exchange with a recorded endpoint rode one `h2` connection per launch under C7 and C, seven under A7 and six under A6 (window 1's C also one). Two window-2 C launches are partly unrecorded: the failed one logged no endpoint or protocol for any of its 12 exchanges, and the 16.5 s one none for 6 of its 27 (`getServices` and the first five feed reads; the other 21 share one connection). The log has one time per exchange, at completion, so that the feed reads overlap on the one connection, which the gate asks for, is not measured.
  3. **Fails with gate 5:** C's failed run stops C's comparison (the rule above). Window 3's numbers alone would pass: C 1602 against C7 1620 (IQR 110); worst 1932 against 1943.
  4. **Fails with gate 5,** likewise. Windows 1 and 3 alone would pass: C 1481 and 1602 against A6 1508 and 1624; worst 2002 and 1932 against 2351 and 1955.
  5. **Fails for C.** Windows 1 and 3 have no failed run (one window-1 C launch logged 27 exchanges, one pagination read fewer, with all 19 feed reads). But window 2's C failed two launches, and by the rule above a treatment failure fails that arm's gate; contamination doesn't excuse it. One painted nothing (the null in its block); the other painted at 16.5 s with 14 of its 19 feed reads, five author feeds never connecting and not retried (it is in the table's median, IQR and worst; without it, C's window 2 is 1513, IQR 186, worst 3050).
- **In sum:** in the clean windows' gated pairs async was not slower (C7 against A7, C against A6 and C7), and in window 3 its spread was the narrowest (C's IQR 78, C7's 110, against 284 and 128); but the primary outcome, gate 1, is a tie well inside the noise, and C failed two launches in window 2.
- **Finding: connection setup did not complete before the deadline (window 2).** The failed launch waited 15 s, the request deadline (`data/atproto.ts`), then logged clustered `NSURLErrorCancelled` (-999) completions. Those twelve records carry no endpoint or protocol. They establish a setup stall and shared cancellation timing, but do not establish how many physical connections existed or its cause. The earlier “one connection, one point of failure” description was an inference, not a measurement. The 16.5 s launch (`connect_to_post` 16263 ms) may share the cause; its five unconnected feed reads do not prove that. No retry or speculative duplicate request is introduced on that evidence.
- **Remaining qualification:** stage 2 (Linux), the preconnect experiment, the non-gating recordings other than `connect_to_post` and the connection log (the feed reads' span, active exchanges and byte-budget stalls, peak threads and memory, CPU, `fling-discover`, LAN runs, the idle-executor cost), and the physical-device network/UI gate. The October 13 stage-1b repairs have completed independent code review (§4). Results: `~/x2p2/gate/lte-{1,2,3}` on the measuring machine.

## 6. Questions for the ruling

1. Independent-lane order stays FIFO, except that a head at its origin's HTTP/1.1 limit is passed over so later origins start (D3)?
2. Linux: the bounded adapter now, tokio/hyper only on evidence after a size spike?
3. Preconnect: a default-off measurement flag with D6's acceptance rule?
4. Sessions: one per lane per executor, not per process?
5. The declared change in D1: oversized headers on an intermediate redirect hop now fail the fetch?
6. The numbers: Apple's in-flight cap of 32 (also the independent session's per-host connection limit), the adapter's 8, `MAX_THREADS` 384?
7. The declared changes collected in §3?
8. **A parked upload's liveness** (round 7, Astra B1): refuse an upload at its park when its whole charge can't fit beside the waiting charges of the jobs queued behind it, and while it is parked, count its outstanding charge as taken at admission to its lane? The alternatives are a bypass of the parked head for jobs whose waiting buffers it needs (breaks FIFO execution), or refusing such an upload whenever any later admission would crowd it (less predictable).

## 7. Known gaps this RFC doesn't close

- Opaque Rust closure captures remain count-bounded only (§8.4).
- A handed-off outcome can retain more than the lane's budget when it completes, as LLP 1041 already records (`llp/1041-graceful-overload.rfc.md:689`).

**Controlled setup-stall follow-up (2026-10-13).** The ignored Exact test
`native_tls_setup_stall_observes_connections_deadlines_and_an_unretried_write`
uses the real `DarwinLane` and executor, with a loopback peer that accepts TCP
and retains TLS ClientHello without replying. Nineteen independent reads have
the original 15-second deadline. Cold-origin admission allows six requests;
activation-relative deadlines produce terminal waves of 6 / 6 / 6 / 1 at
15.35 / 30.35 / 45.35 / 60.36 seconds. The peer accepts **four TCP connections**,
one per wave: URLSession coalesces the six tasks waiting for setup. An ordered
POST to a healthy loopback origin completes exactly once during the stall; an
independent GET succeeds afterward, and every charge returns. This supports the
setup-sharing mechanism under a controlled failure, not the cause of the old
LTE failures or a performance claim. The run used macOS 27.0.1 on a loaded host.
The first diagnostic harness had incorrectly allowed only 25 seconds for all
nineteen activation-relative deadlines; that failed expectation and the corrected
run are retained in the follow-up run ledger. Production deadlines are unchanged.

The historical `bsky4` commit `c5e6e50` and its `tools/perf/measure.py` are absent
from the available local, Air and Studio checkouts. The older local Bluesky
checkout has a Rust-only data module despite `rust.mode=off`, so using it would
require an app migration and would not reproduce the specified baseline.
Charlie assigned the historical `bsky4` benchmarking and rollout campaign to
the agent on Tuft on October 13. That lane owns recovering the original fixture
and measurement script, the four interleaved 12-run LTE arms, HTTP/2 transaction
and connection-overlap evidence, and the remaining §5 recordings. Its handoff
includes the controlled stall's four connections and 6 / 6 / 6 / 1 terminal
waves, the exact activation-relative deadline semantics, and the prohibition
on speculative replay of writes. Current included-app device smoke and these
controlled diagnostics cannot satisfy that gate. Independent Codex and Grok
code reviews cleared the current stage-1 implementation and pin adoption at
Exact `56f107fa4`; their verdicts are retained in the consumer-release run
ledger. The default remains off until the external campaign passes §5. These
reviews do not complete the broader Linux/render, thread-cap and stream stages.

## Ibex consumer adoption, 2026-10-13

The transport interface now comes from upstream Ibex's asynchronous fetch
implementation. A transport reserves capacity before accepting an exchange;
a refusal releases Exact's origin, upload and lane accounting without waiting
for a callback from work never accepted. The first accepted activation arms
Exact's one real-time deadline. Upload sizing and parking still precede it,
and redirects reuse it. Logical outcome, physical hop retirement and final
upload-buffer release retain their separate accounting.

The portable fallback uses Ibex's bounded worker service instead of spawning
one thread per exchange. Exact still owns lane policy and its numerical
budgets. The render host explicitly selects `portable-http`; Apple's native
transport stays the default. This adoption does not select asynchronous mode
by default or replace this RFC's production comparison.

The vendor refresh also imports Ibex's borrowed clock/cycle primitives.
Exact's modules still expose neither ambient timers nor sockets: the runner
owns application clocks and stream handles (LLP 1027.000 and 1016.000).

The portable worker materializes one additional upload buffer. Its lane therefore
reserves twice a file upload's length (and the extra inline body length) before
starting it, with checked size arithmetic. Native Apple uploads retain the shared
buffer once. Both charges stay until physical retirement and the last buffer
release; the portable worker queue also has an explicit capacity.

The old vendored process-wide metric-file hook is now Exact's optional
`executor_metrics.rs` observer of asynchronous hop callbacks. It preserves the
`IBEX2_HTTP_METRICS` file selection for that lane. Blocking/native comparisons
use Ibex's explicit diagnostic; a file from only the async observer is not a
qualified two-arm comparison. The production async switch stays opt-in.

Portable fallback pools start lazily at their first reservation: one ordered
worker and the configured independent-worker count (six by default). Their
slots count alongside the six machine threads under Exact's process limit, and
each actual worker owns its slot until its thread exits, including idle retirement
and partial pool construction. Failed pool construction is terminal for that lane;
it cannot retry using slots still held by a previous attempt.

If any base thread cannot start, the failed core releases every unstarted
portable reservation immediately. Already-started base threads keep their own
slots until exit. The lifecycle witness injects failure at each of the six
base positions while retaining the failed cores, then checks lazy startup,
physical pool retirement and refusal to retry failed pool construction.


## Physical iPhone follow-up, 2026-10-14

The isolated Conduit fixture was built from Exact `a01c62241` with
`EXACT_HTTP_ASYNC=1` and the published V5 Hermes bundle, then signed and installed
on a physical iPhone 17 Pro Max (`iPhone18,2`), iOS 27.0.1 (24A446). Its first
launch was refused while the phone was locked. After a normal unlock it
launched, but the test-control socket could not reach the Mac's selected IPv4
endpoint (`No route to host`). The fixture was then rebuilt normally with the
documented `host.ios.localNetworking=true` option. Its next launch did not
establish the agent connection before the readiness timeout. That is a control
channel failure, not evidence of an HTTP transport failure or a successful
anonymous-network UI drive. No rerouting or device security setting was changed.

The prepared storage fixture has the same documented local-network option;
its UI write/read and same-store restart remain unqualified while the control
connection is unavailable. The separate signed ImageIO harness needs no LAN
control channel and passed all six bounded physical cases recorded in
LLP 1069.002's October 14 qualification. Those native image-service results
neither stand in for the missing network/storage UI proof nor pass the LTE
gate. Tuft still owns the measured rollout campaign, and the async default
remains off. The broader stages and Draft status are unchanged.

## Physical iPad follow-up, 2026-10-15

The prepared signed Conduit and storage fixtures from Exact `a01c62241` were
exercised on a physical iPad Pro 12.9-inch (5th generation, `iPad13,11`),
iPadOS 27.2 (24B5089g), after normal Trust and Developer Mode setup. Their
binary hashes match the retained October 14 build receipts; Conduit uses
`EXACT_HTTP_ASYNC=1` and published V5 Hermes. These are source-attributed
results for those prepared binaries, not a qualification of newer main.

Both fixtures' first normal agent launches failed with `No route to host`.
A subsequent bounded launch of each established the authenticated agent
connection on the Mac's normal Wi-Fi IPv4 address. No permission sheet was
observed in the captured screens. First-operation privacy/readiness timing
remains a hypothesis; the cause was not established. The successful drives
used the existing carrier without a host override, proxy, network change or
source fix.

Conduit loaded four articles and fifteen tags from its anonymous live API.
A real hit-tested article tap opened the expected route and loaded its body
and two comments, with no pending work at either checkpoint. The original
15-operation drive then stopped at operation 8: `tap nav-home` was refused
because a `UIView` covered the control's center at `(1080.93, 28.0)`. Return
through that control and the later login route are unqualified. The refusal
was preserved; hit testing was not bypassed. Current main was not tested.

The storage fixture passed its normal UI drive: two hit-tested button taps
advanced both displayed and resource values from `0` to `1` to `2` through
`fs.readFile` and `fs.atomicWriteFile` at `app:/data/count.json`. A separate
launch with the same named isolated store read back `2`; every checkpoint
had no pending work. This closes the prepared storage fixture's physical UI
write/read and same-store restart gap. Compact assertions, original failures,
raw trees/states, screenshots, device identity and binary hashes are retained
in the `20261015-device-and-maintenance` run receipts. The storage witness covers atomic-file persistence, not SQLite.

These results do not pass Tuft's measured LTE rollout gate or enable async
by default. The broader stages and this RFC's Draft status remain unchanged;
physical Android qualification remains pending an available device.
