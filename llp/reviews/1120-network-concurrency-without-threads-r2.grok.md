# Design review: LLP 1120 r2, 2026-10-09 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `587886bf89905233caa9b2e23806f4d64c58092725f16365fa016939237d97fb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r3 (§0, round 2).

---

## Round 1

| §0 finding | Disposition |
|---|---|
| One blocking ordered job; handoffs free the slot | Taken in D3. Matches `handed_off` charging nothing extra (`executor_core.rs:803-807`) and `Later` returning without `complete` (`executor_core.rs:1075-1081`). A burst already in the queue does not move unless the handoff's return re-enters the decision. |
| Cap on the independent lane only, as a wait | Taken in D5. An ordered release no longer shares that cap. |
| Each task delegate refuses redirects | Taken in D2. Same `completionHandler(nil)` as `darwin_http.mm:226-230`. |
| Stop at `head` | Taken in D1, including the HEAD / 204 / 205 / 304 exception at `darwin_http.mm:149-159`. |
| Opaque work on both lanes | Taken as a split (ordered work thread, independent work thread, handoff returns at once). The thread that makes the call is not one of the threads D3 lists. |
| `exactBodyFrom` stays on its reader threads | Taken. The path-not-bytes gap is recorded in §7. The new cap multiplies it. |
| Streams keep a session; per-event SSE ceiling; WebSocket rules | Taken in D7. |
| Nothing transport-related under the state lock; one terminal outcome; lazy sessions; delegate holds state | The lock rule, the single terminal outcome, and laziness are taken. D3 still starts handoffs on the event thread, which includes the delegate queue. |
| Independent completions keep the running charge; windows overlap; ceiling about 32 MiB | Taken. `complete` keeps `run.charge` on lane 1 (`executor_core.rs:853-857`). The 128, the sixteen, and the markers' own 128 match the 2026-10-09 amendment. `limit > BYTES[1]` refuses a ceiling much above 32 MiB (`executor_core.rs:1007-1013`). |
| Linux: one rustls transport per lane, inside the process cap; no hand-written reactor | The adapter and the rejection of a private reactor are taken. The cap has no number and no failure mode. |
| Redirect rules and one deadline for the chain | Taken in D1 and D3. |
| FIFO | Taken. |
| Preconnect default off | Taken, with a real accept/reject rule. |
| A vs C under a throttled link; matched concurrency; connection metrics | The arms exist. The accept rule does not use the matched arms. |

## Findings

**1. MAJOR — D3 and D8 name different threads for the same call.** `llp/1120-network-concurrency-without-threads.rfc.md:86-101`, `:167-170`. `js/src/native.rs:58-62`. `host/apple/src/executor_core.rs:1042-1081`.

D3 decides under the lock, drops it, then on that same stack calls `task.resume()`, the handoff, or a work-thread post. Completions arrive on the session's delegate queue, and `admit` / `drain` arrive on the host thread. A `Later` call from either of those is an arbitrary closure on a thread D8 forbids, and it is not "a thread of its own," which is the `native.later` contract. D2 also builds the outcome on completion, which D8 says a callback must not parse.

The ordered rule has the same hole. "The decision never loops over lane 0" (`:92`) forbids the worker's continue-loop. "A handoff returning" is listed as an event (`:88`), but a synchronous return happens inside the starter. If that return does not re-enter before the starter parks, a burst already queued behind a handoff sits until some later admit or completion. Today's worker starts the successor itself. §4's new test admits the next job during the handoff, so it does not catch the burst.

§3's "FIFO execution" and "handoffs free the slot" (`:177-179`) are true of the slot and of settlement through `ordered`. They are not true of a decision that starts one ordered job and waits.

Fix: one starter that is neither the delegate queue nor the UI thread. Delegate callbacks only append bytes and enqueue. HTTP tasks are resumed there or on the starter, not under the lock. Handoffs, `Now`, storage, discard, and outcome construction run on the lane work thread. A handoff that returns re-enters the decision before the starter parks, so a run of handoffs still reaches the next blocking job. "Never loops" means only: do not start a second blocking ordered job, and do not skip the head.

