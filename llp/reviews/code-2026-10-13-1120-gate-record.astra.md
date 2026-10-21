# Review: LLP 1120 §5's as-measured Apple gate, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `25d9d3e9502af87e404f1e80f1407eb18c80be10598d5e18c96b111febae0636`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's text: gate 5 not discounted, the stall's causes as hypotheses, the connection claim limited to recorded endpoints, the log's time at completion, the 27-exchange launch, the per-arm shift.

---

- **MAJOR — [1120-network-concurrency-without-threads.rfc.md:552](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:552): Treatment failure cannot be discounted under gate 5.** Window 2’s `C-2.json` contains a null launch. §5 explicitly says a treatment failure fails that arm’s gate and stops its comparison. Record that failure alongside the contamination; clean-window results do not erase it.

- **MAJOR — [1120-network-concurrency-without-threads.rfc.md:554](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:554): The stall conclusions exceed the evidence.** Your supplied context establishes the failed launch’s 15-second setup stall and cancellations. It does not establish that the 16.5-second launch escaped the same stall just before its deadline, rule out CPU contention, or demonstrate that workers would lose only one request. Moreover, losing one required feed read also fails gate 5. Keep the observed failure; label the causal explanations and proposed remedies as hypotheses.

- **MAJOR — [1120-network-concurrency-without-threads.rfc.md:549](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:549): The connection claim has an exception, and gate 2 remains unproven.** The failed launch’s 12 records in `lte-2/C-2.jsonl` have null local endpoint, remote endpoint, and protocol—not an identified `h2` connection. Restrict the one-connection claim to launches with recorded endpoints. Required transaction overlap remains unmeasured. Also, despite the description of `t` as a start time, the named build’s `metrics_log.rs:54` timestamps metrics reporting; it does not record exchange start times.

- **MINOR — [1120-network-concurrency-without-threads.rfc.md:552](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:552): “Every launch made its 28 exchanges” is false.** Window 1, C block 2, `cold 1` has 27 records. The missing record relative to adjacent launches is the `wired.com` pagination request. All 19 initial feed reads are represented, so this discrepancy alone does not establish a gate-5 failure.

- **MINOR — [1120-network-concurrency-without-threads.rfc.md:546](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:546): The common slowdown and its cause are overstated.** Window 1→3 median increases are A7 **177 ms**, A6 **116.5 ms**, and C **121 ms**. C7 lacks a valid window-1 comparison. These measurements do not establish that the shaped path caused the change.

- **MINOR — [1120-network-concurrency-without-threads.rfc.md:553](/Users/admin/x2p2/apps/exact2-d4/llp/1120-network-concurrency-without-threads.rfc.md:553): Qualify “async is not slower.”** The evidence supports the stated latency comparisons in the clean windows, not an unconditional conclusion “whatever the noise,” particularly with window 2’s failure and 16.5-second result.

All table medians, linearly interpolated IQRs, and worsts reproduce with whole-millisecond rounding. All 144 launch slots are present, including the one null. Gate 1 fails; gates 3 and 4 satisfy their stated numerical criteria in the cited windows. Keeping `EXACT_HTTP_ASYNC` off is supported.

**NOT READY**
