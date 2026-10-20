# Code review: LLP 1121 stage 3, commit 3, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `3f67cd27cdbf9f5bfe84218cba96768be8bd89691292bf12a1155320755b752a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three high, three medium; Grok: two medium, two low.
- **Disposition:** fixed in round 2; the lockless editor's window between the last read and the rename is documented, not closed

---

No high findings. The crash points on the normal path hold, and the round-1 fixes for confinement, digests, undo ids, the line cap, and `Checked.names` hold. Two medium holes remain on the journal error paths.

## Medium

**[medium] Recovery renames before the set is known to be applicable — `contract/cli/src/tamber_serve.rs:306`**

`recover` renames each temp into place as it goes. A later entry that has the wrong digest, a read error, or a missing temp whose target does not match returns only after earlier renames have already landed. The journal stays, so the next start retries, but the design is already half-applied. A crash of this server does not produce that journal (temps are fsynced before the record), but a corrupt or hand-written record does, and the one-entry tests never catch it.

Classify every entry first (temp digest matches, or temp absent and the target already has the recorded digest). If any entry is neither, return without renaming. Then perform the renames.

**[medium] A failed journal publish treats a read error as "no journal" and deletes the temps — `contract/cli/src/tamber_serve.rs:773`**

Startup `recover` fail-closes when the journal cannot be read. This path does not:

```773:779:contract/cli/src/tamber_serve.rs
            if read_plain(&journal).ok().flatten().is_some() {
                // Renamed into place but not flushed: the record stands, and
                // completing it now is as good as completing it at start.
                return self.complete_or_exit();
            }
            clean(&renames);
            return Err(fail(format!("the journal: {e}")));
```

`.ok()` turns a read error into absence, `clean` removes the only copies of the new text, and a journal that did rename into place is left behind. The next start keeps that journal and refuses, with the postimage gone.

On `Err`, leave the temps and call `complete_or_exit` (or exit). Delete them only when the read succeeds and the journal is absent. Same block: if `sync_dir` fails after the rename, exiting here is safer than `complete_or_exit`. The journal directory entry is not durable yet, and `recover` will start design-file renames that can reach disk on their own.

## Low

**[low] Replacing `.exact` after start bypasses the lock — `contract/cli/src/tamber_serve.rs:248`**

The lock is an fd on the inode opened at start. Temps, the journal, and `sweep` look up `.exact` by path again. Renaming that directory aside and putting another in its place sends later writes at the new directory, and a second server can lock the new `tamber.lock`. Pin the directory (a directory fd / `openat`, or compare device and inode on each commit) and refuse if it changed.

**[low] On Windows, `create_new` can follow a dangling reparse point — `contract/cli/src/tamber_serve.rs:172`**

The `#[cfg(unix)]` gate is right: `libc` is a Unix-only dependency, `O_NOFOLLOW` is only applied there, and the file builds on Windows. `sync_dir` is a no-op off Unix because flushing a directory handle is not portable there. `OpenOptions::create_new` is `CREATE_NEW` without `FILE_FLAG_OPEN_REPARSE_POINT`, so a dangling symlink planted at the temp name can be followed and the new text created at the target. On Unix, `O_EXCL` already fails if the last component exists; `O_NOFOLLOW` matches that.

## Checked, not findings

**Crash order, normal path.** Temps and `tamber.journal.tmp` are written and fsynced, then the journal is renamed, then `.exact` is fsynced, then design files are renamed, then their directories are fsynced, then the journal is removed and `.exact` is fsynced again. A crash before that first `.exact` fsync leaves no durable journal name (the temp-file fsyncs happen before the rename), and startup sweeps the temps. A crash after it, including between renames and before journal removal, leaves a durable record: `recover` redoes a temp whose digest matches and treats a missing temp plus a matching target as already done. `complete_or_exit` is safe when every rename has already succeeded: temps are gone, targets match the recorded digests, and `recover` only removes the journal. It is also the right retry when a later rename failed and the journal is already durable.

**Re-check window.** One snapshot is what the digest check, the plan, and the undo precondition use. The re-read compares those same snapshot digests, for files the plan will change, and it happens before the journal rename. A hand edit in that gap is overwritten once the journal is the commit point. Closing it would mean holding the destination inodes across the fsync, which this protocol does not do.

**Sweep.** It runs only after `recover` returns, so a journal that is kept still has its temps. Names have to match `tamber-*.tmp` or `tamber.journal.tmp`. Removing a symlink removes the link, not its target.

**1 MiB reader.** Over-long lines are discarded through the newline and answered with an error; the next line is still framed. Non-UTF-8 is an error reply. The server stays up. Exactly 1 MiB is accepted.

**`Checked.names`.** The edge is recorded in `Scope::named` before `value()`, so a memo hit still counts for the caller. Unselected `when` lines are folded with that token on the stack, so their names are included. `influence` clears the memo and walks the same expressions again; it can only add refs, and it runs after unselected lines are already recorded. Spellings go through `role_key`, the same row spelling as `Checked.name` (`-exact-label`, `Canvas`). Order is token index, which is declaration order. Dials that appear only on a `when` no tuple selects stay out of `dials` (influence never folds that line). That matches the existing "dials that move this token" meaning; `names` is the wider set.

**API for commits 4 and 5.** `tamber/1.0` over one JSON object per line. `inverse` is `inv-<pid>-<start>-<n>`, not the restores; `undo` is the only way to apply them. `changed` is `{file, token}` sorted, using those IR spellings, on both the reply and the `changed` event. The event follows the reply and carries `files`, `changed`, and `client`, not `class` (that stays on the reply). `propose` returns the baseline digests and does not store a proposal; a held-proposal id can be added beside that. Do not put commit 5 state in `.exact` under `tamber-*.tmp` — startup sweep removes those names.

**Tests and caps.** The integration tests cover the crash-between-renames restart, a journal that escapes the app or loses its temp, typed requests, the 1 MiB and non-UTF-8 replies, propose digests, attribution, undo scope across files, and explain/resolve/describe on the small design. They do not cover a multi-entry bad journal, a journal read error during publish, or an unselected `when`'s names. Nothing in this diff crosses the 1,500-line cap (`tamber_serve.rs` is 971, `check.rs` is 975). `host/web/collection.test.mjs` is untouched.
