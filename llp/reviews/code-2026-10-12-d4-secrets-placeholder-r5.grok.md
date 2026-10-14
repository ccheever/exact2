# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 5 (7522b2eae), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4e41ed17b2c3266fee6d9a3e0a24a28322014ce15d0d9ed549ad000c48b6882e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

No findings. **READY**

The fifth commit moves the pass-over out of the last arm of `kept_seed || failed || (…)` and makes it a filter on every reuse. A compiled value that `secrets_unseen` passes over is no longer kept just because `failed` (or `kept_seed`) is true.

```475:477:runner/src/runner/settlement.rs
                        .filter(|s| {
                            !(s.value.is_compiled() && self.secrets_unseen(i, &row, &resources))
                        })
```

That closes the remaining way a passed-over bake could still be shown before readiness. The other ways were already closed and this commit does not reopen them: the compiled-answer arm still requires `!secrets_unseen` (`settlement.rs:514`), and a fresh placeholder still refuses an equal compiled object (`settlement.rs:747`). With a declared `else` in hand, settlement takes that placeholder arm and never reaches the later-answer path, which is the only other place a compiled value is built (`settlement.rs:707`). `else source()` still waits until that row has settled (`settlement.rs:392`) before the owner chooses.

Kept-seed and failed reuse are otherwise the same. A kept seed is `Held::new` (`runner.rs:969`), so `is_compiled()` is false and the new filter does not touch it. A failed non-compiled value, and a failed compiled value when `secrets_unseen` is false, still reuse through `failed` ahead of the argument and placeholder checks (`settlement.rs:456`). `a_failed_compiled_answer_is_passed_over_too` (`kept_secrets_tests.rs:254`) is the case that used to miss: `give_up`'s unchanged arguments, then `use` makes `consume` a store reader at equal arguments. I read that test against the settlement path; I did not execute it.

Nothing else moved in that commit. The launch snapshot is still set once in `Store::from_names` (`store.rs:194`), which is the only constructor boot, reload carry, checkpoints, and module mirrors use; a checkpoint restore does not rebuild it. The LLP paragraph and the web-JS queue line match the code.
