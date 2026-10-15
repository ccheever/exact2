# Code review: LLP 1121 stage 3, commit 5, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `c8f7bf13d6e7fe01fdf3b2f38d8f5bca469b6666fe590f2033c5d04903826ae2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: nine medium, three low; Grok: one high, two medium, two low.
- **Disposition:** fixed in round 2

---

**High** — An in-flight first write's baseline can be overwritten, and commit then refuses that file for the rest of the process. `scripts/tamber-commit.mjs:50-61`, consumed at `scripts/tamber-commit.mjs:70-72`, triggered from `scripts/tamber-server.mjs:349`.

`prepare` always stores a new snapshot for any path not yet in `files`. `wrote` runs only when that write's reply is applied. A second write can run `prepare` after the first has hit disk and before that reply is applied: its `git diff` sees the first write and sets `pending` to "differed from HEAD" or "staged". `wrote` then does `facts.dirty ?? …`, so that string sticks and the `atHead` check never runs. The file stays dirty for the life of the process, including after restart once it no longer matches HEAD, so the session cannot commit its own edit.

Undo makes this likely. It calls `prepare` on every known path, synchronously, so it blocks the event loop long enough for the other write to land, and it refreshes files it will not touch. Two overlapping transactions on a path the session has not yet recorded do the same.

A later write after a failed or refused one does not reuse stale facts. That path is not in `files`, so the next `prepare` overwrites `pending` before `wrote`. Keeping the first snapshot is safe only if a failed write drops it when nothing else is in flight: `atHead !== before` still marks a real content change dirty. Fix: if `pending` already has the path, bump an in-flight count and do not recompute; on success `wrote` consumes it; on failure decrement and delete the entry at zero so the next `prepare` takes a fresh snapshot.

**Medium** — Accept retires a proposal in the map without telling subscribers, and the failure path puts it back after a drop already 404'd. `scripts/tamber-server.mjs:395-408`.

The proposal is deleted before the await, and `proposal-dropped` is never sent for that id. A successful accept only carries the id on the `changed` event (`proposal`, `acceptedBy`). A partial or stale accept emits a new `proposal` and does not say the old id was replaced (`replaces` is only on the accepter's HTTP body). Another subscribed client keeps a review item whose accept is 404 until it polls or resubscribes.

On a non-conflict failure the same object is inserted again if it has not expired, with no event. A drop or second accept during the write was already told there is no such proposal, then it is held again and can be accepted. If it expires during the write, it is not restored and no expiry event is sent, so the timer never sees it. If the recompute `hold` fails, the original is gone and no drop is sent.

Fix: keep it out of `held()` while the accept runs, but remember a tombstone. Emit `proposal-dropped` when it is actually finished (accepted, replaced, or expired). On failure, restore and re-announce only if no drop landed and it has not expired; otherwise emit the drop and do not restore.

**Medium** — Index rollback on a SHA-256 repo can leave the new blobs staged while HEAD stays put. `scripts/tamber-commit.mjs:136-139`.

`ls-files -s -z` round-trips through `update-index --index-info` for entries that were present, including spaces, quotes, backslashes, and tabs. A missing entry is removed with `0` and 40 zeros. SHA-256 git rejects that line (`malformed index info`; the empty name is 64 hex digits). Removal is one batch with the restores, so one absent path fails the whole rollback, and the `catch` ignores that error. The caller only sees the `update-ref` failure. The real index still has every chosen path at the new blob.

This is the path where a file was in the index at the baseline and was removed from the index before commit (`git rm --cached`), or any other chosen path absent from `ls-files`. If every chosen path is still in the index, the absent line is empty and SHA-256 rollback works. Fix: use `'0'.repeat(blob.length)` (or the repo's object format). If that `update-index` fails, include it in the error; do not report only the `update-ref` failure.

**Low** — Every undo re-probes every known file the session has not written, including files the inverse will not change. `scripts/tamber-server.mjs:349`, `scripts/tamber-commit.mjs:55-59`.

Each of those paths is two or three synchronous `git` processes (`diff --cached`, `diff`, `show`) before the undo is sent. That blocks reply handling and widens the baseline race above. One `git diff-index` / `status --porcelain=v1 -z` for the paths not already in `files` or `pending` is enough.

**Low** — The attribution test still passes when the watcher announces the hand edit first. `scripts/tamber-server.test.mjs:280-283`.

Both the fixed comparison and a plain `lastBy` lookup then yield "changed on disk", so a slow run does not prove the digest check. Set `settle` high enough that the undo wins, or assert that no `changed` event for that edit arrived before the 409.

Checked and sound:

- Bytes are read once and passed to `hash-object --stdin`. A Buffer stays intact under `spawnSync`'s `encoding: 'utf8'`. `commit` returns 409 while `writing` is non-zero, and `commit` itself does not yield, so a write cannot start mid-commit.
- The real index is updated in one `--index-info` before `update-ref` with the old value. On SHA-1, failure restores prior entries and removes entries that were absent. Other paths' staging is untouched. HEAD is not moved if the index update throws.
- Mode-only dirtiness at the baseline is caught by `git diff --quiet HEAD` when `core.filemode` is true (the macOS default). `core.filemode=false` ignores the executable bit, which is git's own rule.
- Watcher reads run in the server's reply order via `done`, including a late reply after a client timeout. `rescan` during `writing` sets `again` and runs once when the count hits zero. A failed read does not publish.
- `by` names `lastBy` only when the conflict digest equals the last announced one. An unannounced hand edit or delete does not match, so it is "changed on disk". Deletion matches this too: a missing file is not `undefined === undefined` while `known` still holds the old digest.
- The new `stale` arrays name the paths the wire's `by` map needs. A write that omitted a digest is listed only for files that op touches.
- After a failed write, the next `prepare` of that path recomputes facts. Stale `pending` is not what a later sequential write uses. The bug is the in-flight overwrite above, not a failed write's leftover snapshot.
- Held-proposal shape matches D10 for the tool: `ops` by index, bases stored and sent back on accept, partial or stale accept returns a new proposal instead of writing, drop requires a client, expiry and the 64-cap emit `proposal-dropped`, and `subscribed` / poll carry the current list. `commit` never pushes.
