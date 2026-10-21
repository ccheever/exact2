# Design review: LLP 1120 r7, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `1b5cef594fb4f27a1efb4dd1185e5666df2c2d7b0af59a3383713f119a214994`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Taken in r7 without another round (§0, round 7): M1 written in; both NITs fixed.

---

The RFC is one change short of a ruling. Round 6’s blocker and the other findings are real mechanisms now. r7’s park path opens the adapter deadline again, on the wait that returning the thread creates.

## Round 6

| Finding | In r7 |
|---|---|
| Grok B1 / Astra M3 — release does not wake the starter | Resolved. Callbacks, the timer, the deallocator, and reader exits only post to the finisher. The finisher applies the charge, the ordered slot, the per-origin count, the upload, and the reader permit under the state lock, then wakes the starter. A permit returned to the process counter wakes every starter waiting on it. `drain` releases under the same lock before it wakes. The tests make each of those the only event that can move a parked upload or a queued handoff (`llp/1120-network-concurrency-without-threads.rfc.md:191-198`, `:289`, `:443`). |
| Astra M1 — `forget` stops cancelling streams | Resolved. Forget cancels every stream, including a POST SSE with `exactBodyFrom`, while preparing or open. Retirement aborts every HTTP exchange and stream. Writes keep the non-cancellation rule (`:242-253`, `:395`, `:442`). |
| Grok M2 — a write waiting for an adapter thread is dropped | Resolved. That wait is a queued job: a safe read is removed, and a write, `Now`, storage, or handoff is marked and runs when a thread accepts (`:262`, `:395`, `:449`). |
| Astra M2 — a straggler’s exit gives away the reserved ordered thread | Resolved. The ordered role and its permit belong to the lane. A returning straggler takes the role back if nobody holds it, and otherwise exits. A replacement is created only when a job is ready, nobody holds the role, and the lane has fewer than three ordered adapter threads (`:264-267`, `:449`). |
| Grok M3 — the upload deadline runs through the adapter wait | Resolved for the first start. Sizing begins only after accept, and that thread is held through sizing, reading, and the exchange. Not resolved after a park. Finding 1. |
| Grok m5 — phase 2 asks for a second reader permit | Resolved. The sizing reader waits for the decision and, if the charge is taken, reads under the same permit (`:310-314`, `:444`). |
| Astra m4 / Grok m6 — Linux thread bound and the render platform row | Resolved. The base set is 6 for a platform-session transport (Apple, and the render host with `EXACT_RENDER_TRANSPORT=platform` on Apple) and 15 for the adapter. At most two extra ordered threads per executor are in the arithmetic and in §5 (`:257`, `:274-282`, `:290`, `:506`). |
| Astra m5 / Grok m4 — file-backed streams | Resolved. An interested stream refused after sizing keeps its charge until the reader exits and the outcome drains. Both streams admitted before either is sized are refused. The read and the resume are written (`:315`, `:344`, `:441`). |
| NIT — reader failure is not a terminal transition | Resolved (`:247`). |
| NIT — a treatment failure stops the other experiments | Resolved. It stops that comparison only (`:482-484`). |

## Findings

**1. MAJOR — After a park, the adapter deadline arms when the bytes are taken, which is before a thread accepts.** `llp/1120-network-concurrency-without-threads.rfc.md:238`, `:261-262`, `:313-314`.

r7 returns the adapter thread when an upload parks, which is the right release. The next start then needs a thread again. D4 says a job waiting for a thread is not started and has no deadline, and that the deadline arms at acceptance. D3 and D5 say the parked job starts again when its whole charge can be taken, and that this is when the fresh deadline arms.

Those are different moments. Bytes often become free while every ordered thread is a straggler, or while the counter is exhausted. The starter decides under the lock, and the natural decision is: the bytes fit, so take the charge and arm. Acceptance happens afterward, outside the lock, and may not succeed. A 100 ms POST that already parked then times out in the core, with nothing on the wire. That is the round-6 failure, on the wait this round added. The new test (`:450`) only covers the first wait, before the first accept.

