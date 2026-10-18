# Code review: LLP 1121 stage 3, commit 5, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `c8f7bf13d6e7fe01fdf3b2f38d8f5bca469b6666fe590f2033c5d04903826ae2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: nine medium, three low; Grok: one high, two medium, two low.
- **Disposition:** fixed in round 2

---

No high-severity findings. I found these remaining issues:

1. **Medium — Index updates do not preserve filenames.** [scripts/tamber-commit.mjs:133](/tmp/rv-s3c5/scripts/tamber-commit.mjs:133) writes raw filenames into newline-delimited `--index-info` input. A legal filename containing a newline can become multiple index records, potentially changing unrelated staged paths. Rollback has the same encoding problem. **Fix:** use `-z --index-info` throughout and parse saved records by their fields.

2. **Medium — SHA-256 rollback can silently fail.** [scripts/tamber-commit.mjs:138](/tmp/rv-s3c5/scripts/tamber-commit.mjs:138) uses a 40-digit zero OID to restore entries previously absent from the index. SHA-256 repositories require 64 digits. Following an `update-ref` failure, restoration therefore fails and its error is swallowed, leaving the session’s entries staged. **Fix:** derive the OID width from the repository and report restoration failures explicitly. The [locked-index test](/tmp/rv-s3c5/scripts/tamber-server.test.mjs:405) fails before this rollback path and does not cover it.

3. **Medium — Overlapping writes overwrite the first write’s baseline.** [scripts/tamber-commit.mjs:61](/tmp/rv-s3c5/scripts/tamber-commit.mjs:61) stores pending facts by path. If write A reaches disk before its reply is processed, write B’s `prepare()` sees A’s changes as dirtiness and replaces A’s clean baseline. A’s reply then permanently marks the session dirty. **Fix:** associate preparation facts with each request and consume those facts in its reply callback.

4. **Medium — Conflict attribution still races later replies.** [scripts/tamber-server.mjs:354](/tmp/rv-s3c5/scripts/tamber-server.mjs:354) compares digests after `await ask()`. When a conflict and a later successful write arrive in one stdout chunk, the later write updates `known` first. The conflict then incorrectly reports `"changed on disk"`. Reproduced in memory. **Fix:** calculate `by` in the ordered `done` callback.

5. **Medium — Recovery from a failed initial read can hide foreign changes.** [scripts/tamber-server.mjs:177](/tmp/rv-s3c5/scripts/tamber-server.mjs:177) treats the first successful refresh as a silent baseline even if writes have already succeeded following the initial read failure. A hand edit observed by that refresh produces neither an event nor `session.seen()`, and can retain the previous client’s attribution. **Fix:** only initialize silently before any file state has been published.

6. **Medium — A timed-out accept can remain held after succeeding.** [scripts/tamber-server.mjs:407](/tmp/rv-s3c5/scripts/tamber-server.mjs:407) restores the proposal on HTTP timeout although its transaction remains in flight. A late successful reply publishes the write but never removes the restored proposal. **Fix:** keep it unavailable until the actual reply settles, and restore only on definitive failure.

7. **Medium — Failed recomputation loses the proposal.** [scripts/tamber-server.mjs:398](/tmp/rv-s3c5/scripts/tamber-server.mjs:398) deletes the original before recomputing, but neither partial-accept nor stale-accept recomputation restores it on failure. A temporary error or invalid selected subset destroys the pending review. **Fix:** retain the original until replacement succeeds, restoring it on failure while unexpired.

8. **Medium — Proposal removals are not consistently announced.** [scripts/tamber-server.mjs:397](/tmp/rv-s3c5/scripts/tamber-server.mjs:397) removes proposals without a lifecycle event. Replacement events omit `replaces`; successful no-op accepts emit nothing; failed accepts crossing expiry disappear silently. Existing SSE subscribers retain proposals that now return 404. **Fix:** announce every terminal transition, including the old ID and acting client, and carry replacement information in broadcasts.

9. **Medium — Preparation repeatedly blocks the server on Git.** [scripts/tamber-server.mjs:349](/tmp/rv-s3c5/scripts/tamber-server.mjs:349) passes duplicate operation paths and every known file for undo. `prepare()` launches three synchronous Git processes per unrecorded path occurrence. Measured here: 100 operations on one file launched **302 processes and took 1.1 seconds**. **Fix:** deduplicate and filter paths before querying Git, return immediately when none need preparation, and prepare only an inverse’s affected files.

10. **Low — Restoring failed accepts bypasses the proposal cap.** [scripts/tamber-server.mjs:407](/tmp/rv-s3c5/scripts/tamber-server.mjs:407) reinserts without enforcing `HELD`. Starting with 64, accepting one, holding another, then failing the accept leaves 65; reproduced. **Fix:** count pending accepts toward capacity or enforce the cap on restoration.

11. **Low — Closing a server leaves its expiry interval alive.** [scripts/tamber-server.mjs:365](/tmp/rv-s3c5/scripts/tamber-server.mjs:365) discards the interval handle. `unref()` permits process exit but does not release its captured server state. **Fix:** retain the handle and clear it in `close()`.

12. **Low — Deleting the main design bypasses stale-inverse attribution.** [contract/cli/src/tamber_serve.rs:706](/tmp/rv-s3c5/contract/cli/src/tamber_serve.rs:706) calls `snapshot()` before constructing `stale`. A missing primary design makes that snapshot fail immediately, returning an ordinary error without `stale` or `by`. **Fix:** include missing inverse targets in conflict detection before rejecting the snapshot.

Ordinary failed/refused writes do refresh pending facts on a later preparation; that suspected stale-cache path is sound.

Caps and `git diff --check` passed. Race and lifecycle reproductions used the actual JavaScript with in-memory dependencies; the preparation measurement used read-only Git commands. The integration suite was not run under the read-only constraint. No files were changed.
