# Code review: the dev loop's -dev bake driver rebuild, round recheck2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0e7226e893`.
- **Method:** one brief (sha256 `1a2cb717838661f66c9ac4fe4917133b085069b6743b6f1d3c5431e3707797bd`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `scripts/app.mjs:1188` (release at `scripts/app.mjs:1192`; taken at `host/web/dev-bin-run.test.mjs:38`, released at `host/web/dev-bin-run.test.mjs:63`). Takeover is check-then-unlink. Both waiters read the dead claim, `claimLive` runs `ps` (`scripts/app.mjs:1217`) while that file is unchanged, both second reads still match, and both `rmSync` the path. The later unlink removes the winner's new `wx` file, and that waiter enters too. `release` then `rmSync`s the path with no content or inode check, so whichever run finishes first deletes the other's claim and a third run can take `sparkline/target` and `deck/` while a test is still inside them. The same pattern is called out in `host/web/build-lock.mjs:3-5` and `:94-96`: a stale lock file cannot be removed atomically. Fix: hold the kernel `flock` for the whole run, as `takeLock` does. The waiter that gets `LOCK_EX` rewrites the pid in that inode; the other blocks until the fd closes (a killed owner's flock is already gone). On release, close the fd and unlink only when `stat(path).ino` is still the flocked inode. Wait out this test's deadline — `takeLock`'s 30s (`host/web/build-lock.mjs:29`) is shorter than a cold build.

**Non-blocking**

1. `scripts/app.mjs:1182` and `host/web/dev-bin-run.test.mjs:38`. The previous lock at this path is a directory. `wx` fails on it, `readFileSync` of the directory throws, and the test treats that as contention and retries until the 25-minute deadline. The directory is never removed. If the path is a directory, remove it and claim again.
2. `scripts/app.mjs:1182` and `scripts/app.mjs:1214`. `wx` creates the inode before the write. An empty or non-JSON claim is treated as live forever, so a kill in that window leaves a file later runs will not take over. Under the flock above, replace a claim that stays unreadable past a short age (the 1s age in `takeFileLock`).