The head is not passed over, so nothing behind it can take the bytes. The job can stay in the parked row, charge not taken and deadline disarmed, until the starter can accept a thread. In that same decision it takes the whole charge, accepts the thread, and arms the fresh generation. If no thread can be accepted, it stays parked. The test to add: bytes become free while the lane cannot accept a thread; the deadline does not fire; when a thread later accepts, one fresh deadline covers the read and the exchange.

**2. NIT — §0 sends every reader result through the finisher.** `llp/1120-network-concurrency-without-threads.rfc.md:97`, against `:184` and `:310`.

D3 and D5 are the procedure: a size or the bytes wake the starter, which the reader is waiting on; a failure or an exit is a finisher event. §0’s “readers only post to the finisher” is the release rule, overstated. Implement the design sections.

**3. NIT — The fifth-reader refusal quotes a sentence the code does not use.** `llp/1120-network-concurrency-without-threads.rfc.md:288`. `host/apple/src/executor_body.rs:127-129`.

Today’s refusal is `exactBodyFrom: 4 file reads are still running (a stalled disk?); nothing was sent`. The behavior to preserve is that refusal. The quoted “the app’s files are not answering” is not the current diagnostic.

## The five questions

**§3 against D1–D8.** It holds for D1, D2, and D5–D8: admission, the fence, re-asks, settlement through `ordered`, grants, per-hop headers, explicit independence, stream cancellation, and upload charging. It holds for the starter, the finisher, and the disposer, and for D4’s forget rule and thread counts. It does not yet hold for `exactTimeout` on Linux and the render host, because D3’s re-arm and D4’s unarmed thread wait disagree after a park. That is finding 1.

**Starter, limits, terminals, release, threads, uploads, caps.** The starter is the only place a lane starts work. The finisher is the only place a release becomes visible, and it wakes the starter after the lock is released. Per-origin start is at most six HTTP/1.1 exchanges per lane, lifted when the finisher records `h2` or `h3` and restored on any other protocol. The seventh waits in the core with no deadline. Any platform queue after `resume` counts against `exactTimeout`, and that change is declared. The terminal list has one winner under the request lock, including a reader failure, and a firing already in flight after a park sees a stale generation. Logical and physical release are the later of the two on every table row, with upload bytes on their own rule: the last hop that sends the buffer, or the terminal transition if none is outstanding, and never before the lent buffer’s last release. Apple reserves 6 threads, the adapter 15, all or nothing; sixteen executors reserve 96 or 240, plus at most two stragglers each. A stuck resolver keeps its own executor’s permit. Replacements come from the counter, and a returning straggler restores the ordered role without borrowing another executor’s base thread. The in-flight caps are resumed independent HTTP exchanges, 32 and 8, held to physical end. Handoffs, `Now`, undrained completions, and streams stay outside them.

**§5.** The gates can decide this change and do not bless the wrong cause. C7 against A7 removes “seven workers instead of six.” Overlapping transactions on one `h2`/`h3` connection per origin, against several connections in A7, is the session mechanism. C against C7 rejects a win that appears only at cap 32. C against A6 rejects a regression relative to what ships, on median and worst, so one bad worst sample fails closed. A treatment failure fails that comparison only. A control failure reruns the sequence once, and only when every treatment run completed. Preconnect is a separate experiment. Its cold check runs on C, and a positive `domainLookup` is evidence. The Linux bound now counts the extra ordered threads, so a faithful run is not failed for threads the design itself creates.

**What an implementer can settle in code review.** The role check and replacement creation share the state lock. The redirect state machine runs on the finisher. The starter resumes the next hop. The buffer event is the last `Arc` drop, not each hop’s `NSData` free. An ordered charge already drained before physical end ends at physical end. A forgotten stream releases its charge, count, and upload as a forgotten safe read does, at physical end. Thread permits stay with the thread until it exits. The reader’s size and bytes wake the starter directly.

## Verdict

**RIGHT WITH CHANGES.** Finding 1 is the change that has to land before implementation. Findings 2 and 3 do not. With finding 1 written in, the RFC is ready for a ruling and for x2-perf3.