**2. MAJOR — a deadline starts when the task is resumed, which is before URLSession will run it.** `llp/1120-network-concurrency-without-threads.rfc.md:109`, `:146`. `host/apple/src/executor_core.rs:1231-1238`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:348-349`.

Today the deadline is armed when a worker actually enters `execute`, and a job waiting for a worker has none. D3 keeps that sentence and then defines the start as `task.resume()`. `HTTPMaximumConnectionsPerHost` defaults to 6 per session. With small ceilings the byte budget allows more than 6, and the cap allows 32, so the rest sit inside URLSession with the deadline already running. `timeoutInterval` is an idle timer (`darwin_http.mm:348-349`). A request LLP 1041 would still have queued can come back `Timeout`.

§3's "exactTimeout as D3" (`:187`) therefore does not preserve LLP 1041. Arm the deadline when the task leaves the session's connection queue (first send, or `didReceiveResponse`), and keep `deadline + 1 s` as the idle timeout from there. The outcome order at `executor_core.rs:1273-1289` can stay, including "already aborted, so the flag is never set."

**3. MAJOR — the process cap is not a number, and a stuck resolver can take the ordered thread's permit.** `llp/1120-network-concurrency-without-threads.rfc.md:125-128`. `host/apple/src/executor_core.rs:17-22`. `vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:188-191`.

Today `MAX_WORKERS` is `16 * 7 = 112`, counted only for the long-lived workers, reserved entirely at `Executor::start`, and a failed reserve disables the executor. Deadline threads, stream threads, and the Linux WebSocket writer are outside it. D4 folds work, timer, adapter, and stream threads into one cap "sized for sixteen executors" and does not say what 16 full executors add up to, whether adapter threads are reserved at start or taken per exchange, or what happens when the counter is full (refuse the executor, refuse the request, or block).

A Linux or render executor at the cap is 2 work threads + 1 timer + 1 ordered adapter + 32 independent adapters + up to 16 stream threads, plus a writer thread per WebSocket that D7 keeps (`websocket.rs` `writer_loop`) and does not put in the cap. `getaddrinfo` cannot be interrupted (`rustls_http.rs:188-191`); D4 correctly holds the permit until it returns. If those permits are one global counter, independent adapter threads of this or another executor can exhaust it and the ordered pool of one never spawns. That is the round-1 cross-lane stall again, on the render host as well as Linux, because the render host uses this adapter by default (`host/render/src/executor.rs:23-31`).

Fix: state the integer and the formula. Reserve, at start, the threads an executor needs in order to make progress: both work threads, the timer, the one ordered adapter, and the stream allowance. Take independent adapter threads from what remains. If the reserve fails, disable the executor as today. If an independent adapter cannot be spawned, that job waits. A WebSocket writer counts, or it has its own cap. Readers stay at their own 4.

**4. MAJOR — "32 exchanges" does not say what is in flight, and it is the case §7 leaves unbounded.** `llp/1120-network-concurrency-without-threads.rfc.md:96-100`, `:146`, `:260-261`. `runner/src/request.rs:120`. `host/apple/src/executor_body.rs:95`.

D5 caps "32 exchanges." D3 applies that cap to whether an independent job starts, and independent jobs include `Later` native calls and `Now` driver faults. A native call stays in `running` until the owner replies, and today that is limited by the 128, not by the six workers, because a handoff does not keep the worker. Counting those toward 32 would newly stall `native.later`. Counting a finished exchange until `drain` would stall the lane on held replies while the sockets are already gone. The cap only does what D5 says (sockets, descriptors, adapter threads) if it counts a resumed HTTP exchange and releases it at the terminal callback.

The same sentence is the upload hole. An `exactBodyFrom` body is admitted as the path's size and then becomes up to 64 MiB (`MAX_BODY_FROM_BYTES`). The four reader threads limit how fast those reads start, not how many bodies sit in exchanges afterward. Six workers held about seven such bodies. Thirty-two independent exchanges plus the one ordered exchange hold about thirty-three, on the order of 2 GiB, outside the 64 MiB lane. §7 says this is true today and defers the fix. It is true today at a smaller bound. The cap was added for tiny ceilings, which is when the most uploads fit. Either the running charge includes the file body for the life of the exchange, or §7 states the new worst case and the ruling accepts it.

**5. MAJOR — §5 can accept the session change for a seventh in-flight read.** `llp/1120-network-concurrency-without-threads.rfc.md:220-249`. `host/apple/src/executor_core.rs:17`, `:1007-1013`.

A 4 MiB independent ceiling charges about 8 MiB plus the 128 KiB header allowance. Seven fit in 64 MiB and eight do not. Arm A has six workers. Arm C, under the same budget, starts seven. The 2026-10-09 amendment's gain was from two workers to six. One more wave on nineteen reads can beat arm A's interquartile range by itself. A7 and C7 are the control that separates that from a shared HTTP/2 session, and the accept rule never looks at them. Linux "decides the adapter" with no median, worst-case, or Storm threshold.

Gate C on C7 beating A7 by more than A7's interquartile range, with C7's worst not worse, and require the recorded metrics to show reuse (`isReusedConnection`, one protocol, connections opened). Define A7 as a rebuild whose independent worker count is 7. Give the Linux arm the same shape of rule against today's six workers, for cold load and for the Storm drive. Keep fling-discover recorded; it does not have to gate this decision.

**6. MINOR — a 3xx `Stop` is described as a terminal outcome.** `llp/1120-network-concurrency-without-threads.rfc.md:46-53`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:149-159`. `vendor/ibex/crates/ibex2/src/stdlib/fetch.rs:637-651`.

