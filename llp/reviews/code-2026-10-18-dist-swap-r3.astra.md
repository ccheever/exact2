# Code review: builds put in place in one step (the dist swap), quick re-check of round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort high, sandbox read-only, a throwaway worktree.
- **Scope:** round 2's commit.
- **Method:** one brief (sha256 `e91bd9d13009d3806607ae41cb8c16b11c9078abb4f917276073a0387fed802d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nothing blocking; the fallback race test's residual flake, a leaked probe directory).
- **Disposition:** the probe directory removed in the next commit; the fallback race kept as theme2's condition asks (up to eight runs of 400 swaps).

---

Nothing is blocking. The claimed production fixes hold: refusals stop when realpath and bigint dev/ino are unchanged; the moved-tree realpath race still retries; `staticRead` propagates null; the probe performs an exchange; absent-dist installation precedes `.previous` deletion; and the log is reworded. No production regression found.

1. **Should-fix** — [replace-build.test.mjs:52](/tmp/rv-dx-3/host/web/replace-build.test.mjs:52): Eight attempts still leave the fallback test scheduler-dependent. All reads can miss the rename window, causing line 57 to reject correct behavior. This existing flake is reduced, not fixed. Synchronize a read with the rename gap, or stop requiring stress runs to observe a failure.

2. **Nit** — [replace-build.test.mjs:40](/tmp/rv-dx-3/host/web/replace-build.test.mjs:40): Every test-file load leaks an `exact-exchange-probe-*` directory. `directoryExchange` removes its children only. Retain the temporary path and remove it in `finally`.

Ten in-memory refusal/swap checks plus null propagation passed. The filesystem stress tests were not run; no files were changed.
