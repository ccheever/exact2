# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 4 (3c3792ddd), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `205882b9c04e3ebba03512db9508b796beca1ad84b05b2adc487720417750fc5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR is taken: the pass-over is its own filter over every reuse, a failed resource's included; test.

---

**MAJOR — [runner/src/runner/settlement.rs:473](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:473): failed-resource reuse still bypasses `secrets_unseen`.** The exclusion sits inside the last branch of `kept_seed || failed || (…)`, so `failed` can still retain a compiled answer.

This is reachable before readiness:

1. Send, then cancel, a deferred mutation that refreshes `consume`. Cancellation leaves reconciliation owed.
2. A subsequent viewport recommit fails a task-key check. [admission.rs:52](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/admission.rs:52) calls `give_up`, marking `consume` failed and clearing its forced refresh. A gate conditional on `failed(consume)` can let the retry succeed.
3. Set `use` in the equal-argument regression fixture. `consume` becomes a store reader, but its unchanged arguments retain `failed=true`. Reuse short-circuits the new exclusion, keeping the compiled answer with `pending=false`.

The exclusion needs to cover this reuse path too. The current regression exercises only nonfailed resources.

The opening-snapshot interpretation, store constructors, reload carry, mirrors, and checkpoint restores otherwise look consistent. Placeholder rows settle before their owners, preventing the suspected unsettled-`else source()` flip. The LLP amendment matches those paths.

All 61 focused tests passed using the current prebuilt runner binary. The failure sequence above is a code-path finding; I did not compile a new reproducer under the read-only constraint.

NOT READY