`head` both refuses a declared length and drops a 3xx body. A 302 is `hasBody` in the current session delegate, so a `Content-Length` over the ceiling is refused today and never followed. The terminal-outcome sentence then says the outcome recorded at the stop wins over the cancellation error, with `over_limit` as the example. A redirect stop is not terminal: `FetchTask` still has to follow, and the cancellation `end` must not become the fetch's failure. `streamed_redirect_keeps_admission_and_does_not_collect_redirect_body` needs `DeniedRedirect`, not `over_limit`, for a small redirect body. Say the declared-length check runs first and refuses, and only a 3xx inside the ceiling is a non-terminal `Stop`.

**7. MINOR — the running-charge sentence drops the 32 KiB floor.** `llp/1120-network-concurrency-without-threads.rfc.md:140-141`. `host/apple/src/executor_core.rs:1007`.

The code charges `request bytes + max(2 × ceiling, 32 KiB) + 2 × 64 KiB`. D5 says "twice its response ceiling" and also "exactly today's." For ceilings under 16 KiB those differ. The 32-exchange cap binds before the byte budget in that range, so this will not show up on the clone. An implementer who codes the sentence will still change who waits.

**8. MINOR — the asynchronous WebSocket is a list of invariants, not an interface.** `llp/1120-network-concurrency-without-threads.rfc.md:164-166`.

Redirect refusal, `maximumMessageSize = ceiling + 1`, text-only frames, per-socket sessions, and per-message ceilings are the right invariants. Message delivery, the send queue, and close still have to be invented, and on Linux the writer thread's place in finding 3's cap has to be chosen with them.

**9. NIT — `didBecomeInvalidWithError` is not a task-delegate callback.** `llp/1120-network-concurrency-without-threads.rfc.md:79-82`.

Task delegates receive `didCompleteWithError`. Session invalidation is delivered to the session delegate. Per-task `didComplete` is enough to release each exchange after `invalidateAndCancel`, provided some session-level object actually observes invalidation if the session's own delegate retains shared state.

## What holds

The ordered rule, read as one blocking job and handoffs that do not hold it, is the worker that exists. Settlement through `ordered`, one completion per pump, `counts[0] == 0`, the fence's 128 and 64 MiB, marker placement after fenced work, and forget (queued safe reads dropped, admitted writes and module turns still run) are all properties of `State`, which D3 leaves in place. Markers never start an exchange. Grants and the redirect steps in D1 match `fetch.rs:403-486`. Nothing in D1–D8 infers `exactIndependentHttp`. The reachable independent ceiling really is about 32 MiB. Preconnect is honestly outside URLSession's pool, default off, and has a two-part acceptance rule. The Apple cold-load question is the right primary outcome once finding 5 gates it.

## Verdict

**RIGHT WITH CHANGES.** No finding says the direction is wrong. Findings 1–5 have to be written into the mechanism before an implementer can keep LLP 1041's timeouts, ordered progress, and lane isolation without inventing the scheduler.
