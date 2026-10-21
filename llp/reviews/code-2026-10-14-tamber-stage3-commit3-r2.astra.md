# Code review: LLP 1121 stage 3, commit 3, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `3f67cd27cdbf9f5bfe84218cba96768be8bd89691292bf12a1155320755b752a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three high, three medium; Grok: two medium, two low.
- **Disposition:** fixed in round 2; the lockless editor's window between the last read and the rename is documented, not closed

---

Six findings remain: **three high, three medium. No low-severity findings.**

1. **High — Recovery can rename files without a durable intent.** [tamber_serve.rs:774](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:774)  
   If publishing the journal succeeds but syncing `.exact` fails, `complete_or_exit()` enters `recover()`, which immediately starts renaming files. It never retries the intent’s durability barrier first. A crash during this recovery can therefore leave changed files without a persisted journal. Restarting after a process crash between journal rename and directory sync has the same exposure. **Fix:** require a successful journal-directory sync before any recovery rename; preserve everything and exit if that fails.

2. **High — Windows silently loses every directory durability barrier.** [tamber_serve.rs:185](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:185)  
   `sync_dir()` returns success without doing anything on Windows. Consequently, journal installation, target replacements and journal removal have no enforced persistence ordering. Syncing files before their renames does not establish that ordering; Windows exposes separate write-through move behavior. **Fix:** implement and qualify Windows durable replacement, or explicitly refuse `serve` there until supported. The dependency/cfg gates themselves look consistent. [Microsoft’s MoveFileEx documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexa)

3. **High — The final digest check still permits lost external writes.** [tamber_serve.rs:748](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:748)  
   After checking the targets, the server writes and flushes the journal before replacing them. An editor or `tamber fmt` can save during that interval; the subsequent unconditional rename overwrites those bytes. The returned inverse contains the earlier snapshot, so the intervening edit is lost. **Fix:** make repository writers participate in the app lock, and use a replacement protocol that preserves and validates displaced bytes for external saves. Moving another read closer to `rename()` only narrows this race.

4. **Medium — Temporary filenames are not durable before journal publication.** [tamber_serve.rs:761](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:761)  
   Each temporary file is synced, but `.exact` is first synced after the journal rename. A machine crash before that sync can leave a surviving journal whose temporary directory entries were never persisted. Recovery then cannot complete the recorded transaction. **Fix:** sync `.exact` after staging the files and before publishing the journal; also persist the app-directory entry when creating `.exact`. File syncing alone does not guarantee directory-entry persistence. [fsync documentation](https://man7.org/linux/man-pages/man2/fsync.2.html)

5. **Medium — A failed final flush becomes a successful commit.** [tamber_serve.rs:794](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:794)  
   When journal removal succeeds and the following directory sync fails, `complete_or_exit()` calls `recover()`. The journal is absent, so recovery immediately returns `Ok`, and the server acknowledges success without completing the failed flush. A crash can resurrect the obsolete journal and disrupt subsequent recovery. **Fix:** track the post-unlink phase and retry its directory sync explicitly; exit if durability remains uncertain.

6. **Medium — The publish-error path still treats journal read errors as absence.** [tamber_serve.rs:773](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:773)  
   `read_plain(&journal).ok().flatten()` discards read errors. If the journal was installed but cannot be read after publication fails, execution deletes its temporary files and continues serving. The retained intent is then unrecoverable. **Fix:** distinguish confirmed absence from an unreadable journal; preserve the temporaries and stop when intent state is uncertain.

I found no additional defect in `Checked.names` across memo hits, influence refolds, unselected values or role spellings; the bounded reader, inverse handling, diff counts and normal startup sweep also look sound. The stated later-stage work remains outside these findings.

The crash test covers process exit after the first rename, but not the persistence and I/O-failure cases above. Those need targeted fault coverage.

Read-only review completed without changes. `git diff --check` passed. `caps` reports only the known pre-existing 1,501-line `host/web/collection.test.mjs`. Cargo builds, integration tests and Windows compilation were not run under the read-only constraint.
