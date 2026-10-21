# Review: LLP 1120 §5's as-measured Apple gate round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `a3234db1ea64541178d5e7e6fc4b47251e3f6ea186d8f5ff682cc0c832b92f5e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 3's text: the claim per exchange with the partial launches disclosed, gates 3 and 4 failing with gate 5, the CPU reading as a judgement, two to seven, the omitted recordings named, the table's rises, the gated pairs.

---

- **MAJOR — [1120…rfc.md:549](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:549):** The connection claim still exceeds the evidence. Window 2’s 16.5-second C launch has **six exchanges without endpoints or protocol**, including five initial feed reads ([raw lines starting at 41](/Users/admin/x2p2/gate/lte-2/C-2.jsonl:41)). Only its remaining 21 exchanges identify one HTTP/2 connection. Qualify the claim per **exchange**, and disclose this partial coverage. A non-null first-post time also does not establish that this launch completed all 19 reads for gate 5.

- **MINOR — [1120…rfc.md:554](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:554):** “A 15 s stall is not what CPU contention looks like” remains an unsupported causal assertion outside the hypotheses qualifier. The configured deadline explains the duration without identifying the underlying cause. Remove this sentence or explicitly label it a hypothesis.

- **MINOR — [1120…rfc.md:554](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:554):** “Two to six of 28” misses an observed seven-exchange connection. Window 1, A6 block 3, launch 2 assigns seven exchanges to `10.0.123.4:64480` ([raw data](/Users/admin/x2p2/gate/lte-1/A6-3.jsonl:29)). The observed range is two to seven; neither range establishes the counterfactual loss under a stall.

- **MINOR — [1120…rfc.md:555](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:555):** “Non-gating recordings beyond the connection log” overstates what remains undone. `connect_to_post` was recorded in the JSON results and is quoted immediately above. Name the measurements that were actually omitted.

All 33 table statistics match independent calculations with linear-interpolated quartiles and displayed rounding. All 144 launch entries are present, with one null. The window-1 pagination exception and per-arm shifts check out; the scoped gate verdicts, gate-1 failure, and decision to keep the switch off remain supported.

**NOT READY**
