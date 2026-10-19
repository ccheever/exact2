# Code review: Tamber axes bound before the fold (the JS loop's when-token edit), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f8ad6c5658`.
- **Method:** one brief (sha256 `90b12ae349c6f68c655a290da8501ddf9fa7c2e1e0c2d38af8b7252f028f35c2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; P2 (another fatal statement error, e.g. a bad share, still drops collected refusals: pre-existing), two P3s
- **Disposition:** the P3s taken in f8e66c6187 (the late check truncates to MAX_REFUSALS; the test asserts the design's refusal first, through compile_path too); the P2 is the lowering's general fatal-error shape, older than this commit, not changed here

Nothing blocking found. Three non-blocking follow-ups:

1. **P2 — A later action error can still discard collected refusals.** After the new push in [stmts.rs:119](/tmp/rv-axes-2/contract/lower/src/stmts.rs:119), append `share(files=["#missing"])` to the duplicate-axis fixture’s action. `share_pictures(...)?` propagates through [lib.rs:584](/tmp/rv-axes-2/contract/lower/src/lib.rs:584), returning only `lower-share-picture` and losing both Tamber errors. This exposes the existing broader error-aggregation weakness; it requires another independently invalid statement. Preserve accumulated refusals when returning that fatal error.

2. **P3 — The final return omits `MAX_REFUSALS` truncation.** [lib.rs:676](/tmp/rv-axes-2/contract/lower/src/lib.rs:676) should mirror the first check’s truncation. `push` only deduplicates. This is currently defensive consistency: child-state expression lowering and `bake_texts` do not append errors, and the CLI separately caps diagnostics.

3. **P3 — The regression test checks presence, not priority.** [tamber.rs:810](/tmp/rv-axes-2/contract/cli/tests/it/tamber.rs:810) permits `tamber-mode` to precede `tamber-duplicate`. Assert the first diagnostic, ideally also through `compile_path`, since its first-error behavior motivated the fix.

The non-fatal push itself is safe: argument types and arity are already checked, command emission does not resolve axes or execute anything, and the first error check prevents plan finalization. Warnings are collected separately, so the final check introduces no warning-driven rejection. Apart from other fatal exits noted above, the design refusal stays first.

The tests cover the original refusal, duplicate-axis aggregation, recovery, and retained pin behavior. The JS wait removes the fixed-delay assumption.

Read-only review of both commits; no builds or tests run.
