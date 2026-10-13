# Design review: LLP 1120 r5, 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `ecb0631869531f61b1294be56ef90c883a97a73e77b9a2cc40fd32a3b52b201c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r6 (§0, round 5).

---

The RFC is not ready for a ruling that authorizes implementation. The round-4 end states are largely written down, and the session-change gates can decide the Apple change. The upload path, forget, one release row, and two measurement rules still do not close.

## Round 4

| Finding | In r5 |
|---|---|
| Astra B1 / Grok M1 — uploads deadlock, or wait with the deadline armed | The end state is written: stat with no response reservation, then one atomic charge, or return to the head disarmed. The transitions that reach that state are not. Finding 1. |
| Both M — “URLSession never queues” | Resolved. Any platform queue after `resume` counts against `exactTimeout`, including a limit Apple lowered, and §3 lists it. |
| Astra M3 / Grok n12 — prompt publication, opaque drops | Resolved for a stuck transport, a blocked reader, a resolver, and destructors. The new terminal event treats every `forget` as an abort. Finding 2. |
| Both M — ordered slot held until the reader exits | Resolved for a stalled read: the slot ends at the terminal transition, the permit and upload charge do not, and the §4 stall test matches. The park-between-phases path has no such row. Finding 1. |
| Astra M5 / Grok m5 — release table vs `complete` | Resolved for interested success and failure: independent keeps the running charge less the upload until drain. A carried upload whose next hop never starts has no release event. Finding 3. |
| Both M — gates bless the wrong cause | Resolved for separate preconnect, a reboot before every preconnect run, and a treatment failure failing its gate. The control-failure rerun and the cold check on C+P are still open. Finding 4. |
| Grok m6 / Astra m7 — thread arithmetic and the Linux gate | Resolved. Apple 6 and Linux/render 15 match the rows, unspawned permits return at once, a reader is refused when the counter is exhausted, and the Linux gate counts SSE, both WebSocket threads, readers, and retiring threads. |
| Grok m7 — URLSession’s copy of the body | Resolved. `HTTPBody` is a no-copy buffer over the charged bytes, and D2 holds the charge until the later of physical end and the last release. The failure rows drop the second half. Finding 6. |
| Grok m8 — one delegate queue | Resolved. One serial queue per lane session, and callbacks return without waiting. |
| Grok m9 — metrics after `didComplete` | Resolved. The record outlives `didComplete` until metrics arrive or one second passes; anything other than `h2`/`h3` stays at 6. |
| Grok m10 / Astra n8 — the FIFO question and `start_socket` | Resolved. Question 1 states the origin exception. `start_socket` takes the ceiling. |
| Grok n11 — prose bounds | Resolved. The fit check is `reservation` plus the file against `BYTES[lane]`. |

## Findings

**1. BLOCKER — A file upload never reaches `resume` on the events the starter actually waits for.** `llp/1120-network-concurrency-without-threads.rfc.md:158-161`, `:200-206`, `:264-268`, `:280-282`. `host/apple/src/executor_core.rs:799-813`, `:1231-1238`. `host/apple/src/executor_body.rs:107-158`.

The wake list is admission, `drain`, `forget`, `resume_ordered`, a finished exchange, and a handoff’s return. A reader finishing is none of those. Reader *failure* is a finisher event (`:167`). A successful stat or a successful read is not.

D5 then says the reader exits and the starter decides. The starter is also “the only place” that resumes a task (`:152`, `:160`). For a 1 KiB file that fits, the reader stats, the charge is taken, the reader reads the bytes and exits, and the starter is parked. The phase-2 deadline is already armed (`:268`). Nothing resumes the task. The timer fires and the body is never sent. That is the round-4 armed-deadline failure on every `exactBodyFrom`, including ones that never wait for bytes.

The other edge has the same shape. After stat, a charge that fits the lane budget but not the free bytes is supposed to return to the head, disarmed, holding nothing. Reaching that decision needs the same missing wake. The release table, which says every path has one release event, has no park row. The ordered slot’s only written release is the terminal transition (`:175`), and the parked job was just disarmed, so that transition never comes. An ordered upload behind undrained bytes — ordered running charge is about 128 MiB (`reservation` uses the 64 MiB ceiling), so three undrained responses leave too little for a 64 MiB file plus that charge — sits at the head holding the only slot.

“Holding nothing” also drops the waiting charge. A queued job is charged its request buffers (`:260`, and `executor_core.rs:49-50`). The atomic take is `reservation` plus the file on top of whatever the job still holds, so the row has to say which.

