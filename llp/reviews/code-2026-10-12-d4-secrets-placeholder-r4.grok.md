# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 4 (3c3792ddd), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `205882b9c04e3ebba03512db9508b796beca1ad84b05b2adc487720417750fc5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra's MAJOR is taken: the pass-over is its own filter over every reuse, a failed resource's included; test.

---

No findings.

The fourth commit closes the remaining way a passed-over compiled value could be shown before readiness. `READY`

`secrets_unseen` is the gate: the source is not ready, the resource is a store reader, an `else` is declared and available, and the store was opened holding a granted secret. A compiled value can be chosen in four places in `runner/src/runner/settlement.rs`. Each one now refuses that case.

- The reuse filter at `settlement.rs:473` rejects a compiled value when `secrets_unseen` is true, including when the arguments and the store revision are unchanged. That is the round-3 hole (`consume(use ? account : "")` with `account` compiled to `""`). `kept_seed` and `failed` still skip that clause; neither can be holding a compiled value here. A kept seed is built with `Held::new` (`runner.rs:969`), and a failure is recorded only on the query arm, which this resource does not reach before readiness once an `else` exists.
- The compiled-adoption arm at `settlement.rs:512` already requires `!secrets_unseen`.
- The equal-object arm at `settlement.rs:745` will not keep a compiled object for a placeholder. After reuse declines, the placeholder arm sets that flag before the comparison, so an equal `else` (`""`) does not restore the compiled `Held`.
- The `Answer::Later` fallback that rebuilds a compiled value (`settlement.rs:705`) is not entered. Before readiness, a non-runner source with a decodable `else` is taken by the placeholder arm once adoption and reuse have declined. An `else source()` row is waited on (`settlement.rs:392`) and is in `resources` before `secrets_unseen` is asked.

Store-readerness from arguments is known before that choice: `store_readers[i] |= store_dependent` runs at `settlement.rs:403`, and `load_resource` copies the callee’s flag (`vm.rs:778`). A resource that has already settled is not left on a compiled value while its flag flips later in the same pass. `opened_with_secrets` is still set only in `Store::from_names` (`store.rs:194`), which is the only constructor, so boot, reload, module mirrors, checkpoints, and restores all use the launch snapshot.

The fourth commit changes only that reuse conjunct and the test. The test runs both argument cases and asserts `placeholder && !is_compiled()`, which fails if either the reuse filter or the equal-object guard is missing. The LLP paragraph matches the code: a declared `else` and a launch snapshot with a granted secret, otherwise the build-time answer, journal line included, web JS still queued.
