# Code review: the runner's substitute cache, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ff6c862e3f`.
- **Method:** one brief (sha256 `9d1607c456db0a8217478558f6231c774e137fd2b90ff60e27b118898fa82fbd`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The round-2 counter bug is fixed, and the substitute cache is correct. The only test that reads `SUBSTITUTE_PARSES` zeroes it before boot (`runner/src/runner/modules/substitute_tests.rs:117`), so an earlier test on the same thread cannot move the count.

The cached value is the plan string after `this_target`, and the plan is not written again on a live runner. `set_module_target` drops the map whenever the host target string actually changes, and it does that before `announce` refills it. `substitute_for` and `member_answering` clone the `Json` before any change, and `Arc` does not allow in-place mutation of the cached value. The map holds at most one entry per substitute string and is cleared on a target change. The scroll test commits a handled scroll, and the target test checks both the filtered permission and the one-entry bound across 50 targets.

VERDICT: APPROVE

No findings.
