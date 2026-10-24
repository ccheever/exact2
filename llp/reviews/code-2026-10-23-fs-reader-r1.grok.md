# Code review: filesystem reader retry, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3b41efe390`.
- **Method:** one brief (sha256 `262e2eb8fc6ffa5e9b7d1e77c67856b5f68831c1578e8292ffe18899696523b0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The retry is one extra `send` per `filesystemRead`. The `.catch` in `filesystemRead` checks `error.code === 'EXACT_FS_READER_EXITED'` and returns the second `send` with no further catch, so a second exit, a spawn `error`, an unexpected reply, or `closeFilesystemReader` (plain `"filesystem reader closed"`, no code) rejects the read. `fail` clears the module `reader` only when it still points at that session, and it does so before rejecting, so the re-ask's `??=` either starts a new helper or uses one a sibling retry already started. A session that already failed returns `Promise.reject(failure)` without writing. Pending entries are either shifted and settled from a stdout line, or spliced and rejected once; a late line hits an empty queue and `fail` returns because `failure` is already set. `fail` removes the process-exit listener, kills the child, and drops the liveness ref before the retry microtask runs; the new reader uses the same idle-unref path, and the new test's process exits. Only `get` is retried: `filesystemRead` rejects anything else, and `--serve-reads` refuses non-get before `operate`. `Directory.read` is `O_RDONLY` and does not create parents. The stub test passed (`bun test scripts/install-page.test.mjs -t 'resident reader'`, 1 pass). Both writes land before the one-shot helper runs, so the `xx` spawn counts are stable here: one unanswered read is re-asked once, a helper that exits 42 is re-asked once and then fails, and a synchronous close is not re-asked. That is the rejection `staticRead` was turning into a 404. Removing the QUEUE entry matches the tree: digest-named capture (`ef56fda74d`) is what stopped the cross-worktree `ENOENT`, and `a7656a62f3` is what stopped the idle reader holding Bun open.