The §4 stall test (`:387`) requires the starter to be free while a reader is blocked, so the starter cannot be joined to the read. The reader-result wake has to be in the list at `:158`, and the park path needs a table row: ordered slot released, deadline disarmed, waiting charge left at the request buffers, in-flight slot not taken.

**2. MAJOR — `forget` of an in-flight ordered write becomes an abort and frees the lane.** `llp/1120-network-concurrency-without-threads.rfc.md:165-169`, `:208-211`, `:340`. `host/apple/src/executor_core.rs:512-517`, `:528-539`, `:557-561`, `:842-845`.

The finisher event for `forget` publishes `Aborted` and releases the ordered slot at that transition. Today `forget` aborts a safe read and marks every other in-flight job forgotten. A POST stays on the worker until the exchange ends; `complete` then drops the outcome because it is forgotten. The worker is busy the whole time, so the next ordered job waits. LLP 1041 §8.4 says the same: a write still runs, and it keeps its slot until it ends.

On this text the starter has already returned from `resume`, so it is free. `forget` posts the terminal event, the slot opens, and the next ordered POST can be resumed while the first POST is still on the wire. §3’s “admitted queued writes still execute” and “one blocking job” are then false of D3. A forgotten *read* is the case the event is for: cancel it, publish through `complete`, and let `complete` drop it when the ticket is forgotten. A write’s terminal transition stays the write’s own end.

**3. MAJOR — Upload bytes carried onto a hop that never starts are never released.** `llp/1120-network-concurrency-without-threads.rfc.md:144`, `:284-292`. `vendor/ibex/crates/ibex2/src/stdlib/fetch.rs:410-418`, `:469-475`. `host/apple/src/executor_core.rs:1365-1372`.

The redirect row releases the old hop’s in-flight slot at its physical end and carries the upload onto the next hop. The next hop’s grant is checked only after that, and a redirect off the granted origin is a real refusal (`DeniedRedirect`). Hop 2 is never resumed. The reader already exited when the body was first read. Physical end, the table’s release for a “refused redirect hop,” is defined as `didComplete`, the adapter thread’s return, or the reader’s exit (`:292`). All three are already in the past. The same hole is a timeout while the continuation waits to take the cap again: D3 keeps that wait on the deadline (`:206`), and there is no task whose `didComplete` can free the file.

Ten mebibytes then sit in the lane budget after the outcome has settled. The failure row has to release a carried upload at the terminal transition when no hop is outstanding.

**4. MAJOR — Two gate rules still give one sequence two opposite results.** `llp/1120-network-concurrency-without-threads.rfc.md:69`, `:300`, `:400-402`, `:414`.

The C7-against-A7 gate, the one-connection mechanism check, and “C is not worse than C7” can decide the session change. Preconnect is no longer mixed into that sequence.

Gate 4 says a failed treatment run fails that arm, and a failed control run makes the comparison inconclusive and the whole sequence is rerun. One sequence in which A7 drops a feed read and C7 does too is both a failed gate and a rerun. The rerun drops C7’s failure. The same paragraph says there are no replacement runs. A control failure may rerun the sequence only when every treatment run completed; a treatment failure stops the campaign.

The preconnect check has the other problem. §5 requires every run’s first independent task to report `domainLookup` > 0. D6’s acceptance requires that same duration to fall, and the flag’s only job is a TLS handshake to that origin, which warms DNS before the first task. A preconnect that clears the lookup fails its own cold check and the experiment is rerun from the start, so a working flag never produces a cohort. §0’s narrower check — `domainLookup` > 0 on C, after the reboot — is the one that matches the acceptance rule. “No reused connection” can stay on both arms, because that connection is outside URLSession’s pool.

**5. MINOR — The fast path also arms a fresh deadline, and §3 declares that only for a wait.** `llp/1120-network-concurrency-without-threads.rfc.md:201-205`, `:268`, `:350`. `host/apple/src/executor_core.rs:1231-1238`.

D3 resets the clock when a request waited between stat and read. D5’s other branch, the one where the charge fits immediately, starts the reader with “a fresh deadline.” Today one `Deadline::arm` covers the stat, the read, and the exchange. Two reader phases each arming their own clock drop the stat out of `exactTimeout` on every file upload. The declared change should match whichever rule is kept. Carrying the phase-1 deadline into phase 2 when the charge fits is the rule D3 and §3 already state.

**6. MINOR — Failure rows release the upload at `didComplete` while the no-copy buffer can still be alive.** `llp/1120-network-concurrency-without-threads.rfc.md:144`, `:284-290`.

D2 holds the charge until the later of physical end and the deallocator’s last release. The success row says both. The failure, forgotten, and retirement rows say physical end only. A cancel can deliver `didComplete` before `NSData` calls the deallocator, and the lane will lend those same bytes to another exchange.

