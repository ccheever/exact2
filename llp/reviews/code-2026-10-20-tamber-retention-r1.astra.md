# Code review: LLP 1121 stage 12 B1 follow-up, the retention build: stop at retention, reuse types and expansion, round 1 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `71541de4e8`.
- **Method:** one brief (sha256 `dec4703c351116def0c9c79c45643aea881ca8aee59d086d229f77c623bbe17b`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, 3 SHOULD-FIX).
- **Disposition:** each finding taken in the next amend (the commit message's round section).

**One BLOCKING finding; three SHOULD-FIX findings.**

- **BLOCKING — Q2’s client refusal is missing.** [tamber_retention_reuse.rs:248](contract/cli/tests/it/tamber_retention_reuse.rs:248) explicitly expects the invalid `gap: 8px → #444444` transaction to succeed as `rebuild`. Only a separate compiler invocation observes `tamber-kind`. That contradicts the ruling’s required refusal to the client and [tamber_serve.rs:403](contract/cli/src/tamber_serve.rs:403)’s claim that write checks handle the failure. Existing behavior on main does not discharge this condition.

- **SHOULD-FIX — Thread-local cleanup is not unwind-safe.** [retained.rs:87](contract/cli/src/retained.rs:87) resets the flag and `PROGRAM` only after compilation returns. A caught panic leaves subsequent builds on that thread in retention-only mode; nested calls would also overwrite the outer state. Use a scoped restoration guard. Normal `Result::Err` returns do clean up; I found no current reentrant caller.

- **SHOULD-FIX — Parse failures do not enforce condition (c).** [retained.rs:109](contract/cli/src/retained.rs:109) compares after removing both designs, without requiring successful parsing. For an app without motion substitutions, malformed design text can leave the compared program identical: `design_tokens` and motion extraction tolerate parse failure. Reuse therefore proceeds instead of falling back to step 1. Current design-independent typing makes this conservative-rule violation distinct from demonstrated stale output.

- **SHOULD-FIX — The new comparisons bypass production reuse downstream.** [tamber_retention_reuse.rs:47](contract/cli/tests/it/tamber_retention_reuse.rs:47) always enables VERIFY, whose branch subsequently uses **fresh** types and expansion. The fixtures also have no nonempty synthetic expressions and never compare retention directly. Add unverified reuse versus full-compilation comparisons, nonempty candidates, malformed-design recovery, and the actual package-header repair case—the current header test merely renames a valid app header.

What checks out: retention and candidates are assigned before the stop and never changed afterward. Skipping `params::reached` removes diagnostics only; no earlier pass observes the flag. `retention().is_some()` correctly identifies completed retention, including empty candidates, without implying compilation succeeded. The current equality gate covers the merged package/app program, substituted motion literals, strings and mapping mode; I found no omitted current typing/expansion input. Source-key changes discard `kept`. VERIFY compares successful results and makes mismatches fail the expected-log assertions.

The [recorded benchmark](/tmp/rl/ret-bench.txt) supports the commit’s numbers: all medians check out, approximately **2.57 s → 0.56 s (4.6×)** for alias edits, with value edits essentially unchanged.

No files changed. Tests and benchmark were not rerun because they write files.
