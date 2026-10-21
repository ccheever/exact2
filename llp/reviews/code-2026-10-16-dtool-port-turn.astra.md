# Code review: tests take a server's port in turn, round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `389c6c799`.
- **Method:** one brief (sha256 `38a257a4ef0f99f0a45a2a0256ddce86859ab8b30571d9adec064065d0be445b`), the one reviewer pass the lead asked for. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 5 medium, 1 low).
- **Disposition:** taken, but 2 and 5 in part.
  - The turn is no longer a file. It is a second loopback port (the port + 10000) held by listening on it. The kernel lets one process listen and frees the port when that process ends, so there is no stale recovery, no PID reuse and no half-written file (1, 7). The holder answers a connection with its label, pid and time, which the next run says while it waits.
  - A run started by the holder has the turn already: `EXACT_PORT_TURNS`, inherited by its children (3).
  - `startServer` gives the turn back if its setup throws, and `stop` gives it back in a `finally` (4).
  - verify's preflight refuses an answering server only when no run holds that port's turn (6).
  - Giving the turn back is one-shot (8).
  - The design-tool tests' `beforeEach` has a 25-minute budget (2).
  - In part: a hook that times out anyway does not cancel its wait (2). On `process.exit` the turn ends with the process, while the server child is killed by the exit handler, so there is a short window (5).

---

1. **High — Stale recovery can delete a fresh lock.** [scripts/port-lock.mjs:33](/tmp/rv-dt-lock/scripts/port-lock.mjs:33)  
   A and B both read dead holder D. A removes D and acquires the lock; B then removes A’s fresh file and acquires it too. Both return successfully. An incomplete file older than five seconds can also belong to a suspended writer. **Fix:** use a kernel-managed lock on a stable file, or serialize the entire recovery operation. Another read before unlinking does not make it atomic.

2. **High — Ordinary contention times out `beforeEach`.** [scripts/design-tool.test.mjs:22](/tmp/rv-dt-lock/scripts/design-tool.test.mjs:22)  
   The hook retains Bun’s default five-second timeout; the tests’ `900_000` limits do not extend it. A holder taking six seconds therefore fails setup instead of queuing. The uncancelled acquisition can subsequently acquire and assign `givePort` after that test’s cleanup. **Fix:** explicitly budget the hook beyond `waitMs`, make acquisition cancellable, and keep cleanup tied to that acquisition. [Bun timeout default](https://bun.sh/guides/test/timeout), [pinned hook timeout handling](https://raw.githubusercontent.com/oven-sh/bun/bun-v1.4.2/src/runtime/test_runner/ScopeFunctions.rs).

3. **Medium — Additional nesting waits on its own ancestor.** [scripts/verify.mjs:139](/tmp/rv-dt-lock/scripts/verify.mjs:139), [owner check:237](/tmp/rv-dt-lock/scripts/verify.mjs:237)  
   Parent P holds the lock and launches legacy `test.mjs` C. C recognizes P and bypasses startup, then launches `exact.mjs test` G. G compares the owner against C, misses P, and waits up to 20 minutes while P waits for its descendants. Previously this failed promptly. **Fix:** propagate and validate an app/port-specific ownership token through nested runs. The ordinary direct-parent `api-test → test` case works.

4. **Medium — Failed setup leaks the lock until process exit.** [scripts/verify.mjs:139](/tmp/rv-dt-lock/scripts/verify.mjs:139)  
   After acquisition, `mkdirSync`, database cleanup, owner-file writing, or `Bun.spawn` can throw before a stop handle exists. `withAppServer` catches the error, but a surviving process retains the lock; a retry waits on itself. A throwing `clear()` during `stop()` also skips `give()`. **Fix:** protect setup with exception cleanup and release in teardown’s `finally` once the child is stopped.

5. **Medium — Exit releases before server shutdown.** [scripts/port-lock.mjs:40](/tmp/rv-dt-lock/scripts/port-lock.mjs:40), [scripts/verify.mjs:152](/tmp/rv-dt-lock/scripts/verify.mjs:152)  
   The lock’s exit listener is registered before the listener that kills the server. On `process.exit()`, another run can acquire while the old server still answers, producing a false occupied-port failure. **Fix:** tie ownership to the server’s lifetime and await shutdown before release; avoid the earlier independent release listener.

6. **Medium — `verify` usually bypasses the new queuing behavior.** [scripts/verify.mjs:779](/tmp/rv-dt-lock/scripts/verify.mjs:779)  
   When another cooperating run’s server already answers, this preflight refuses immediately, before reaching locked startup. Thus overlapping `verify` runs still fail in the main contention scenario. **Fix:** make preflight lock-aware or perform the occupied-port check after acquiring the lock.

7. **Medium — PID reuse prevents stale recovery.** [scripts/port-lock.mjs:11](/tmp/rv-dt-lock/scripts/port-lock.mjs:11)  
   After an unclean exit, an unrelated process can inherit the recorded PID. `kill(pid, 0)` then keeps the abandoned lock “alive” for the full wait despite a free port; `at` is only diagnostic. **Fix:** validate process birth identity, including reboot identity, or use an OS lock automatically released on death.

8. **Low — An old release handle can remove a later acquisition.** [scripts/port-lock.mjs:39](/tmp/rv-dt-lock/scripts/port-lock.mjs:39)  
   In one process: acquire A, release A, acquire B, call A’s release again. The PID matches, so B’s lock disappears. **Fix:** make release one-shot and validate a unique acquisition token.

`EPERM` is correctly treated as a live holder. Ordinary assertion failures inside the tests’ `try` blocks run their cleanup before `afterEach`; timeout handling needs the fixes above. Calls reaching `startServer` now wait up to 20 minutes for cooperating holders; `agent` uses the separate, unchanged `driveServer`.

Static review only; no files changed or tests run.

**Verdict: Request changes—mutual exclusion and cleanup are not reliable yet.**