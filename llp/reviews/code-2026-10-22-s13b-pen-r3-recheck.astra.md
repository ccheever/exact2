# Code review: stage 13 B, the pen importer, round 3 re-check (a75ca46936) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aec7cb74e9`.
- **Method:** one brief (sha256 `5c642a30ffb01d6b9599c99d60d6db828ad4af342fa451aa54bca0a8499d49d5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [dtcg.rs:960](tamber/src/import/dtcg.rs:960): **Uncharged error amplification remains.** A self-alias with a 64 KiB name, a short alias to it, and 10,000 colour aliases to that short name fit a 671 KiB wire request. Cycle errors retain the full name in both `memo` and `fates`: over 1.2 GiB before budget refusal.

- [dtcg.rs:1319](tamber/src/import/dtcg.rs:1319): **Uncharged quadratic cycle detection.** An 18,000-token colour chain fits a 950 KiB wire request. Ordering each target before its alias lets memoization bypass the recursion bound; `reaches` then performs approximately 162 million visits without charging the budget.

- [pen.rs:189](tamber/src/import/dtcg/pen.rs:189): **`NOT_BINDINGS` also suppresses legitimate opaque-map bindings.** With `n=8`, a frame’s `gap:"$n"` and a script’s `inputs:{"id":"$n"}` incorrectly produce `8px`. Script input names are arbitrary, including `id`; apply the exclusion only to actual schema properties. [Pencil schema](https://docs.pencil.dev/for-developers/the-pen-format#typescript-schema)

**Should-fix:** None.

**Nits:** None.

The other listed fixes check out by inspection; no additional count defect found. The permitted test command failed before execution because the read-only sandbox denied Cargo’s temporary-file creation. No files changed.
