# Code review: the dev loop's -dev bake driver rebuild, round recheck1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `1c92a20820`.
- **Method:** one brief (sha256 `5dddf096a3b006956b8a726f2c0c457a8fb0fedf0f1601882a5c8196f29c0f81`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/web/dev-bin-run.test.mjs:42` (release at `:67`). Two runs that both see a dead owner both `rmSync` the lock path and then `mkdirSync` it again. Removal is on the path, and nothing makes the two removals exclusive, so the later `rmSync` deletes the directory the earlier run has already created, filled with its pid, and entered. Both then build in the same `target/` and both clean `deck/`. The first run's `finally` deletes whatever directory is at that path, including the second run's lock. Fix it the way `lock()` in `host/apple/touches.mjs` does: remember the pid bytes just read, create `lock + '.takeover'` with `flag: 'wx'` so only one waiter steals, and `rmSync` the lock directory only if a fresh read is still those same bytes. The other waiter then sees the new pid and waits. In `finally`, remove the lock only while its pid file still contains `process.pid`.

2. `host/web/dev-bin-run.test.mjs:39` and `:41`. The pid is written after `mkdirSync`, so the directory is visible with no pid file. The waiter reads it with `flag: 'a+'`, which creates a missing file and returns `''`. `Number('' || 0)` is `0`, and `!owner` on line 42 removes that directory. A run that loses `mkdirSync` therefore removes a live owner's lock and enters too. `ENOENT` (the holder already removed the directory, or the owner's `writeFileSync` runs after that removal) is outside the `EEXIST` check, so it fails the test. `EPERM` from `kill(pid, 0)` is already counted as alive on line 37, same as `claimLive` and `build-lock.mjs`. Read the pid file with the default flag and retry on `ENOENT`. Treat a missing pid as stale only when the lock directory is older than about a second (a kill between `mkdirSync` and the write, or a lock left by the previous version of this test). A younger directory means the owner is still publishing: sleep and retry. Keep the `EPERM` branch. Give the pid write its own `ENOENT` retry so it does not share the `mkdirSync` catch.

**Non-blocking**

1. `host/web/dev-bin-run.test.mjs:37`. `alive` is only `kill(pid, 0)`. A recycled pid of an unrelated process counts as the owner, and the waiter sits until the 25-minute deadline. `claimLive` (`scripts/app.mjs:1212`) stores a start time and rejects a pid whose `ps` `lstart` is later than the claim. This pid file has no start time, so that check cannot be made.

`bakeWatchEnv` and `devBinRun` compute the same values as the functions they replaced (same `EXACT_ASSET_ROOTS` / `EXACT_BACKEND_SELECTION` expressions, same `args`, `cwd`, and `env`), and `host/web/dev.mjs:535` is the same `spawn`.
