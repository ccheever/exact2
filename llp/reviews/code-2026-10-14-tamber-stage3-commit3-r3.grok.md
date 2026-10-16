# Code review: LLP 1121 stage 3, commit 3, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `3b9cf8131a7a27422d0b24124f3fb9d1b1fbe0ee39ff93e7a10cf19b7b74986a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, three medium; Grok: three medium.
- **Disposition:** applied without re-review (before digests in the journal and withdrawal; the held lock file's identity rather than a directory descriptor)

---

No high-severity findings. Three medium ones are in the new failure paths; the round-2 fixes hold on the paths they were written for.

**Medium** — `contract/cli/src/tamber_serve.rs:833`

If the late digest check decides to withdraw and `remove_file` of `.exact/tamber.journal` fails, `stop()` leaves a valid journal and the temporary files in place. Nothing on disk records the conflict. The next start takes the normal recovery rule: a temporary file whose digest matches is renamed onto the design file. That overwrites the edit the withdrawal existed to keep, and the client never got the conflict response. Poison the journal before exiting (overwrite it with bytes `journal()` will reject, `sync_all` that file, then `sync_dir`) so recovery keeps it and renames nothing. Unlink-and-fsync should stay the success path.

**Medium** — `contract/cli/src/tamber_serve.rs:337` (`sweep` at `:409`, called from `:315`)

`recover` returns immediately when no journal is visible, and does not fsync `.exact`. `start` then deletes every `tamber-*.tmp`. That is wrong after a crash, or after `stop()`, in the gap between unlinking the journal and a successful directory fsync (the withdrawal fsync at `:833`, or the post-unlink fsync at `:866`): the unlink is only in the page cache, the temps are still the journal's copies. A warm restart sweeps them. A later power loss can persist those deletions and not the journal unlink. Recovery then sees the journal, no temporary file, and a design file that does not have the recorded digest, and refuses to start. On the no-journal path, fsync `.exact` before `sweep`, and if that fsync fails, do not sweep.

**Medium** — `contract/cli/src/tamber_serve.rs:849` (`recover` at `:337`, pin checked only at `:760`)

`same_state` runs once at the start of `commit`. Renames, the journal, and `recover` look up `.exact` by path again. `recover` treats a missing journal as success, and `complete_or_exit` returns that success to the client. If `.exact` is removed or replaced after the pin check and a rename then fails, the client is told the commit landed, any renames already done are left in place, and a journal still sitting in the old directory is never replayed. Hold the locked directory with an `O_DIRECTORY|O_NOFOLLOW` descriptor and do the journal and renames through it. On this in-commit path, an absent journal is success only when every recorded target already has its digest.

What holds: recovery fsyncs `.exact` and judges every entry before any rename, including a fresh start where the set cannot complete. The server refuses to start off Unix, so the non-Unix no-op `sync_dir` is unreachable. The digest re-check is after the journal fsync and before the renames; a successful withdrawal (unlink and directory fsync) really does drop the record. Temps are directory-fsynced before the journal names them. A failed fsync after unlinking the journal is retried once and then `stop()`s, and it is not reported as success. An unreadable journal on the publish error path stops instead of being treated as absent. Creating `.exact` fsyncs the app directory. The remaining editor window between the last read and the rename matches the module comment.
