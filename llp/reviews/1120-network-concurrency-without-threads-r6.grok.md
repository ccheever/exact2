# Design review: LLP 1120 r6, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the draft's worktree, read-only by instruction.
- **Method:** one brief (sha256 `35e5cebb3c38e9986f32cb19f1c80965865719e280b14b3ce143412314552970`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES.
- **Disposition:** Every finding taken in r7 (§0, round 6).

---

The RFC is not ready for a ruling that authorizes implementation. Most round-5 fixes are real mechanisms. Holding a charge until physical end removes the only wake that lets a parked upload continue, and the new adapter wait both drops an admitted write and runs a deadline that same section says is off.

## Round 5

| Finding | In r6 |
|---|---|
| Grok B1 — reader result never wakes the starter; no park row | Resolved for the fit-at-once path and for the park row itself. The bytes a parked upload waits on now arrive at physical end, and that arrival does not wake the starter after the release. Finding 1. |
| Astra M1 — drain releases bytes physical cleanup still owns | The rule is written: later of drain and physical end, on every row, with both orders tested. Applying it is Finding 1. |
| Grok M2 — `forget` of an in-flight write aborts it | Resolved for a write already on the wire. A write that has not been accepted by an adapter thread is removed. Finding 2. |
| Grok M3 / m7 — carried upload, and a 303 that drops the body | Resolved. No outstanding hop releases the upload at the terminal transition, and that release is still the later of that moment and the buffer's last release. A 303, or a 301/302 of a non-GET/HEAD, ends the charge with the hop that sent the body. |
| Astra M2 — file-backed streams can deadlock | The deadlock is closed: a stream never parks, and a file that does not fit is refused. The test asserts an outcome that charge cannot produce. Finding 4. |
| Astra M3 — ordered slot freed, adapter thread still stuck | The straggler and the unarmed wait are written. `forget` and the upload deadline disagree with them. Findings 2 and 3. |
| Astra M4 / Grok M4 — gates | Resolved. C is compared with today's A6. A treatment failure stops that comparison whatever the controls did. A control failure reruns the sequence only when every treatment run completed, and only once. Preconnect's cold check is on the C runs, and a positive `domainLookup` is evidence. |
| Astra m6 / Grok m5 — two upload deadlines | Resolved on the Apple path. One deadline, disarmed only for a byte park, generation bumped, re-armed if it starts again. Linux puts an adapter wait inside that clock. Finding 3. |
| Grok m6 — failure rows drop the no-copy buffer early | Resolved. Every row uses the upload rule, which waits for the buffer's last release. |
| Grok m8 / Astra n7 — handoff test waits for physical end | The test now waits for the terminal transition. Nothing on the wake list runs the starter at that transition. Finding 1. |
| Grok m9 — origin table under the exchange lock; sizing takes a full reservation | Resolved. The callback records the protocol on the exchange; the finisher writes the lane table under the state lock and then wakes the starter. Sizing holds the request buffers in the starter rule and in D5. |
| Grok n10 — "always applied" | Resolved. Metrics after one second are dropped, and an origin never reported as `h2` or `h3` stays at 6. |

## Findings

**1. BLOCKER — The release that unblocks a parked upload does not wake the starter after the bytes are free.** `llp/1120-network-concurrency-without-threads.rfc.md:173`, `:180-183`, `:197`, `:226`, `:299-300`, `:407`. `host/apple/src/executor_core.rs:829-858`, `:487-503`.

Drain no longer frees a timed-out exchange. Line 299 keeps the running charge until drain and physical end have both happened, and an ordered outcome shrinks only after physical end. The parked upload at line 197 is waiting for exactly those bytes. `drain` can fire first, find the charge still held, and go back to sleep. The later `didComplete` is what actually frees the bytes.

That callback is specified to wake the starter itself (line 173) and, separately, to enqueue the finisher (line 181). The protocol report was given the safe order: the finisher writes the lane table, then wakes the starter (line 183). The charge release was not. A starter that runs the callback's wake before the finisher subtracts the bytes sees the old total, parks, and sleeps. The finisher's release has no wake of its own. Nothing else is coming. The upload stays at the head.

The same gap is why line 407 does not happen as written. The timer posts the terminal event to the finisher and does nothing else (line 226). The ordered slot opens on the finisher. The wake list has no terminal transition. Today's worker calls `complete` and takes the next job on that same stack (`executor_core.rs:1110` then the loop). Here the next handoff waits for whatever listed event arrives later. The finisher has to free the slot, release a charge, or record a per-origin return under the state lock, and only then wake the starter. The callback only enqueues.

**2. MAJOR — A write waiting for an adapter thread is dropped.** `llp/1120-network-concurrency-without-threads.rfc.md:243`, `:235`, `:371`, `:422`. `llp/1041-graceful-overload.rfc.md:698-699`. `host/apple/src/executor_core.rs:528-539`.

Line 243: a job waiting for a thread has not started, and `forget` removes it. Line 422 repeats that for the ordered fetch that waits at two stragglers. Line 235 and §3 say a queued write still executes, and a running write keeps the slot until its own end. LLP 1041 §8.4 says the same, and `forget` implements it: a queued safe read is dropped; every other queued job is marked and kept (`executor_core.rs:532-538`).

An admitted POST that has not yet been accepted by an adapter thread is still a queued write. Removing it drops the write. Apple does not have this wait, so the two hosts disagree, and §3's forget paragraph is not true of D4. The removal has to be the queued-read rule: a safe read goes; a write, `Now`, storage, or handoff stays marked and runs when a thread accepts it.

**3. MAJOR — A file upload's deadline runs through the adapter wait that D4 leaves unarmed.** `llp/1120-network-concurrency-without-threads.rfc.md:218-224`, `:243-245`, `:286-290`. `host/apple/src/executor_core.rs:1231-1238`.

Line 219 names three arm points as alternatives. On a Linux or render `exactBodyFrom` they are three different moments. D5 arms at the sizing reader's start, keeps that deadline when the charge fits, and disarms it only to park for bytes. The reader then posts the bytes, and only then does the starter resume HTTP. Resume needs an idle adapter thread. With two stragglers, or with the counter exhausted, there is not one.

D4 says that wait is in the core, not started, with no deadline. The deadline is already armed, and a byte park is the only disarm. A 100 ms POST that has already paid for its read times out before any hop, which is the case line 301 then settles as "no hop outstanding." Today one `Deadline::arm` covers stat, read, and the exchange because the worker that armed it is the thread that sends (`executor_core.rs:1235-1272`). There is no second wait inside the clock.

Sizing has to stay behind adapter acceptance on this host, with the one deadline armed at accept, or the wait has to disarm and bump the generation the way a byte park does. §3's "one deadline, unless it parked for bytes" does not describe this wait.

**4. MINOR — The two-stream test cannot happen under the charge it cites.** `llp/1120-network-concurrency-without-threads.rfc.md:278`, `:291`, `:417`. `host/apple/src/executor_core.rs:369-371`, `:1007-1008`.

The deadlock is closed. A stream is admitted with its charge or refused, then the file is taken at once or the stream is refused. It never parks.

`admit` charges `limit` for a stream, not the response ceiling alone: request buffers plus `max(2 × ceiling, 32 KiB)` plus `2 × 64 KiB`. Two independent streams with a 12 MiB ceiling hold at least `2 × (24 MiB + 128 KiB) = 48.25 MiB` of the 64 MiB lane before either file is charged. Free bytes are at most 15.75 MiB. A 20 MiB file fits neither, so the first is refused after sizing too. If the first is admitted, charged, and started before the second is admitted, the second fails at admission (`limit` no longer fits), which is the old refusal, not "refused after sizing."

Line 417 asserts the first starts and the second is refused after sizing. An implementer who makes that assertion true has to under-charge admission, which is the reservation `admit` takes so one event can grow to twice the ceiling. The test has to expect both refused when both were admitted first. The stream sentence also never says the file is read and the task resumed after the charge; the fetch path at line 290 does.

**5. MINOR — The sizing reader still holds its permit when the read starts.** `llp/1120-network-concurrency-without-threads.rfc.md:286`, `:290`. `host/apple/src/executor_body.rs:120-140`.

The sizing reader posts, then exits, and the permit drops when that thread ends. The starter can handle the post first. Phase 2 asks for a new permit. Four uploads sizing at once hold every permit, all four posts arrive, and every phase-2 acquire fails with "the app's files are not answering" while those readers are on the way out. The permit has to be released before the result is visible, or phase 2 has to be allowed to take the permit the sizing reader is returning.

**6. MINOR — The Linux thread bound does not count the threads r6 adds.** `llp/1120-network-concurrency-without-threads.rfc.md:245`, `:266`, `:422`, `:478`.

A straggler keeps its permit and exits rather than returning to the pool. The replacement is an extra thread from the process counter. One straggler beside one live replacement is one thread above the base of 15. Sixteen executors in that state are 256 executor-owned threads. Line 266 still says they reserve 240, and line 478's gate is `15 × live executors` plus streams, sockets, readers, and retiring threads. A faithful run fails the gate.

Line 245 also creates a replacement "while the executor has at most two stragglers" and waits only "past that." At two, "at most two" is still true, so a third thread can be created. Line 422 waits at two and does not start. One of those is the cap.

**7. NIT — A reader's failure is a finisher event and not a terminal transition.** `llp/1120-network-concurrency-without-threads.rfc.md:182`, `:227-233`. A failed stat and a firing deadline can both post. The request lock picks one winner. The numbered list has no reader outcome, so the winner's `Refused` is inferred from D5.

**8. NIT — "Stops the campaign" is written inside the Apple gate and names the other experiments.** `llp/1120-network-concurrency-without-threads.rfc.md:454-456`. A treatment failure stops the comparison that owns that arm. A Linux or C+P failure has to leave the Apple gates able to finish.

## What holds

The direction is unchanged and still right: one session per lane, task delegates, byte and count caps, upload bytes in the charge, Linux on the same core.

The round-5 Apple upload path is otherwise closed. A reader result is on the wake list. A 1 KiB file that fits is read and resumed under the deadline that was armed at sizing. A file that fits the budget but not the free bytes returns to the head with the deadline disarmed, the generation bumped, the waiting charge kept, no slot, and no in-flight count. The atomic take is the running charge plus the file, less the waiting charge already held. A stale firing does nothing. Two uploads cannot each hold the reservation the other needs.

`forget` of a write that is already running publishes nothing early, keeps the ordered slot until that write's own end, and `complete` drops the outcome. A safe read is the cancellable terminal event. The terminal list matches those two cases, and a late firing after success, abort, or timeout loses.

Upload release matches the hops. The charge ends at the later of the last sending hop's physical end and the lent buffer's last release; with no hop outstanding, the first of those is the terminal transition. A refused redirect and a timeout while the next hop waits for the cap are in the table. A 303, or a 301/302 of a non-GET/HEAD, stops carrying the charge. `dataWithBytesNoCopy` over the charged buffer is the right fix for `dataWithBytes:length:` (`vendor/ibex/crates/ibex2/src/transport/darwin_http.mm:347`). Failure, forget, and retirement use that same upload rule.

Per-origin start is at most six HTTP/1.1 exchanges per lane, lifted when the finisher records `h2` or `h3` and restored on `http/1.1`, under the state lock. A head at that limit is passed over, another origin starts, and the ordered lane has its own count. The seventh has not been resumed, so its deadline is not armed. The in-flight cap is resumed independent HTTP exchanges, 32 and 8, held to physical end. Handoffs, `Now`, undrained completions, and streams stay outside it. Platform queuing after `resume` is inside `exactTimeout` and declared.

Logical and physical release match `complete` where the table names the path. An interested independent outcome keeps the running charge, less the upload, until both drain and physical end. Ordered keeps what it retains, and shrinks only after physical end. The ordered slot ends at the terminal transition for a stalled reader, a cancel, and a normal completion, except for the wake in finding 1. The finisher publishes a timeout without waiting for `didComplete`, the reader, or the resolver (`vendor/ibex/crates/ibex2/src/transport/rustls_http.rs:188-213`).

Thread reservation is consistent apart from finding 6. Apple's base is 6. Linux and render add one ordered adapter and eight independent ones, 15. Sixteen executors reserve 96 or 240 before stragglers. Creation is all or nothing. Threads never spawned return their permits. Readers and Linux streams refuse when the counter cannot supply them. A stuck resolver keeps its own executor's thread.

§5 can decide the session change and does not bless the seventh connection, the jump from 7 to 32, or a win that is slower than today's six workers. Gate 4 is a hard ceiling on median and worst against A6, so one noisy worst sample rejects a genuine win. That fails closed. Preconnect is a separate experiment. Its cold check runs on C, where the flag did not touch DNS, and a failed check makes the cold-cache claim inconclusive instead of discarding the treatment.

§3 holds for D1, D2, D6, and D7, and for D3 and D5 on the Apple path: admission, the fence, re-asks, settlement through `ordered`, grants, explicit independence, per-hop headers, and the declared platform queue. It does not hold for D4's `forget`, or for a Linux file upload's deadline.

An implementer would still have to invent the finisher-then-wake order (finding 1), which jobs survive `forget` while waiting for a thread (finding 2), and where a Linux upload's deadline sits across that wait (finding 3). Also still unwritten: the render host with `EXACT_RENDER_TRANSPORT=platform`, which is Apple's session path inside the host whose row is the 15-thread adapter (`host/render/src/executor.rs:32-40`, RFC line 239). r5 named it. "By default" does not pick the base set.

## Verdict

**RIGHT WITH CHANGES.** Rounds 1–5's direction holds, and the round-5 Apple upload, release, and measurement rulings are in the mechanism. Finding 1 strands a parked upload that the new release rule was supposed to unblock. Findings 2 and 3 have to land before §3 is true on Linux and the render host.
