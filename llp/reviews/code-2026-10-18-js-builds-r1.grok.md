# Code review: the JS dev loop's builds set aside and stopped, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `6bb58ffb3`.
- **Method:** one brief (sha256 `8abfd4becd3b99171e29aca3a7157e624b48faffba98d0586bc5f38cd67910d9`), shared by the reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (nothing blocking; a double build, the first build, an abort on an exited build; the tests).
- **Disposition:** fixed in `220157296` (see its message).

---

I'll review this commit read-only: the message first, then the build/rebuild/changed and stop-build paths against the owner's rules.The diff is in. Next I’ll read `build`, `rebuild`, `changed`, and the stop paths in full, plus the new tests.Nothing blocking. The stage swap, the one-abort guard, the process-group kill, and the superseded check line up with the ruling on the paths that actually publish.

1. **Should-fix** — `host/web-js/dev.mjs:411` (with `:435` and `:446`). A value edit whose design read is still in flight when the previous build's exit handler runs is scheduled twice, and the second run can reload the page again.

   The fast path clears `edited` before `designOf`. If that build finishes while `designing` is set, the superseded handler puts its own files back, calls `rebuild()`, and that call bails out with `again = true` because a design read is in progress. The design read then continues and starts a design-only build without clearing `edited` or `again`. That build's `began` already includes the edit, so it is not superseded. If it is kept, it does not publish; `if (again) rebuild()` then starts a second, full build and sends a second reload. If the logic revision moved, the design-only build publishes and reloads, and the follow-up build reloads again. Final bytes are the new files either way. The page reloads twice for one burst.

   Fix: before starting a build, if `edited` or `again` is set, put `pendingEdits` back and `return rebuild()` so one invocation owns the next build. The line 411 return should restore those edits the same way line 414 does.

2. **Should-fix** — `host/web-js/dev.mjs:128` and `:565`. The startup build never enters the new gate, so an edit made while it runs is published and then dropped.

   `build()` defaults `superseded` to `() => false`, and the first call uses that default. Watchers, `inputs`, and `changed()` are created only after that await returns (including `EXACT_DEV_BUILD_HOLD_MS`). A save during a cold first compile is not counted, `replaceBuild` still runs, and no later pass replays the edit. The server is not listening yet, so the first page after startup is the stale dist and stays that way until another save.

   Fix: arm the input count before the first `build()`, pass the same predicate, and on `{superseded}` delete the stage and build again before `plan ready` or `listen`.

3. **Should-fix** — `host/web-js/dev.mjs:83` and `:351`. An edit in the gap after the child has exited spends the one abort on a dead pid, so the follow-up build is not aborted.

   `building` is cleared only inside the hold timer. `exitCode` is already set. A watch callback in that gap (production hold is `setTimeout(0)`; the tests hold 2–4s) still sees `building`, and if the build was young it sets `stopped` and calls `stopBuild()`. The group kill throws `ESRCH`. `stopped` stays true until some later build is not superseded, so a real young follow-up is left to run out and be set aside instead of aborted. That is the "edit just as the build ends" case this change is aimed at.

   Fix: in `stopYoungBuild`, return when `building.exitCode !== null || building.signalCode !== null`, and do not set `stopped`. Leave `building` set until the handler finishes so the next spawn still waits for this exit.

4. **Nit** — `host/web-js/dev-builds.test.mjs:65` and `:112`. The tests would fail on the old loop. They do not prove the value batch or the "no second abort" rule.

   On the old code the first test's 10ms sampler would see `qz-second` in dist, and the second would time out on `(aborted)` and `edit → a build superseded`. What they miss: `edit → batch ready` is also logged by a kept build (`built in`, not `no build`), and the 60s wait covers the 4s hold, so a batch that arrives only when the later build finishes still passes. Nothing asserts that a superseded build did not reload, did not change `/__dev/design`, or did not move the page map. Nothing asserts that a third edit in the same burst does not abort. The 1200ms sleep assumes the second build has already entered its hold.

   Fix: count the `no build` batch log before the hold can finish, and assert one `(aborted)` across a burst of three edits.

Scrub, aside, released, and `served` are left alone on the superseded return, so a value batch applied while a build runs is not rolled back; a kept or full publish of that build cannot run afterward because `inputs` has already moved. A killed build only writes the stage directory; `replaceBuild` runs after the superseded check and a non-zero exit (a signal is `code === null`) deletes the stage instead. `stopped` is cleared only when a build is not superseded, so a burst aborts at most one build. The next spawn is inside the exit timer, after that child has exited, and the signal is `process.kill(-pid)` on the detached child.
