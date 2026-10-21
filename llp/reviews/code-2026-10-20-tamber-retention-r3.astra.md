# Code review: LLP 1121 stage 12 B1 follow-up, the retention build: stop at retention, reuse types and expansion, round 3 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `ec35893f05`.
- **Method:** one brief (sha256 `9fd08efb38e785ac75ff8a2c570ba9c1055be0eb684d04653771aaf7aa7f96e0`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (2 NIT).
- **Disposition:** nits taken where cheap (the commit message).

**BLOCKING: none. SHOULD-FIX: none.** I found no new cached-versus-fresh divergence in the changed retention-build path.

- **NIT — Direct oracle coverage remains narrower than its comment.** [tamber_retention_reuse.rs:386](contract/cli/tests/it/tamber_retention_reuse.rs:386) claims every design-only edit kind, but the raw retention/candidate comparisons cover six cases. Package alias moves, package-header repair and motion arguments still compare proposal JSON only. Extend the oracle or narrow the comment.

- **NIT — Benchmark attribution needs updating.** The commit’s [benchmark paragraph:68](/tmp/rl/ret-msg.txt:68) calls the earlier `contract-ret2` measurements “this tip.” The later [ret-bench2.txt:1](/tmp/rl/ret-bench2.txt:1) records `contract-ret3`: alias medians **686–687 ms versus 2730–2787 ms**, and value medians **270 versus 249 ms**. The speedup remains substantial; identify the measured revision explicitly.

What checks out:

- **Round 1:** scoped restoration of `PROGRAM` and the previous flag handles unwinding; no reachable reentrant build caller found. Parse failures force fresh checking. Tests exercise production reuse and VERIFY, plus synthetic expressions, package-header repair, parse recovery, strings, motion arguments and pre-theme failure.
- **Round 2:** the new whole-build oracle directly compares retention and nonempty candidates. Fresh-check rejection now calls `refused_fresh`, producing `built stale`. The package-source and cache-hit comments are corrected.
- **Step 1:** retention and candidates are assigned before the stop and never rewritten afterward. `params::reached` contributes diagnostics only; earlier passes do not observe the flag. `retention().is_some()` identifies completed retention, not successful compilation.
- **Step 2:** equality covers the merged app/package program, strings and mapping mode. Motion substitutions remain compared; no omitted current typing/expansion input found. Source-key changes discard `kept`; VERIFY checks both successful differences and fresh refusals.
- **Q2:** the kind-change test matches the corrected ruling. The commit explicitly leaves the background whole-build guard to the next commit and claims no worker here.

No files changed. Whitespace check passed; working tree is clean. Tests and benchmarks were not rerun because they write files.
