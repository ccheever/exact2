# Code review: a shared helper ending a test's children and what they started (scripts/test-children.mjs), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d7f057243`.
- **Method:** one brief (sha256 `f7539803baedc84eddf266a53b74e571aa52ce9645bb242b0d10ac899b9eab8a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker (discovery from a reused root pid) and six should-fix (exit fields are no identity; a blocking stop(); no discovery during the grace; a failed ps taken for success; the grandchild's readiness guessed; a failing nested test leaves its fixtures).
- **Disposition:** all taken in e2dc1362cd; left: lstart is whole seconds.

One blocker remains.

1. **BLOCKER — discovery can adopt an unrelated process tree.** [scripts/test-children.mjs:50](/tmp/rv-kids-2/scripts/test-children.mjs:50)  
   After the original child exits, its watcher—and the final `note()` in `stopChild()`—still traverse its numeric PID. If that PID is reused, the replacement’s descendants enter `known` with their current start times and subsequently pass `remaining()`’s checks. Cleanup can kill unrelated processes. **Fix:** validate the original root’s identity before discovery and signalling; stop discovery from that root after exit while retaining previously recorded descendants. Add a regression covering root PID replacement.

2. **SHOULD-FIX — null exit fields do not prove the child is unreaped.** [scripts/test-children.mjs:63](/tmp/rv-kids-2/scripts/test-children.mjs:63)  
   Bun updates these fields through a deferred `nextTick` callback; they are not native process-identity guarantees. Consequently, the direct-child exception can admit a replacement PID while exit notification remains pending. **Fix:** validate the root’s recorded birth identity too, preferably using a native process handle. [Bun 1.4.2 implementation](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/js/node/child_process.ts#L1334)

3. **SHOULD-FIX — ordinary `stop()` blocks the runner throughout its grace period.** [scripts/test-children.mjs:81](/tmp/rv-kids-2/scripts/test-children.mjs:81)  
   Both adopters call `await dev.stop(30_000)`, but the wait happens synchronously before a promise returns. Pipe consumers, timers, and other children’s discovery intervals cannot run. A child waiting for its output to drain can therefore require an unnecessary SIGKILL. **Fix:** keep normal stopping asynchronous and retain ownership until completion; use synchronous cleanup as the `afterEach` backstop. Cover a timeout occurring *inside* normal stopping.

4. **SHOULD-FIX — discovery stops during the shutdown grace period.** [scripts/test-children.mjs:73](/tmp/rv-kids-2/scripts/test-children.mjs:73)  
   The watcher is cleared, and subsequent iterations only check already-known processes. A SIGTERM handler can start a helper that remains attached throughout the grace period, yet that helper never receives either signal. This blind interval can last 30 seconds. **Fix:** continue discovery from identity-validated surviving owned processes during shutdown.

5. **SHOULD-FIX — failed `ps` calls silently turn cleanup into success.** [scripts/test-children.mjs:23](/tmp/rv-kids-2/scripts/test-children.mjs:23)  
   A missing, failed, or timed-out `ps` can produce an empty table. `note()` then forgets descendants, `remaining()` can omit even the live direct child, and ownership is deleted without signalling anything. **Fix:** distinguish enumeration failure from an empty successful snapshot, preserve recorded identities, safely stop the direct child, and report incomplete descendant cleanup.

6. **SHOULD-FIX — the stubborn grandchild’s readiness is still guessed.** [scripts/test-children.test.mjs:19](/tmp/rv-kids-2/scripts/test-children.test.mjs:19)  
   The parent prints the PID before the grandchild installs its SIGTERM handler. `alive()` plus the 400 ms sleeps does not establish readiness under load; the test can pass with the grandchild simply accepting SIGTERM. **Fix:** have the grandchild acknowledge handler installation. If testing periodic discovery, wait for `child.known` to contain it.

7. **SHOULD-FIX — a failing nested regression leaves its fixtures running.** [scripts/test-children.test.mjs:58](/tmp/rv-kids-2/scripts/test-children.test.mjs:58)  
   When cleanup regresses, the assertions fail and `finally` only removes the directory. The outer registry does not own the nested runner’s children, whose intervals run indefinitely. **Fix:** give fixtures bounded self-termination and independent cleanup of recorded PID/identity pairs in `finally`.

The per-file hooks, async-lane inclusion, and dev-builds adoption look correct. The `lstart` parser treats localized dates as opaque text; changed start strings reject ordinary descendant PID reuse. However, macOS `lstart` discards subsecond precision, so it is not a unique birth identifier. [Apple’s implementation](https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/print.c#L649)

Read-only review; no files changed, tests run, or builds run.
