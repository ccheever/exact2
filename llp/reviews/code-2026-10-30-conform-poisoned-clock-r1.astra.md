# Code review: the conformance drive's clock refusals on a poisoned runner, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fa6609d320`.
- **Method:** one brief (sha256 `6cb921af6097e599d43aee79fa3c768aa3f9a2620d30acf6d29ca90aa1e0de22`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **Blocking: a Linux disagreement can pass `--strict`.** At [conform.mjs:511](host/web-js/conform.mjs:511), mismatches throw inside `onLinux`, whose catch records a skip without calling `fail`. Concrete input: wasm refuses `clock: Poisoned`, JS refuses `clock: t=0 refused advance: Poisoned`, Linux answers `{clock: 0, settled: true}`. The result has **zero failures** and already records “Poisoned on every target.” A different Linux refusal produces the same false pass. I reproduced this using the committed functions in memory.

   This preserves Linux’s existing stop behavior and the correct `settle` / `clock +60000` keys, but violates the stated **every-target** acceptance condition. Keep declared `LINUX_APART` stops; record unexpected clock disagreements as failures before stopping Linux. Report only the targets actually compared. Add an integration regression: the three-target `clockVerdict` test misses this because `clockAll` passes that helper only the two pages.

Non-blocking notes:

- The web verdict is appropriately narrow: normalization must yield exactly `Poisoned`; one-sided, differing and non-Poisoned refusals fail. Malformed web errors are rethrown. Linux malformed errors encounter the swallowed catch described above.
- Budget’s bound-reason/PC comparisons still run after accepted settles, including case 17. The change does not skip subsequent state/tree comparisons.
- The saved [budget report](/tmp/exact-web-js-conform/report.md) shows zero failures and 73 equal entries: **60 ordinary comparisons plus 13 refusal entries**, including post-poison `argtrap` and the closing jump. These count comparison records; the closing jump has two records. This report contains no Linux comparisons.
- [conform.mjs:506](host/web-js/conform.mjs:506) fixes stale error attribution, though the closing failure label becomes `clock +60000 clock +60000`.
- The seven added assertions pass in memory, but cover the verdict helper rather than recording or continuation. The permitted Bun suite stalled after its banner and was interrupted; I cannot claim a fresh suite pass.

CHANGES
