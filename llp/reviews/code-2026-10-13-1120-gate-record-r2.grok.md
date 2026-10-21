# Review: LLP 1120 §5's as-measured Apple gate round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `a3234db1ea64541178d5e7e6fc4b47251e3f6ea186d8f5ff682cc0c832b92f5e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 3's text: the claim per exchange with the partial launches disclosed, gates 3 and 4 failing with gate 5, the CPU reading as a judgement, two to seven, the omitted recordings named, the table's rises, the gated pairs.

---

The table matches the raw `zero_to_post` values. One connection claim does not, and two gate labels do not follow the rule above them.

**MAJOR** — Window 2's slow C launch is left out of the one-connection claim — `llp/1120-network-concurrency-without-threads.rfc.md:549`

`lte-2/C-2` cold 2 (`zero_to_post` 16505, `connect_to_post` 16263) logged 27 exchanges. Six have null `local`, `remote`, and `protocol` (`getServices` plus the first five `getAuthorFeed`s). The other 21 share one `h2` connection (`10.0.123.4:51558`). The sentence says every launch with recorded endpoints put all of its exchanges on one connection, and that only the failed launch recorded none. This launch has recorded endpoints and does not. The failed launch itself is as written: 12 lines, no endpoint or protocol, two clusters 15013 ms apart.

**MINOR** — Gates 3 and 4 are marked passed after a C failure that stops C's comparison — `llp/1120-network-concurrency-without-threads.rfc.md:550`

The rule at line 506 says a failed treatment run fails that arm's gate and stops its comparison, and that a treatment failure is not rerun. Window 2's null C run is that failure, and gate 5 says so. Gate 3 (line 550) and gate 4 (line 551) are C's comparison, and both are labelled **Passes** on window 3. The window-3 inequalities hold (C 1602 vs C7 1620, worst 1932 vs 1943; C 1481/1602 vs A6 1508/1624, worst 2002/1932 vs 2351/1955). The pass labels do not.

**NIT** — The window-1-to-3 rises disagree with the table by 1 ms — `llp/1120-network-concurrency-without-threads.rfc.md:546`

Unrounded medians rise 177.0 (A7: 1629.5−1452.5), 116.5 (A6: 1624.0−1507.5), and 121 (C). The table's rounded medians differ by 178, 116, and 121. The sentence says 177, 117, and 121.

**NIT** — "Async was not slower" does not hold for every clean-window pairing — `llp/1120-network-concurrency-without-threads.rfc.md:553`

In window 1, C's median is 1481 against A7's 1452. The gated pairs do hold: C is under A6 in windows 1 and 3, and C7 is under A7 in window 3.

Round 1's other fixes hold. Gate 5 fails for C on the window-2 null, and contamination is not used to drop it. The stall's cause, the worker counterfactual, and the timeout-and-retry idea are labelled as hypothesis or "worth trying". `t` is written from `didFinishCollectingMetrics` / lane `metrics` at `SystemTime::now()`, so overlap is correctly "not measured" and gate 2's verdict is right. The window-1 C launch with 27 lines is one `wired.com` cursor page fewer; all 18 first-page author feeds and both `getFeed`s are present. The per-arm rise gives no cause. Medians, IQRs, and worsts in the table match the helper's linear-interpolation IQR, including window 2 C at n=11 (1521, IQR 678, worst 16505) plus the separate failure. A7 and A6 are seven and six `h2` connections on every launch; C7 in windows 2 and 3 is one; window 1 C is one; window 1 C7 is `session` with six connections. Window 3's exchanges run 06:18:44Z–06:26:15Z.

NOT READY
