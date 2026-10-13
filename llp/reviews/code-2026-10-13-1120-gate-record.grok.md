# Review: LLP 1120 §5's as-measured Apple gate, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `25d9d3e9502af87e404f1e80f1407eb18c80be10598d5e18c96b111febae0636`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's text: gate 5 not discounted, the stall's causes as hypotheses, the connection claim limited to recorded endpoints, the log's time at completion, the 27-exchange launch, the per-arm shift.

---

The table and the gate-1 decision match the raw launches. Two mechanism claims in the new record do not.

## MAJOR

**[MAJOR] The connections-per-launch sentence counts a launch that never opened an h2 connection — `llp/1120-network-concurrency-without-threads.rfc.md:549`**

Window 2’s failed C launch (`lte-2/C-2.json`, cold 1, `null`) has 12 connection-log lines, all with `protocol` null and no local or remote endpoint, in two clusters 15.0 s apart. It did not open an h2 connection. The other launches in windows 2 and 3 do match the stated counts (C7 and C one, A7 seven, A6 six), and window 1’s C launches are one each. Line 554 calls this same failed launch “the one `h2` connection.” The simulator-log fact stands: a 15 s setup stall cancelled that launch’s requests (`NSURLError` -999). The connection log does not show an h2 connection for it.

**[MAJOR] The overlap caveat has the log’s clock backwards — `llp/1120-network-concurrency-without-threads.rfc.md:549`**

`t` is `SystemTime::now()` inside `note()`, and both transports call that from `didFinishCollectingMetrics` (build `0612b2087`, `darwin_task.mm` and `darwin_http.mm`). That is the completion of the task. The log has one timestamp per exchange, so overlap on the one connection is still not shown, and gate 2’s qualified pass can stand. The missing edge is the start.

## MINOR

**[MINOR] Gate 5’s “28 exchanges” misses one window-1 C launch — `llp/1120-network-concurrency-without-threads.rfc.md:552`**

`lte-1/C-2` cold 1 completed (`zero_to_post` 2002, the window’s worst C) and logged 27 exchanges: 20 `getAuthorFeed` calls against 21 on every other complete launch. Windows 1 and 3 have no null entry, so the pass still fits the failure rule. The parenthetical does not.

**[MINOR] The cross-window shift is not about 120 ms on every arm — `llp/1120-network-concurrency-without-threads.rfc.md:546`**

Median `zero_to_post` rose 177 ms on A7 (1629.5 to 1452.5), 116.5 ms on A6, and 121 ms on C. Dropping A7’s first block leaves a median of 1621, still about 169 ms above window 1. The cited block medians 1770, 1613, and 1634 are right, and they explain A7 against the other arms inside window 3.

**[MINOR] A worker stall drops one connection’s exchanges, which is several — `llp/1120-network-concurrency-without-threads.rfc.md:554`**

On window 3, A7’s seven connections carry 2–5 of the 28 exchanges and A6’s six carry 4–6. No worker launch in these windows lost an exchange to this stall; every worker launch logged 28 h2 exchanges. The single-connection failure mode is the real contrast.

## NIT

**[NIT] Two “every” claims in the setup sentence have disclosed exceptions — `llp/1120-network-concurrency-without-threads.rfc.md:536`**

Window 1’s three C7 blocks log `transport` `session` and six connections; the next bullet says that arm ran the workers. Eighteen lines in the failed C launch have `remote` null. The other 3996 lines are `public.api.bsky.app` at `169.150.221.147:443`, the address `shape-on.sh` shapes.

The recomputed table matches, including linear-interpolation IQRs. Gate 1 fails: the rounded medians differ by 10 ms (exact medians 1629.5 and 1620.5) against A7’s IQR of 284, while C7’s worst is better (1943 against 2810). Gates 3 and 4 pass on the clean windows. §4 keeps `EXACT_HTTP_ASYNC` off until this gate passes, so the switch staying off follows. Window 3’s log runs 06:18:44–06:26:15Z. The xrpc timeout is 15 s (`data/atproto.ts`). The 16.5 s launch’s `connect_to_post` is 16263 ms with a normal hatch and runtime zero.

NOT READY
