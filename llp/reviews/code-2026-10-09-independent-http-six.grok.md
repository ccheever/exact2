# Code review: six independent HTTP workers (6677007d6), 2026-10-09 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4f07671a6f929f9339b69822b7e6ac30110daa96957b69f6463de715c99fd794`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra 1 taken: the ordered-releaser and stream saturation tests hold every independent worker (`INDEPENDENT`). Astra 2 / Grok MINOR and NIT taken: LLP 1041's worker cap, Completion Storm's README, LLP 1054.000's refusal note and LLP 1110's long-poll cost say six workers, 64 MiB and 112. Astra 3 taken: the amendment says six bounds an executor's concurrent independent requests across origins, not connections per origin. `cargo fmt` applied.

---

The runtime change matches the claim. Linux and the render host compile the same `executor_core.rs`, so they pick up six independent workers and the 64 MiB lane with no leftover copy of the old constants. What is wrong is the surrounding text that still describes the old limits.

## MINOR

**LLP 1041 still states the process cap as 48** — `llp/1041-graceful-overload.rfc.md:495`

The new paragraph at line 448 says the bound is sixteen executors' workers, and the code is `MAX_WORKERS = 16 * WORKERS` with `WORKERS = 7` (112). The same section, still in the present tense, says each host "limits live and retiring executor workers to 48 until exit." Those two sentences cannot both be the current rule.

**Completion Storm still documents two workers, 32 MiB, and a cap of 48** — `apps/completion-storm/README.md:109`

This is the app LLP 1041 cites as the independent-HTTP opt-in. Line 60 still says held data uses two independent workers, lines 109–111 still say 128 requests, 32 MiB, and two data transports, and line 121 still caps live and retiring workers at 48. All three numbers are now 6, 64 MiB, and 112.

**The landed Bluesky admission note still describes the 32 MiB lane** — `llp/1054.000-what-the-bluesky-port-asks.rfc.md:51`

It says a ceiling the lane can never hold is "any `max_response_bytes` from about 16 MiB up," and it describes `an_independent_read_over_the_budget_waits_instead_of_refusing` as two 8 MiB reads. This commit moved that refusal to `32 << 20` and the wait test to two 16 MiB reads. The refusal is now about 32 MiB: `reservation` charges `request bytes + max(2 × ceiling, 32 KiB) + 128 KiB`, so a 32 MiB ceiling is over the 64 MiB lane and a ceiling near 31.9 MiB still fits.

## NIT

**LLP 1110 still says two long polls fill the independent lane** — `llp/1110-live-snapback-reads.rfc.md:379`

A long poll on `exactIndependentHttp` "occupies one of the native executor's two independent workers," and "two live streams fill them." The lane now has six workers, so two held polls leave four free. The draft is not accepted, and the cost it is arguing from is no longer the code on this commit.

## Checked, no defect

`LIVE_WORKERS` stays balanced. `with_owners_on` adds `owners.len()` once, and every iteration builds one `WorkerSlot`. A failed `spawn` drops that slot with the closure (one `fetch_sub`). A disabled core that never reserved spawns nothing. Retirement sets `retired`, each live worker returns, and its slot decrements. Partial spawn still refuses admission, and the threads that did start release their own slots on drop. Production `start` / `start_on` (Apple, Linux, render) always pass `owners()`, length 7. Test cores that pass `(0..WORKERS)` reserve the seven threads they spawn.

Ordering is unchanged: index 0 is the only lane-0 worker, and the ordered FIFO, the 512 MiB budget, the 16-ticket effect cap, and the 128-read window are untouched. `drain` still alternates when both lanes are ready, and an independent backlog does not sit in front of a newly ready ordered completion (`next` stays 0 while only lane 1 is drained). `forget`, retirement, `STREAMS` (16, own threads, not in `MAX_WORKERS`), and the 128-request cap are the same logic with a wider lane.

Six transports are the intended cost. Each independent worker calls `host()` once at thread start. `DarwinTransport` and the rustls trust store stay lazy until the first request, and each worker is one request at a time, so a cold `Promise.all` opens one session per worker (six), not the pool's four-session ceiling. Two embedded sessions are 14 worker threads, under 112. The render host's per-worker executor cache still tops out at sixteen full executors, the same count as before, at seven threads each instead of three.

The 2026-10-09 paragraph in LLP 1041 matches that behavior: six owners, 128 requests / 64 MiB, one ordered worker at 512 MiB, six 4 MiB reads fitting because each running charge is about 8.1 MiB (six × that is about 49 MiB). The seventh 4 MiB read still fits in the byte budget and waits on a worker, which is what `independent_reads_overlap_up_to_the_lanes_workers` asserts.

READY
