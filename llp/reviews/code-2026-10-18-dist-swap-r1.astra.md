# Code review: builds put in place in one step (the dist swap), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the dist-swap commit (07982a7d0).
- **Method:** one brief (sha256 `bbad1b5899eb8ffd4e26f5474a4fc9af08e5008cba70cc91bf98934928215a57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (case aliases refused, a transient miss not retried, the tests' assumptions).
- **Disposition:** fixed in round 1's commit (26d0dd87d; see its message).

---

Nothing blocking found.

1. **Should-fix — [host/web/serve.mjs:671](/private/tmp/rv-dx-1/host/web/serve.mjs:671): valid case aliases become permanent 404s.**  
   `root !== at` compares the canonical directory name with the supplied basename. On this case-insensitive Mac, `buildTreeFile('/private/tmp/RV-DX-1', '/README.md')` returns `null`, while the lowercase spelling succeeds. All four attempts reject the same valid directory. Compare filesystem identity rather than path spelling, preserving the intended handling of symlinks.

2. **Should-fix — [host/web/serve.mjs:669](/private/tmp/rv-dx-1/host/web/serve.mjs:669): a transient root failure bypasses every retry.**  
   A lookup can lose the retired directory while exchange and cleanup run; Darwin’s path reconstruction can report `ENOENT` during a move. The exchange path creates no `.previous`, so its failed lookup immediately returns `null`, even if the next attempt would succeed. Replace that inner `return null` with `continue`, and cover an initial lookup failure followed by success. An in-memory fault injection confirmed the early exit. ([XNU implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_cache.c))

3. **Should-fix — [host/web/replace-build.test.mjs:50](/private/tmp/rv-dx-1/host/web/replace-build.test.mjs:50): the stress tests depend on scheduling and exchange availability.**  
   Correct fallback behavior can produce zero misses when the reader never hits the rename window, failing this assertion. Conversely, the first test requires zero misses without establishing that exchange is available; a supported fallback-only filesystem can fail it. Use synchronization to exercise a deliberate interleaving, verify exchange availability before asserting its guarantees, and treat incidental stress misses as diagnostics rather than required outcomes.

The review remained read-only. I ran read-only probes and in-memory fault injection; I did not run the stress suite because it creates and deletes files.
