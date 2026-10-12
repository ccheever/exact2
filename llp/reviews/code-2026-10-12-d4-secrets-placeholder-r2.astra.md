# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 2 (6f0ed913d), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4efc45db7addda28dab63dd559ae435d945070025c0d1fc3c3bb12778d8c443d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MINOR is taken: a placeholder never keeps an equal compiled object; test.

---

- **MINOR — [runner/src/runner/settlement.rs:102](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:102): an equal-valued placeholder can still retain compiled provenance.** `store_readers[i]` can become true at line 403 when an action changes an argument from a constant to a store-reading resource—for example, `consume(useAccount ? account : "")`. With a granted secret at launch and changed arguments before readiness, this switches the compiled answer to the declared placeholder. If both values are `""`, lines 735–740 retain the old compiled `Held`, and `data_ready` emits the incorrect build-time revalidation line. Preserve placeholder provenance during equal-value reuse and add this transition to the regression; the new fixture always starts with `reader = true`.

The snapshot interpretation is sound. Boot, reload carry, module mirrors, defaults/clones, and restores/checkpoints handle it consistently. An unsettled `else source()` cannot cause a flip: settlement waits for that row at line 392. The LLP amendment matches the snapshot policy, and I found no unrelated changes.

76 targeted tests passed using existing binaries. The remaining case above is source-traced; no files were changed or binaries rebuilt.

NOT READY
