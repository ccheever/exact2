# Code review: the wasm host's first-size boot races, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `e86ff7d22c9341dfaf5d17e0512002e91828ad27ba6459cf05b1a27613ad1346`), shared by both reviewers, each blind to the other's review; HEAD `8d2e8a261e`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 2 SHOULD-FIX
- **Disposition:**
  Neither reviewer found anything blocking. Both of Astra's SHOULD-FIXes are taken post-round, in their own commit:
  - Astra 1 (a clock's due timers waited only on `inflight`, which the JS target's hold is in): the hold is also in `sizes`. A clock's due-timer step waits for that set (`mediaAttached(deadline, sizes)`), as it waits for requests. A reload clears it. Module activation still does not wait for it.
  - Astra 2 (Caltrain's own startup loads, its images' hold, could pass the 300 ms and mask a missing hold): the held case settles the Caltrain page before the reload.
  Rechecked by mutation: the merge-base's glue.js fails (2 ms), no geometry chain fails, no no-box release fails. Full synthetic conform was rerun on the final tree.

Nothing blocks. The four round 1 findings are closed. Two SHOULD-FIX findings remain:

1. **SHOULD-FIX — [host/web/glue.js:558](/tmp/rv-dt-wrb2-astra/host/web/glue.js:558): clock jumps lose the JS target’s pre-timer wait.** Moving the hold to `mediaAttaching` preserves `clock settle` and `clock data`, but the due-timer path at line 1238 waits only on `inflight`. With a first report pending, a timer can therefore read pre-resize state on wasm while JS waits for the report. Conform’s boot settle remains protected; a later settle cannot repair state already captured by a timer. Include first-size holds in the clock’s pre-timer wait while keeping module activation’s drain separate.

2. **SHOULD-FIX — [host/web/tests/resize-boot.test.mjs:126](/tmp/rv-dt-wrb2-astra/host/web/tests/resize-boot.test.mjs:126): unrelated loading can mask a missing hold.** The test releases reports after 300 ms, but `clock()` first awaits `imageHold` at glue.js:1227. Neither `booted()` nor `__observed` guarantees that load has finished. If it takes over 300 ms, reports land before settlement and every assertion can pass with the resize hold removed. Settle the initial Caltrain page before reloading the fixture to drain unrelated startup work.

Release accounting otherwise looks correct: first report, dispatch exceptions, no box, timeout, both destroy sites, reload, and loader errors resolve the promise idempotently; set removal happens once. The two-second bound starts **after loading**; stalled loading can consume the 20-second settle deadline. Module activation and the normal page clock do not wait on this set.

The revised geometry gate and `EXACT_WEB_LINK=all` prerequisite address their round 1 findings. Read-only review; no builds or tests run.
astra exit 0
