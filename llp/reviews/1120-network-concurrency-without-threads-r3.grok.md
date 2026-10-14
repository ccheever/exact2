# Design review: LLP 1120 r3, 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `ab76db0e8c5868c62baa766f1afd7b7796c2c0275525c65600c29d8853b479ae`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r4 (§0, round 3).

---

## Round 2

| Finding | In r3 |
|---|---|
| Grok M1, Astra M4 — starter, lock, handoff re-entry | A lane starter is real, and a leading run of handoffs re-enters before it parks. The ordered loop still starts later handoffs during a blocking job. `head` on the delegate is still in D2 while D8 forbids it. |
| Astra M1, Grok m7 — handoff charge and the 32 KiB floor | Resolved. Running `Later` charges no response bytes; the floor is in the formula; §7 records the handed-off outcome gap. |
| Astra M2 — the sixteen | Resolved. D5 matches `admit` (`executor_core.rs:379-386`), including reads past sixteen unfinished ones. |
| Grok M2 — deadline vs URLSession's queue | The HTTP/1.1 start limit of 6 is the right shape. It is not keyed per session, and the first `h2`/`h3` report lifts it for good. |
| Grok M3, Astra M3 — process threads | Resolved as a reservation: 3 or 12 at start, ordered adapter included, WebSocket writer in the stream pool, readers stay at 4. The 384 arithmetic still describes three pools. |
| Grok M4, Astra M5, Astra M6 — what 32 counts, logical vs physical, uploads | The cap counts resumed independent exchanges until physical `end`. Upload bytes are an added charge. An impossible charge waits forever, and D3 releases the byte charge at physical `end` while D5 keeps it until drain. |
| Grok M5, Astra M7 — gates | The 7-vs-6 confound is closed: A7 is seven independent workers, C7's cap is 7, and reuse metrics are required. Two sentences are not yet a procedure. |
| Grok m6 — 3xx `Stop` | Resolved. Declared length is first and terminal; a 3xx inside the ceiling is `Flow::Skip` with the hop's `end` ignored. |
| Astra m8 — SSE and pull consumers | Resolved. Only a 2xx `text/event-stream` is parsed; `Transport::open` stays for `read_response`. |
| Astra m9, m10 — per-hop headers, §3 overclaims | Resolved as declared changes, with the retirement and independence qualifications. |
| Grok m8, n9 — socket interface, session invalidation | Resolved for the executor's receive-only socket (`executor_stream.rs:9-12`). Invalidation is the session delegate's. |

## Findings

**1. BLOCKER — the ordered starter runs later handoffs while a blocking job is in flight.** `llp/1120-network-concurrency-without-threads.rfc.md:119`, `:122-125`, `:235`. `host/apple/src/executor_core.rs:1074-1110`. `data/src/placed.rs:683-703`.

The ordered worker pops one job. A `Later` handoff is invoked and the loop continues, so a run of handoffs can reach the next blocking job with no further event. `execute` then does not return until that fetch, `Now` closure, or storage operation does, so a handoff queued behind it is not invoked. Ordered module work is one of those handoffs: `Work::Later` only enqueues the job to the owner (`placed.rs:683-703`). That enqueue is the start of the ordered turn.

D3 holds the start reservation only until the invocation returns, and `task.resume()` returns immediately. The re-decide rule then allows the head, if it is a handoff, to run: it is not a second blocking job and it is not ahead of the head. §3's "FIFO execution" is that worker loop. §4's new test covers handoffs followed by a blocking job (`:267`) and does not cover a handoff queued behind one.

The ordered starter may invoke a handoff only while no blocking ordered job is in flight. The blocking slot lasts until that job's physical `end` (the worker today cannot loop any earlier, including when a resolver is stuck). The outcome is still published at the terminal transition. "Handoffs don't hold the slot" covers a handoff that has already returned, so the following blocking job can start.

**2. BLOCKER — an independent upload the lane cannot hold waits forever at the head.** `llp/1120-network-concurrency-without-threads.rfc.md:194-200`, `:246`. `host/apple/src/executor_core.rs:37`, `:808-809`, `:1007-1013`. `runner/src/request.rs:120`.

