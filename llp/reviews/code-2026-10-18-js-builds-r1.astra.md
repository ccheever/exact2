# Code review: the JS dev loop's builds set aside and stopped, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `6bb58ffb3`.
- **Method:** one brief (sha256 `8abfd4becd3b99171e29aca3a7157e624b48faffba98d0586bc5f38cd67910d9`), shared by the reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (3 blocking: stale design reads, the first build, a leader's exit; a double build; the tests).
- **Disposition:** fixed in `220157296` (see its message).

---

1. **Blocking — fast edits can stall and publish an obsolete design.** [host/web-js/dev.mjs:393](/private/tmp/rv-jv-1/host/web-js/dev.mjs:393)  
   While a full build runs, value edit A starts a design read. Edit B arrives before that read finishes; its debounce sees `designing` and sets `again`. A then updates `design`, `served`, and `snapshot`/`aside` without checking its input revision, and line 411 returns because the full build remains active. B waits until that build finishes, potentially through another full build. I reproduced this with the actual scheduler functions. **Fix:** version design reads, discard stale results, and drain pending fast edits when the reader finishes independently of full-build completion.

2. **Blocking — the first build bypasses supersession entirely.** [host/web-js/dev.mjs:128](/private/tmp/rv-jv-1/host/web-js/dev.mjs:128)  
   The initial `build(app, dist)` uses the default always-false supersession predicate. Watchers are installed only afterward, at line 565. An edit after the compiler reads a source but before startup completes therefore publishes stale output and queues no replacement build. A stale initial failure similarly remains until another edit. **Fix:** establish input tracking before the first build and run startup through the same validation/retry path.

3. **Blocking — leader exit does not establish that the build group has exited.** [host/web-js/dev.mjs:83](/private/tmp/rv-jv-1/host/web-js/dev.mjs:83)  
   After signalling the group, the leader’s `exit` event clears `building`, removes the stage, and permits a replacement build. Cargo or another descendant can still be terminating and holding its lock. A read-only process probe reproduced leader exit after 1 ms while its descendant remained alive for another 253 ms. **Fix:** wait for descendant completion before releasing the build slot or reusing the stage; use pipe closure plus explicit group cleanup with bounded escalation, rather than leader `exit` alone.

4. **Should-fix — a superseded build finishing during a design read causes an extra build and reload.** [host/web-js/dev.mjs:444](/private/tmp/rv-jv-1/host/web-js/dev.mjs:444)  
   The superseded build restores its edits, then calls `rebuild()`. Because `designing` is true, that call sets `again`. The outstanding reader subsequently starts a replacement build without consuming those restored edits. On completion, `again` starts another build of the same inputs. I reproduced three builds and two reloads for one full edit followed by one value edit. **Fix:** reconcile pending edits and classification after the design read, with one scheduler responsible for starting the replacement build.

5. **Should-fix — the tests do not prove cancellation or process-group isolation.** [host/web-js/dev-builds.test.mjs:101](/private/tmp/rv-jv-1/host/web-js/dev-builds.test.mjs:101)  
   The hold seam delays completion **after process exit**, and `(aborted)` is logged before signalling. Making `stopBuild()` a no-op could still satisfy the log and unchanged-dist assertions through supersession alone. The fixed 30 ms sleep also does not establish that a build has started. **Fix:** synchronize with a live child barrier, observe descendant termination and replacement ordering, and test fast edits during a full build—including overlapping design reads.

The integration tests were not run because they write files. Validation used in-memory scheduler executions and a process-group probe; the working tree is unchanged.
