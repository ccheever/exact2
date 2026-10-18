# Code review: the JS dev loop's builds set aside and stopped, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `6bb58ffb3`, `220157296`.
- **Method:** one brief (sha256 `b2eb3d5db31cda4a2e15f0f37ec43d65942a2c6253038c044036006ca1947de9`), shared by the reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (1 blocking: a future time never publishes; the backstop's gaps, a trailing event, the first build, the tests).
- **Disposition:** fixed in `701e2313f` (see its message).

---

I'll review both commits read-only: the original change and the follow-up that claims to fix round 1, then check each fix against the owner's rules.The follow-up commit is small. I'll read `build`, `rebuild`, and the watch path in the current file and compare them with what round 1 claimed to fix.The five rules hold on the ordinary path: dist is staged and swapped, a burst aborts one young build, the next build waits until that process group is gone, a value batch is applied immediately, and a watch event the counter has already seen is not published. One case never publishes.

1. **Blocking** — `host/web-js/dev.mjs:98` (restart at `145` and `481`)
A successful build is set aside when any Contract source, the design, `app.ts`, or `app.json` has `max(mtime, ctime) >= began`. `began` is wall-clock time at the start of `build()`, not a snapshot of the times the build actually read. A timestamp that stays ahead of the clock (archive, skew, `touch`) is therefore “newer” on every success. The first-build loop and the superseded restart both run again with no cap, so the server never listens, or it listens and never publishes. A stable future `app.json` is enough. Writing `app.contract.d.ts` is not: that event’s name is `app.contract.d.ts`, both watchers ignore it, and it does not bump `app.ts`’s times.
Fix: stat the paths at the start and supersede only if a time changed. A stable future timestamp is the content this build read; publish it.

2. **Should-fix** — `host/web-js/dev.mjs:98-100`
The mtime backstop does not cover the late watch event it was added for.
- A missing path makes `statSync` throw, and the `catch` returns false, so a delete or a replace that loses the inode is published.
- `newer()` runs only when `code === 0`. A torn or failed build still `push`es its error (`104`, then `496-499`) over the newer edit.
- `statSync` follows symlinks, so a link retargeted at an older inode does not look newer.
- `ctime` sets aside a metadata-only update (xattr, mode).
- Assets, `host/web-js`, `host/web`, and `typescript.mjs` are watched and counted, but they are not in this list. If that event arrives after the check, the old dist, design, snapshot, served digest, reload, and page map are published.
Fix: `lstat` at start and at end; treat ENOENT as a change; run the check on failure too and do not `push` the error when it hits; include every watched input, or rely on the counter plus a second check after the watch callbacks have been flushed.

3. **Should-fix** — `host/web-js/dev.mjs:416-427`
The design read is versioned only by `inputs`. Round 1’s late event is the case where the write is already on disk and the callback has not run. The check passes, and the older text is written to `design`, `served`, `snapshot`, and the batch (`428-441`). The verifying build usually reads the new bytes and corrects it, so the bad digest is visible until then. A scrub release in that window calls `releasedMark()` on the stale read.
Fix: before applying, read the file again and discard the read when the bytes differ, the same as `inputs !== readAt`.

4. **Should-fix** — `host/web-js/dev.mjs:145-147` and `87-90`
The first build’s watcher is only `app.dir`, and it is closed in `finally` before the real watchers are installed (`605`). An edit under `host/web-js`, `host/web`, or a Contract package outside the app is not seen (package paths are only in `newer()`, and only after a successful compile). The early callback also does not use `changed()`’s OUTPUT, temp, or `.exact` filters, and the retry loop has no cap if `moved` stays true.
`groupGone` escalates to SIGKILL only after the leader’s `exit`, and only for 5s from that moment. If the leader ignores SIGTERM, the deadline never starts, `building` stays set, and the next build never runs, so Cargo’s lock stays held. Any `process.kill` error, including `EPERM`, is treated as “group gone” (`88`), which starts the next build while a child can still hold the lock.
Fix: use the same watcher as `changed()` for the whole startup, and overlap it with the main watchers. On stop, arm the 5s SIGKILL immediately; proceed only on `ESRCH`.

5. **Should-fix** — `host/web-js/dev-builds.test.mjs:108-132`
The new assertions pass on `HEAD~1`, the code that had the bugs.
- The second value edit is written 500ms after the first “batch ready”, during the 4s hold. `HEAD~1` already ran that read immediately (`building && !fast`). The test never writes while `designOf` is in flight, and it never checks that the second “no build” batch is `#303030`.
- `expect(count(aborted)).toBe(1)` is held by `stopped`, which `HEAD~1` already set. The third write happens while `stopped` is true, so removing the new `exitCode !== null` guard does not log a second abort.
- Nothing fails if the first build is not retried, if the next build starts before the killed group exits, or if a stale design read is applied.
Fix: write the second value edit only after the design-read child has started and before it returns, and assert the served digest at that moment. Abort once against a build that has already exited (`exitCode !== null`, `stopped` still false) and assert no second `(aborted)` and no signal to that pid. Hold the first build, touch a source, and assert a second compile before the port listens.

6. **Nit** — `host/web-js/dev.mjs:443`
When a value batch lands during a build, `pendingEdits` is put back and `pendingAppeared` is not. The other set-aside paths (`422-424`, `446`, `484-485`) put both back. A disappeared temp that was only in `appeared` can make the next pass look like a non-design edit and force a full reload.
Fix: restore `pendingAppeared` here too.
