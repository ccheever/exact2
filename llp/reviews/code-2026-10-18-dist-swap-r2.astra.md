# Code review: builds put in place in one step (the dist swap), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the dist-swap commit and round 1 (26d0dd87d).
- **Method:** one brief (sha256 `ff4cbe435884e772dfd8d0218e31cf45f5ada1c4ea7ac1af7667a4b10d49fd5e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (a real exchange probe, the fallback race, nested retry budgets).
- **Disposition:** fixed in round 2's commit (99dde2cbf; see its message).

---

Nothing blocking found. The reported production fixes look correct for their stated cases; the remaining findings are:

1. **Should-fix — [host/web/serve.mjs:634](/tmp/rv-dx-2/host/web/serve.mjs:634): the exchange test’s availability check is incomplete.** `directoryExchange()` checks only whether an FFI function loaded. If the kernel or filesystem rejects exchanges, it still returns `true`, so the test runs its zero-miss assertion against the two-rename fallback. I reproduced this with an FFI stub returning failure. **Fix:** probe an actual exchange between disposable directories on the test’s volume, and skip based on that result.

2. **Should-fix — [host/web/replace-build.test.mjs:51](/tmp/rv-dx-2/host/web/replace-build.test.mjs:51): three retries do not remove the fallback test’s flakiness.** A valid schedule can run every reader lookup before or after both renames, producing zero misses in all three runs. The test then fails despite correct fallback behavior. **Fix:** assert successful publication and rollback deterministically. If demonstrating the rename gap matters, coordinate a reader between the renames instead of requiring stress to encounter it.

3. **Nit — [host/web/serve.mjs:401](/tmp/rv-dx-2/host/web/serve.mjs:401): permanent refusals consume nested retry budgets.** A stable symlink correctly remains a 404, but `staticFile()` repeats its lookup four times; `staticRead()` repeats that four more times. The in-memory check counted **16 leaf lookups, 16 root resolutions, and 32 root stats** for one refused symlink. **Fix:** distinguish definitive refusals from transient races and use one retry budget.

Read-only probes confirmed case-alias resolution, transient retries, old-stage rejection, lone-`.previous` resolution, stage-symlink rejection, and preservation of a symlink `dist` target during fallback. Node imports and disables exchange correctly. Linux dispatch was checked with stubs; filesystem-mutating stress tests were not run.