**7. MINOR — A 303, 301, or 302 keeps the upload charge after it has dropped the body.** `llp/1120-network-concurrency-without-threads.rfc.md:117`, `:269`, `:287`. `vendor/ibex/crates/ibex2/src/stdlib/fetch.rs:469-475`.

D1 drops the body for 303 and for 301/302 of a non-GET/HEAD. The redirect row carries the upload onto that next hop anyway. The charge should end with the last hop that still sends the buffer.

**8. MINOR — One §4 test still waits for physical end.** `llp/1120-network-concurrency-without-threads.rfc.md:375`, against `:175` and `:387`.

“A handoff queued behind a blocking job waits until that job’s physical end” is the lifetime round 4 removed. The stall test at `:387` is the one that matches D3. Encoding `:375` with a stalled reader brings the ordered-lane freeze back.

**9. MINOR — The origin limit is updated under the wrong lock, and sizing still takes a full reservation in the starter’s general rule.** `llp/1120-network-concurrency-without-threads.rfc.md:159`, `:196`, `:321-323`, `:264`.

The per-origin count is lane state. The starter reads it under the state lock. D8 records the protocol under the exchange’s own lock and allows that state lock only for `message()`. Those are different mutexes, so the lift and the restore are a data race on the written locking rule. Separately, the starter “moves each into `running` with its full reservation” before a file’s size is known. That is the provisional response hold from round 4. D5’s sizing sentence holds only the request buffers. One of them has to be the rule.

**10. NIT — “The origin’s protocol is always applied” is the one-second cutoff.** `llp/1120-network-concurrency-without-threads.rfc.md:145`. Metrics after that second leave the origin at 6. That is a safe default; the sentence says always.

## What holds

The direction is unchanged and still right: one session per lane, task delegates, byte and count caps, upload bytes in the charge, Linux on the same core.

D5’s predicates match `admit` (`executor_core.rs:379-386`), including reads past sixteen unfinished effects and the separate re-ask window. Handoffs add no response bytes. The in-flight cap is resumed independent HTTP exchanges, 32 and 8, held to physical end; handoffs, `Now`, undrained completions, and streams stay outside it. Per-origin start is at most six HTTP/1.1 exchanges per lane until a finished exchange reports `h2` or `h3`, a head at that limit is passed over, another origin starts, and the ordered lane is unaffected. The seventh waits in the core with no deadline. Platform queuing after `resume` is inside `exactTimeout` and declared. Redirect policy matches `fetch.rs`, including per-hop grants, `Flow::Skip` for a 3xx that will be followed, and the declared per-hop header check.

Logical and physical release match `complete` for the paths the table actually names. An interested independent outcome, success or failure, keeps the running charge less the upload until drain. Ordered keeps what it retains. The ordered slot ends at the terminal transition for a stalled reader, a cancel, and a normal completion. The finisher publishes a timeout without waiting for `didComplete`, the reader, or the resolver (`rustls_http.rs:188-213`). The disposer, not the finisher, drops opaque work.

Thread reservation is consistent. Apple’s base is 6 (two starters, two finishers, timer, disposer). Linux and render add one ordered adapter thread and eight independent ones, 15. Sixteen executors reserve 96 or 240. Creation is all or nothing; threads never spawned return their permits immediately; readers and Linux streams refuse when the counter cannot supply them; a stuck resolver keeps its own executor’s adapter thread. `dataWithBytesNoCopy` over the charged buffer is the right fix for the old `dataWithBytes:length:` copy (`darwin_http.mm:347`). The delegate queue is named, and callbacks do not wait on the 64 KiB handoff.

§3 is true of those predicates, of settlement through `ordered`, the fence, re-asks, forget of a queued read, retirement’s “do not join on the UI thread,” grants, explicit independence, and the changes it lists, for D1, D2, D4, D6, and D7. It is not true of D3 and D5 for an `exactBodyFrom` park, for `forget` of an in-flight write, or for the fast-path deadline if D5’s second arm wins.

An implementer would still have to invent the reader-result wake and the park row (finding 1), the upload release when the continuation never resumes (finding 3), which failure wins when a treatment run and a control run both fail (finding 4), and the render host’s `EXACT_RENDER_TRANSPORT=platform` base set (`host/render/src/executor.rs:32-40`), which is Apple’s session path inside a host whose row is the 15-thread adapter.

## Verdict

**RIGHT WITH CHANGES.** Rounds 1–4’s direction holds, and both round-4 measurement and URLSession rulings are in the mechanism. Finding 1 has to land before a file upload is implementable, and findings 2–4 before §3 can be treated as preserved and §5 as deciding the flag.
