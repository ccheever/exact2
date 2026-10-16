# Code review: LLP 1121 stage 3, commit 5, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `d66f62b1feef0c8d662bdf5a9942b88acb9df12cc6714595164931d60f2edb9e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two high, six medium, one low; Grok: one high, two medium, two low.
- **Disposition:** fixed in round 1 (bytes read once, the index before HEAD, the baseline before the first write, reply-order reads, proposals listed and announced); .contract files are watched when a server writes them

---

## High

**HEAD moves before the index update, and a failed index update is reported as a failed commit** — `scripts/tamber-commit.mjs:113`

`update-ref` runs at line 113, and the real index is updated only afterward at line 115. `update-ref` does not take `index.lock`. Any `update-index` failure after that — another git process, an editor, or a stale lock — leaves the new commit on HEAD and returns `commit: fatal: Unable to create … index.lock`. The session is not cleared (`files.delete` is at line 117, after the loop), so a retry is refused with "HEAD has moved". The index still has the old blob. `git status` shows `MM` for that path, and `git diff --cached` is the reverse of the design edit, so the next ordinary `git commit` reverts the file the tool just committed. I reproduced this with `index.lock` held: HEAD advanced, the call returned the lock error, the retry was refused, and the index staged the new bytes back to the old ones.

Update those paths in the real index first, then `update-ref` the branch that was current at the guard (`symbolic-ref -q HEAD`, or `HEAD` itself when detached) with the old sha. If `update-ref` fails, put the saved index entries back. A crash then leaves the design change staged, with HEAD unmoved.

## Medium

**A held proposal is still applied after a successful drop, and a second accept replays it** — `scripts/tamber-server.mjs:368`

`accept` leaves the proposal in the map until `write` returns (line 385). `write` queues the transaction on the design server before that wait yields, so a `drop` during a slow write (the server is allowed 120s) deletes the map entry, emits `proposal-dropped`, and returns success while the write still lands. A second `accept` in that window queues the same ops again. If the first write changed the file, the second conflicts and `recompute` (line 376) stores a new proposal for those same ops; a partial `accept` in the window does that for a subset while the full write still applies. Digests do stop the second transaction from landing on top of the first.

Delete or mark the proposal before any `await`. Let `drop` and a second `accept` 404. Restore the entry only when the write fails for a reason other than conflict.

**A conflict names the previous client for a hand edit the watcher has not published** — `scripts/tamber-server.mjs:342`

`by` is `lastBy`, which changes only when an event is published (line 107). The watcher does not publish while `writing > 0` (line 207), and otherwise waits out `settle` (50ms). That is the window in which the design server's late check returns `stale`: the save landed after the snapshot and before the rename. `by` still says the client who wrote the file last, not "changed on disk". The same miss is permanent for a later `.contract` file: the watcher ignores any name that is not `.tamber` and not the design basename (line 196), so `seen` and `by` never observe it. A hand edit that is still on disk at `commit` is refused by the byte check; one that restores the server's bytes is not remembered, and a stale inverse keeps blaming the last client.

If `error.files[path]` differs from `known`, the change was not a published client write: use "changed on disk". When they match, `lastBy` is the right client. Watch `.contract` the same way as `.tamber` before those writes exist.

## Low

**The commit hashes the file twice** — `scripts/tamber-commit.mjs:86` and `:102`

The guard hashes a `readFileSync`, then `hash-object -w` reads the path again. A save in that gap is what gets committed, past the "differs from the server's last write" check. Hash the bytes once and pass them to `hash-object --stdin`.

**Held proposals are unbounded, and expiry is silent** — `scripts/tamber-server.mjs:351`

`expire` runs only at the start of `hold` and `accept`. A proposal past 15 minutes cannot be accepted, but it stays in memory and on the event stream until then, and nothing emits `proposal-dropped`. `drop` also takes no `client`, so that event names no one. Cap the map, expire on a timer, and send the drop.

## What holds

No command injection. Git is `spawnSync` with an argument list. The message and the client name are the `commit-tree` body on stdin, not a shell command.

The old-value guard on `update-ref` is real, and the temporary index is `GIT_INDEX_FILE` plus `read-tree HEAD`, so other staged paths stay out of the commit. The real index is then touched only for the chosen paths. Paths with spaces commit cleanly. A `../` path fails `git show` and is refused before `update-ref`. Mode comes from `ls-tree` (a `100755` file stayed `100755`). A symlinked app directory is `realpath`'d and commits the file inside the repo. A linked worktree commits on that worktree's branch and does not move the main worktree. Detached HEAD moves. An unborn HEAD is refused with "no commit to start from", and nothing is written. A missing file is refused and not recorded as a deletion; there is no remove in the index, which matches a writer that only renames new text over existing files. A submodule app is committed inside the submodule; the superproject gitlink is left dirty (`M sub`) and is not pushed.

The baseline is the pre-write digest (`before`), not the bytes after the write. A worktree that differed from HEAD, and an index that differed while the worktree matched, are both refused, and HEAD does not move. A later write that read other bytes sets `foreign` permanently, including when a still later write overwrote them. If `seen` observed an intermediate digest, restoring the server's bytes does not clear it. The one hole is a restore that never appears as its own digest: the watcher coalesces inside `settle`, and `.contract` is not watched. The tree that would be committed is still the server's bytes.

`client` on every write, instead of on `hello`, is the right shape. One process serves every HTTP client, so a hello cannot name the writer of a later POST. The string is self-asserted. The only credential is the shared development token, so that is the whole trust boundary, not a second bug. Accept and drop are the same: any token holder can act, and the proposal id is not a capability. Without the token the POST is a 404. Ids are `prop-<epoch>-<n>` with a random epoch.

Accept does not replay a stale proposal onto new bytes. A conflict or a partial `items` list deletes it and returns a new proposal with a new diff; the test's file does not change until that new id is accepted. There is no server-side recompute loop. `items` is range-checked. One naming trap for the tool: on the `proposal` event `items` is a count, and on `accept` it is a list of indexes.

The new tests cover attribution of a published hand edit, commit of only the session file, other staging left in place, no push, a dirty baseline, a hand edit between two writes, and accept / stale recompute / partial accept. They do not cover staged-at-baseline (the guard does refuse it), HEAD moved, `index.lock`, expiry, drop-during-accept, or `by` before the watcher publishes.