`BYTES[1]` is 64 MiB. `MAX_BODY_FROM_BYTES` is 64 MiB. The running charge is the response reservation plus the file size. The reservation is at least `max(2 × ceiling, 32 KiB) + 2 × 64 KiB`, about 160 KiB, and `reservation` already refuses a ceiling the lane can never hold. A legal 64 MiB file plus that floor is over 64 MiB for every independent ceiling. A 4 MiB ceiling leaves room for about 56 MiB of file, so a large part of the legal range never fits either. The ordered lane's 512 MiB does fit the maximum file.

D5 says that job waits for bytes. `next_job` will not pass a head that does not fit, so the independent lane stops. Waiting is the right rule only when some later completion can make the charge fit. When `file size + limit > BYTES[lane]`, selection fails the request the way today's too-large read does: an execution outcome, after the granted `FsOp::Stat` in `executor_body.rs:204-215`, so an ordered failure is not an admission refusal and does not raise the fence. The same clause has to land if the charge is added to today's workers first.

**3. MAJOR — physical release drops the independent response budget.** `llp/1120-network-concurrency-without-threads.rfc.md:148-151`, `:201`. `host/apple/src/executor_core.rs:853-857`.

`complete` keeps an independent run's charge until `drain`. That retained body is the 64 MiB budget. D3 releases "in-flight slot and byte charge" at physical `end`. On success, physical `end` is before drain, so a literal reading admits another lane-full of responses while the previous bodies are still queued. On cancel, timeout, and forget, the body is not retained and both the slot and the response charge have to stay until physical `end`, which is the churn case D3 describes. Upload bytes are the third piece: they are in the running charge and are released at physical `end` on every path, including success. One rule, three releases.

**4. MAJOR — the per-origin limit can stall the other lane and the rest of the queue.** `llp/1120-network-concurrency-without-threads.rfc.md:100`, `:131-136`, `:274`.

`HTTPMaximumConnectionsPerHost` is per session. The ordered and independent lanes have different sessions, and today the ordered worker's session starts while six independent workers are busy on that host. D3 says "exchanges to one origin" and "the session's limit" in the same rule. One origin-keyed counter of 6 makes the ordered fetch wait, with no deadline, behind six independent HTTP/1.1 exchanges. The limit is per lane session.

Independent FIFO plus "the seventh waits" does not say whether that seventh blocks a later job to another origin. Stopping at the head means one HTTP/1.1 origin with six exchanges in flight holds the other 26 independent slots. The cap of 32 exists so those slots can run. The rule has to say which: the head waits and later origins start, or strict FIFO is the accepted stall. §4's seventh-request test should include the other lane and another origin.

**5. MAJOR — the first `h2` or `h3` report turns the deadline bug back on.** `llp/1120-network-concurrency-without-threads.rfc.md:133-135`, `:137-138`, `:274`. `vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:348-349`.

HTTP/2 and HTTP/3 ignore `HTTPMaximumConnectionsPerHost`. Lifting the start limit after a report is what lets 19 reads share a connection. The lift is permanent for that origin, and the deadline is armed at `resume`. A later HTTP/1.1 exchange on that origin — fallback after the h2 connection fails, or the other session if the latch is not per session — is resumed up to the lane cap. URLSession queues the excess at 6 connections, `timeoutInterval` is an idle timer, and those queued tasks come back `Timeout`. Restoring the limit of 6 when a task's `networkProtocolName` is `http/1.1` keeps later resumes honest. Tasks already resumed in that window are still inside the session with the deadline running; the RFC should say they keep the timeout they have, and no further resume to that origin happens until the in-flight HTTP/1.1 count is under 6.

**6. MAJOR — `head` is still on the delegate, and stream start has no thread.** `llp/1120-network-concurrency-without-threads.rfc.md:99-105`, `:226-228`, `:420` in `host/apple/src/executor_core.rs:436-448`.

