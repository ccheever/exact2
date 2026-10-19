# Code review: the JS dev loop's builds set aside and stopped, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `6bb58ffb3`, `220157296`.
- **Method:** one brief (sha256 `b2eb3d5db31cda4a2e15f0f37ec43d65942a2c6253038c044036006ca1947de9`), shared by the reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (5 blocking: generated writes at startup, the first build's inputs, a trailing event on a design read, deletions, the check before the work; the tests).
- **Disposition:** fixed in `701e2313f` (see its message).

---

1. **Blocking — Generated writes can prevent startup.** [dev.mjs:146](/private/tmp/rv-jv-2/host/web-js/dev.mjs:146)  
   The initial watcher invalidates a build for any unfiltered event, including its own generated inputs. Expose supplies a concrete trigger: with `EXPOSE_OPENROUTER_KEY` set, its build script rewrites `local.ts`; with no backend-selection file, the nested Cargo check keeps rerunning that script. Each attempt marks itself superseded, so startup keeps rebuilding before serving. **Fix:** make generated writes conditional on changed contents and distinguish generation already captured by the build from subsequent edits.

2. **Blocking — The first build still misses inputs.** [dev.mjs:146](/private/tmp/rv-jv-2/host/web-js/dev.mjs:146)  
   Change `.exact/backend-origin` after the initial build reads it: `skipped` drops its event, and `newer()` does not inspect it. Startup then records the changed bytes as `backendSeen`, although it serves the old backend configuration. Runtime files and external JavaScript packages likewise lack startup watchers and freshness checks. **Fix:** keep the complete input watchers active from startup, including the backend exception, and validate newly discovered dependencies before accepting the first build.

3. **Blocking — A delayed watcher notification still permits stale design reads.** [dev.mjs:422](/private/tmp/rv-jv-2/host/web-js/dev.mjs:422)  
   Start a design read for A, then write B before the read finishes but before its watcher callback runs. `inputs === readAt`, so A overwrites `design`, `snapshot`, and `served`. If B is a released scrub, `releasedMark()` also clears its preview and attaches the release mark to A. An in-memory execution reproduced this rollback. **Fix:** validate the captured design bytes and dependency versions before applying the read; consume a scrub release only when the result includes its committed source.

4. **Blocking — Deletions bypass the fallback freshness check.** [dev.mjs:99](/private/tmp/rv-jv-2/host/web-js/dev.mjs:99)  
   Delete an imported Contract file after compilation reads it, with its watcher notification delayed until after completion. `statSync()` throws, the catch returns `false`, and the successful stale build publishes its dist, design, maps, and reload. **Fix:** compare recorded input existence and contents against completion state. Distinguish a deleted input from a resolution candidate that was already absent.

5. **Blocking — The publication check happens before potentially lengthy work.** [dev.mjs:100](/private/tmp/rv-jv-2/host/web-js/dev.mjs:100)  
   After checking freshness, the callback synchronously hashes the staged files and calls `replaceBuild()`, which can wait on another publisher’s lock. An edit during either operation cannot advance the watcher counter while JavaScript is blocked, and no further check precedes publication. The stale result then updates every served state. **Fix:** prepare metadata first, acquire the publication lock, and revalidate input versions immediately before replacing dist or accepting a kept result.

6. **Should-fix — The tests do not establish the claimed cancellation guarantees.** [dev-builds.test.mjs:109](/private/tmp/rv-jv-2/host/web-js/dev-builds.test.mjs:109)  
   The third edit arrives 300 ms after cancellation, inside the aborted build’s four-second **post-exit** hold. The replacement build has not started, so “one abort” still passes if cancellation eligibility incorrectly resets for every build. The test also never asserts that final dist contains `qz-four`. Its batch subsection exercises only verifying builds, so the old verifying-only restriction would pass that subsection. **Fix:** use explicit build/read barriers, deliver another edit during the replacement build, assert the final value, and cover batches during a full build.

Validation used in-memory executions of the actual functions and a live process-group probe. I did not run the integration suite because it creates and changes files.
