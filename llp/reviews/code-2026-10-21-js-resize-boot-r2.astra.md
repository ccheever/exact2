# Code review: the JS target's first-size and geometry boot races, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `1d678e8c7d3338bd0e6cb2a6755a16ba830fed52a9c4b0f6e34c2735cc79348f`), shared by both reviewers, each blind to the other's review; HEAD `7e202eb8fa`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX on the test; the wasm host's, QUEUE'd).
- **Disposition:**
  Nothing blocks from either reviewer. The tests' SHOULD-FIXes are taken post-round, without another round (noted in the sign-off request):
  - Astra 1, Grok NIT 2 (the gates assumed ordering): the held-report case starts its settle only once the tab row is observed, and asserts the settle waited (≥ 300 ms). The geometry case releases the module once the tab row was observed and two frames passed (when an unchained glue would have reported on unavailable frames), or after 1.5 s with nothing observed (the chained glue, which waits).
  - Astra 2, Grok 1 (the no-box assertion on the bound's clock): the held-report page sets no bound (`__exactFirstSizeBound`, a test's knob in observers.js), so a no-box element that isn't let go at once leaves the settle unsettled.
  Mutation-checked: main fails (72 ms, no wait), no chain fails (zeros), no immediate release fails (settled: false).
  - Astra 3 (the wasm host): already QUEUE'd (round 1).

**Nothing blocks the JS runtime fix.** Round 1’s cleanup issue is closed; the tests remain partly dependent on timing, and the wasm issue is deferred.

1. **SHOULD-FIX — [resize-boot.test.mjs:85](/tmp/rv-dt-rb2/host/web-js/resize-boot.test.mjs:85): the gates still permit broken code to pass under load.**  
   A2/B2 are only partly closed. Reports now stay gated until the test starts, and reading state in the same evaluation removes the driver gap. But opening each gate after 300 ms still assumes ordering: resize attachment can happen after the report gate opens, and geometry can finish before an unchained observer delivers its first report. Either mutation can therefore pass. Synchronize on the relevant attachment/loading checkpoints and control report delivery explicitly.

2. **SHOULD-FIX — [resize-boot.test.mjs:109](/tmp/rv-dt-rb2/host/web-js/resize-boot.test.mjs:109): the no-box assertion measures from the wrong starting point.**  
   A3/B1 are improved, but not fully closed. Suppressing hidden reports at line 73 is correct. However, the two-second fallback starts at attachment, while `held.ms` starts later, when settling begins. If attachment precedes that by over 500 ms, removing immediate no-box release can still satisfy `<1500`. Conversely, scheduling delays can fail correct code. Control the fallback timer or inspect the outstanding hold while its expiration is prevented.

3. **SHOULD-FIX, pre-existing — [glue.js:558](/tmp/rv-dt-rb2/host/web/glue.js:558): the wasm oracle still has both races.**  
   A4 is recorded in [QUEUE.md:1605](/tmp/rv-dt-rb2/QUEUE.md:1605), not fixed. Resize reporting remains untracked, and geometry installs independently at [glue.js:1440](/tmp/rv-dt-rb2/host/web/glue.js:1440). This matters for conform: opening wasm first gives it time, but [conform.mjs:501](/tmp/rv-dt-rb2/host/web-js/conform.mjs:501)’s settlements provide no missing ordering guarantee. A wasm-side loss can still produce a mismatch. This remains a separate follow-up for the scoped JS fix.

**A1 is closed** at [observers.js:15](/tmp/rv-dt-rb2/host/web-js/observers.js:15), line 17 and line 22: release is idempotent, cancels the timer, runs on scope teardown, and runs in the report callback’s `finally`. First report, no box, timeout and setup failure each release once. Every registration owns its flag, so sharing the promise and observer does not double-decrement. Removal outside scope teardown falls back to the timer.

The diagnosis is correct: [agent.js:376](/tmp/rv-dt-rb2/host/web-js/agent.js:376) waits on `inflight`, while [rt.js:1089](/tmp/rv-dt-rb2/host/web-js/rt.js:1089) returns unavailable geometry with width zero before `Geo` exists. Attachment alone guarantees neither reporting nor geometry availability.

The geometry chain has no cycle. [emit.rs:546](/tmp/rv-dt-rb2/host/web-js/src/emit.rs:546) emits synchronous mounting followed by `geo()`, before the deferred `geoLoading()` lookup runs. Pages without geometry receive `null`; geometry failures are logged and allow resize attachment, with frames remaining unavailable.

There is no universal two-second delay or new paint barrier. `__exactRender` still bypasses both paths. Agent-off pages intentionally delay resize attachment until geometry finishes. The two-second bound starts **after attachment**: stalled imports can still exhaust settle’s existing twenty-second deadline and return `settled: false`.

Read-only inspection; no changes, builds or tests run.
