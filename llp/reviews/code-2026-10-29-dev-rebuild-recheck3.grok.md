# Code review: the dev loop's -dev bake driver rebuild, round recheck3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9182a5cfa7`.
- **Method:** one brief (sha256 `cbe734e45bf5911cac42b1d6eac7541510ec33fe0aa2cf65c47ea98bb590ec44`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/web/dev-bin-run.test.mjs:40` — `takeLock` opens the lock with `openSync(lock, 'a+')` (`host/web/build-lock.mjs:44`) and does not create missing parents. The path is `apps/sparkline/target/.dev-bin-run-test.flock`, and that `target/` directory is not there until `rebuilt()` runs Cargo, which happens only after the lock is held. On a cold tree the open throws `ENOENT` at once. The `catch` treats every error as contention and retries with no delay until the 25-minute deadline, so Cargo never runs and the directory never appears. Two cold runs both spin and both fail; neither takes the flock. `claimBuildOutput` created the parent before creating its claim. Fix: `mkdirSync(resolve(app.dir, 'target'), { recursive: true })` once before the loop, then call `takeLock`. Both racers may create the directory; exclusion stays on the flock.

With the directory present, the two earlier holes are closed. `LOCK_EX` is per inode, so two waiters cannot both hold it. The inode is checked after the lock and again before unlink, so an unlink/recreate does not let a second process keep a lock on a name nobody else opens. A kill releases the flock (including `SIGKILL`). A claim that dies before the pid is written leaves an empty file; an empty owner is not treated as live, so the next taker writes its pid and enters. `release()` runs in `finally`, and the new `.flock` name is not the old `.lock`.

**Non-blocking**

1. `host/web/dev-bin-run.test.mjs:61` — `rmSync(deck)` runs before `release()`. If that removal throws, `release()` is skipped and the flock stays held until this Bun process exits. A second run then waits on a process that has already left the critical section. The kernel still drops the lock on exit. Fix: nest it as `try { if (made) rmSync(deck, { recursive: true, force: true }); } finally { release(); }`.

2. `host/web/build-lock.mjs:57` — A dead pid is taken over, so a kill does not stick. If that pid has been reused by an unrelated live process, this branch treats it as a file-lock holder, drops the flock just acquired, and waits. It never rewrites the file, so each later run burns the 25-minute deadline until that process exits or someone removes the file. Fix: store the owner's start time, as `claimLive` does, and treat a reused pid as stale.
