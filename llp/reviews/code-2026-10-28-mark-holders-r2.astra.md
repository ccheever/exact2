# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `2eb32ba82f`.
- **Method:** one brief (sha256 `4ada0ea68afa78bade0f110005d19cc6f3f468e3ded8bfb25c48924a8ff13b4e`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [kernel/src/kernel/intrinsic.rs:57](kernel/src/kernel/intrinsic.rs:57): Equal-size reports bypass bookkeeping.** The return at line 26 skips updating `mark_reports`. In `a_mark_holds_*`, add a fresh `(22, 22)` report for node 3 after changing it from radio to checkbox. Withdrawing node 2 then incorrectly shrinks node 4 to `(13, 13)`: node 3 remains recorded as a radio despite its new checkbox report. An equal-size non-mark report likewise leaves the old record. The fast path must compare report keys as well as dimensions. The three tests miss this case.

- **BLOCKER — [kernel/tests/it/browser_controls.rs:1517](kernel/tests/it/browser_controls.rs:1517): The added test breaks the mandatory line cap.** Running `bun scripts/caps.mjs` reports 1,518 lines against the 1,500 limit. Split the tests or reduce duplication.

No other lifecycle findings: symbol measurement cannot overlap a live mark, and clone/reset/free-slot cleanup and `mark_reported`’s fast path preserve the record. The additional IdMap insertion is expected amortized O(1) and preserves the no-relayout path; performance was not measured.

Kernel tests were reviewed but not run: no build artifacts were available, and the read-only sandbox also blocked caps’ Cargo subcheck.

VERDICT: findings
