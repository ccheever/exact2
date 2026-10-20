# Code review: Tamber imports' lifecycle leftovers, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `4fe3a693be`.
- **Method:** one brief (sha256 `62b056046f0b18c7b5b209e2f6aac936af723aeb1b82df3dc7955f0022cfa67f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review HEAD `4fe3a693be` read-only: the isolation cancel paths, the design-file read loop, and `kept()`, then the tests that claim to cover them.The commit is the isolation leftover close-out. Next I’ll read the isolate, server, and `kept()` diffs in full, plus the tests around them.The diffs are in. I’ll read the queue, cancel, design-read, and `kept()` paths in context so a double-release or a wrong kill shows up from the code.The queue accounting and the per-import kill are sound. One shutdown signal is not.

## Blocking

None.

## Should-fix

**`stopImports` signals `running` after the planner has already been reaped.** `scripts/tamber-isolate.mjs:54`

`stop()` (`:153`) kills only when `exit` has not been seen, and it closes over that import's own child. `stopImports` then calls `killGroup(running)` anyway. `running` is cleared on `close`, which is after `waitpid`, so between `exit` and `close` the pid can be reused. A server close in that window sends `SIGKILL` to `-pid` with no exit check. The per-import cancel does not do this, and the chain does not point `running` at the next import: the next `planOnce` starts only after this child's `close` handler clears it. Drop the trailing `killGroup`, or skip it once `exit` has been observed.

## Nits

- **`scripts/tamber-isolate.test.mjs:132`** — the shutdown case checks the running import's status (`503`) and not the message. A late watchdog answer would still pass. The queued import's message is checked.
- **`scripts/tamber-isolate.mjs:117`** — nothing forces a short `read`, a file that grows, or the 2s deadline. Reverting the loop would not fail these tests.
- **`contract/cli/src/tamber_export.rs:213`** — the test locks "newest `KEPT`" and a symlinked `.exact`. It does not lock the 64 MiB cap, or a symlink at the baselines directory when `.exact` is a real directory.

## Checked

Slot and per-client count are released once. `release()` (`scripts/tamber-isolate.mjs:73`) is idempotent, and every path hits it: cancel while queued (`:82`), `waitMs` (`:86`), or `finally` (`:99`). `waitMs` and abort cannot both pass the `expired` check; whichever runs first wins, and the promise ignores a second resolve. Cancel while the planner runs does not `release()` itself; `finally` does, after `close`. There is no path that decrements without the matching increment.

A request abort kills only that import's child. One client's `AbortController` and `pairs.onEnd(who.client)` (`scripts/tamber-server.mjs:615`) are not another's. `request.client` for a paired token is forced to `who.client` (`:534`). `unend()` runs on both the success and the `catch` path (`:621`). An `onEnd` that fires immediately because the client is already gone returns a no-op unsubscribe and does not stay in the set.

`res` `close` on a normal answer does not abort. `end()` sets `writableEnded` before `close`, and the listener checks that (`:615`). The abort listener is removed in `finally` before `json()`. The existing reimport tests still get `200`, and the new cancel test gets `503` / "the import was cancelled".

The read loop is right for a regular file: `bytesRead === 0` is EOF, a short read continues at `got`, and `got > max` is `413`. The buffer is `max + 1` and each `read` length is `max + 1 - got`, so a file that grows stops at the cap or at the deadline (`:115`). A `0`-byte read is EOF at that moment; the open inode from an atomic replace stays stable. The outer 2s race can still abandon `realpath` / `open` — that leftover is still listed in `QUEUE.md`, and this commit does not claim it.

`kept()` is sound aside from TOCTOU. `symlink_metadata` + `is_dir()` is false for a symlink, and both `.exact` and the baselines directory are checked (`contract/cli/src/tamber_export.rs:170`). Entries use `file_type()` (a symlink is not a file). The read keeps the newest `KEPT` (16) and stops once a file would pass 64 MiB, without keeping a partial file (`:182`).

`bun test scripts/tamber-isolate.test.mjs scripts/tamber-reimport.test.mjs`: 14 passed. `cargo test -p contract --bin contract kept_tests`: 1 passed. The new cancel, queue-slot, symlink, and newest-`KEPT` assertions would fail on the previous code. The byte cap and the short read would not.