D2 maps `didReceiveResponse` to `head`, then the disposition. D8 says a delegate callback only appends bytes and enqueues. `head` decides refusal, header limits, and `Flow::Skip`, and the disposition has to be delivered through the completion handler after the starter decides, or the body arrives before the stop. Session creation ("on first use", `:99`) and the origin's protocol update are the same class of work: on the starter, off the state lock.

Streams are not lane jobs. `admit` currently drops the lock and spawns the stream thread on the caller, which is the host pump. D7 makes an SSE stream an asynchronous task and does not say the starter is the thread that calls `start`. Left unchanged, the first stream `resume` runs on that caller.

**7. MINOR — one independent starter is not one of six workers.** `llp/1120-network-concurrency-without-threads.rfc.md:228`. `host/apple/src/executor_core.rs:271-280`.

D8 says a long `Now` delays its lane exactly as a busy worker does. A busy independent worker delays one of six. The starter is the only thread that resumes independent exchanges and builds their outcomes, so one slow `native.later` handler, which the contract says returns promptly, delays every other independent completion. Worth one sentence so the equivalence is not implemented as a claim.

**8. MINOR — two gate sentences are not yet decidable.** `llp/1120-network-concurrency-without-threads.rfc.md:301`, `:318-323`.

The Apple rule that closes round 2 is in place: C7 against an A7 built with seven independent workers, median by more than A7's IQR, worst not worse, and the feed on one reused connection per origin. "C is not worse than C7 on the same rule" uses a rule whose direction is "beats by more than the IQR". The operational form is: C's median is not worse than C7's by more than C7's IQR, and C's worst is not worse than C7's worst.

The Linux cold gate has no worst-case clause. "RSS growth over A's explained" accepts any growth. Peak threads "at or under the D4 reservation" does not say whether the four `exactBodyFrom` readers count.

**9. MINOR — the thread totals are three pools described as one cap.** `llp/1120-network-concurrency-without-threads.rfc.md:162-177`.

Executor reservations, the stream pool, and the four readers are separate counters, and 16 executors plus those pools sum to 292. `MAX_THREADS` is 384, and a retiring executor keeps its reservation until its threads exit. If streams and readers are outside the 384, sixteen live executors and sixteen retiring ones are 384 reserved threads plus 100 more. If they are inside it, the slack for retirees is 92, not the leftover of 192. One counter, or three, with the steady-state sum as a check.

A stream refused because the process pool of 96 is empty is a new process-wide refusal. Today's `STREAMS` is 16 per executor (`executor_core.rs:36`). §6 already asks for a ruling on 96; the "as today's full STREAMS" clause overclaims the equivalence.

**10. NIT — the timer both records the outcome and never runs work.** `llp/1120-network-concurrency-without-threads.rfc.md:139`.

The deadline thread today only sets the flag and aborts (`executor_core.rs:1308-1316`). The starter classifies the terminal transition. "Aborts and records" reads as a second publisher.

## What holds

The admission predicates, the fence, marker placement, `counts[0] == 0`, one completion per pump, and forget match `State` and §3. The ordered slot spanning every redirect and the file read is the right reading of one blocking job. The terminal-transition list matches the order in `execute` (`executor_core.rs:1273-1289`) for success, an earlier caller abort, timeout, over-limit, and the transport error. Handoff bytes, the sixteen, the 32 KiB floor, per-hop headers as `Network`, the SSE branch, and the receive-only socket are written as mechanisms. The Apple session split is still founded: the connection limit is per session. The Linux reservation gives the ordered adapter a permit another executor cannot take. A7 versus C7, with the reuse metrics required, decides the session change rather than a seventh in-flight read. Preconnect stays default off, out of the interleaved arms, with its own two-part acceptance rule.

§3 is true of the queue, the fence, forget, retirement, grants, and the declared header change. It is not true of ordered FIFO execution, of `exactBodyFrom` on the independent lane, or of `exactTimeout` after an origin has reported `h2` or `h3`, until findings 1, 2, and 5 are in the mechanism.

## Verdict

**RIGHT WITH CHANGES.** The design is the right one and round 2 is largely closed in the text. Findings 1–5 have to be written into the mechanism before a ruling can treat §3 as preserved.
