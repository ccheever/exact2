# Code review: a shared helper ending a test's children and what they started (scripts/test-children.mjs), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `96acb9c9a`.
- **Method:** one brief (sha256 `055848dd1c9537329bbd5c7e1c814f0b99fbda32d00a46df38cd74fd5d14c7a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two blockers (historical pids could kill unrelated processes; starting cleanup disabled the backstop, so a SIGTERM-deaf process outlived a timeout) and five should-fix (orphaned between looks; an import-time hook is not per file; Windows; not in the async lane; sleeps in the tests).
- **Disposition:** taken in ce7c5fba49 (start-time identity; a synchronous bounded stop, since bun:test fires no exit handler and does not wait out an async hook; each file's own afterEach; POSIX-only tests; the async lane; polling) except orphaned-between-looks, left best effort and said.

I found **two blockers** in `96acb9c9aa`. Static review only; no files changed and no tests or builds run.

1. **BLOCKER — Historical PIDs can kill unrelated processes.** [scripts/test-children.mjs:58](/tmp/rv-kids-1/scripts/test-children.mjs:58)  
   `known` permanently retains bare PIDs, including short-lived compiler processes. If one is reused during a long test, `alive(pid)` succeeds and cleanup signals the replacement. The exit fallback has the same problem. Natural child exits also leave `running` and the polling interval intact until teardown. **Fix:** record and validate process identity, including start time; retire exited identities and stop polling an exited root. Validate retained process groups before signaling them. [PID-reuse behavior](https://nodejs.org/api/child_process.html#subprocesskillsignal).

2. **BLOCKER — Starting cleanup disables its exit fallback.** [scripts/test-children.mjs:53](/tmp/rv-kids-1/scripts/test-children.mjs:53)  
   `running.delete(child)` happens before the asynchronous grace period. If Bun exits after invoking a timed-out test’s hook, a process that ignores or delays SIGTERM receives no SIGKILL: the continuation is abandoned and the exit handler sees an empty set. A second `stop()` also returns immediately while cleanup remains unfinished. **Fix:** retain stopping children until cleanup completes, memoize their stop promise, and include them in synchronous exit cleanup. Add a timeout regression with SIGTERM-resistant processes.

3. **SHOULD-FIX — Polling misses descendants orphaned between observations.** [scripts/test-children.mjs:25](/tmp/rv-kids-1/scripts/test-children.mjs:25)  
   A child can detach a grandchild and exit before the first 250 ms scan. Neither subsequent scans nor the final snapshot can recover that ancestry. Tracking also stops before the grace period, missing detached processes created during shutdown. Both tests keep the parent alive until cleanup, so neither exercises this case. **Fix:** retain process-group ownership where possible or register detached descendants at spawn time; add an early-parent-exit regression. Polling alone provides best-effort discovery.

4. **SHOULD-FIX — Importing the helper does not register a hook for every file.** [scripts/test-children.mjs:76](/tmp/rv-kids-1/scripts/test-children.mjs:76)  
   In a nonisolated multi-file run, the module executes once. Its import-time registration therefore cannot establish a separate file-scoped hook for every subsequent importer. The async lane’s isolation masks this. **Fix:** explicitly register teardown in each importing test file, or expose a registration function called by each file; cover a two-file, nonisolated run. [Bun hook scoping](https://bun.sh/docs/test/lifecycle).

5. **SHOULD-FIX — Windows silently loses descendant cleanup.** [scripts/test-children.mjs:34](/tmp/rv-kids-1/scripts/test-children.mjs:34)  
   Windows returns no descendants and signals only the direct child, providing no explicit tree-cleanup guarantee. The tests nevertheless require detached-grandchild cleanup and invoke Unix `sleep`. **Fix:** either implement Windows process-tree ownership or explicitly restrict this guarantee and its tests to POSIX.

6. **SHOULD-FIX — The new regression tests are absent from the async lane.** [scripts/test-children.test.mjs:17](/tmp/rv-kids-1/scripts/test-children.test.mjs:17)  
   [scripts/async.mjs:111](/tmp/rv-kids-1/scripts/async.mjs:111) explicitly lists script test files; `test-children` is missing. The automatic glob covers only the web host directories. **Fix:** add this file to the existing glue step.

7. **SHOULD-FIX — Fixed sleeps make cleanup assertions load-sensitive.** [scripts/test-children.test.mjs:22](/tmp/rv-kids-1/scripts/test-children.test.mjs:22), [scripts/test-children.test.mjs:44](/tmp/rv-kids-1/scripts/test-children.test.mjs:44)  
   Signal delivery does not establish that processes have disappeared within 200–300 ms. Also, `run.status !== 0` accepts a runner crash or external timeout as the intended test timeout. **Fix:** await direct-child exit and use bounded condition polling for descendants; assert the nested runner’s expected timeout diagnostic and absence of a spawn/runner timeout error.

The current `devLoop()` callers are per-test. The shared `beforeAll` Tamber server uses its own spawn path, so this change does **not** kill that fixture early.

The polling interval is correctly unref’ed and cleared by `stop()`, but costs four synchronous whole-machine `ps` calls per second per registered child. `spawnSync` is compatible with synchronous exit cleanup; its unbounded duration is the remaining concern there. [Exit-hook contract](https://nodejs.org/api/process.html#event-exit).

Immediate descendant SIGTERM does race graceful shutdown: the compiler-exit path at [host/web/dev.mjs:579](/tmp/rv-kids-1/host/web/dev.mjs:579) can bypass normal cleanup. I found no separate blocking lock leak from that ordering alone; owner-first termination would better preserve ordinary explicit `stop()` behavior.

After correcting the helper, adopt it in [scripts/design-tool.test.mjs:1195](/tmp/rv-kids-1/scripts/design-tool.test.mjs:1195) and [host/web-js/dev-builds.test.mjs:25](/tmp/rv-kids-1/host/web-js/dev-builds.test.mjs:25): both retain the original timeout-leak pattern. Avoid mechanically converting browsers shared through `beforeAll`; those need suite-scoped ownership.
