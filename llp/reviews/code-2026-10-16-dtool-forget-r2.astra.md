# Code review: the design tool's undo stacks after a server restart (main red), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ad961cfdb`.
- **Method:** one brief (sha256 `dc4c216b49e963bab1a391a563f785c04bd524c70b5d8aecbd38e32652ba90be`), a quick review under the main-red standing permission. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** needs changes (3 medium).
- **Disposition:** taken in round 3's form. Dropping entries by token is gone: `forget(run)` empties the stacks and moves the session once per run gone. So an in-flight operation of the old run is fenced again, with empty stacks too, and the old run's answers are no longer shown. Each refusal passes its own `used` token (no shared hand-off). An accepted proposal records the token its accept used. Finding 3 (a write of the kept run discarded when the old run's entries go) is gone with the per-entry drop. It is the old unconditional forget's behaviour again.

---

- **Medium — [app.ts:72](/tmp/rv-dt-forget2/apps/design-tool/app.ts:72), [app.ts:493](/tmp/rv-dt-forget2/apps/design-tool/app.ts:493): concurrent refusals can invalidate the wrong run.** After A→B→C, B’s undo refusal can set `refusedBy=B`, then an outstanding A-bound request can overwrite it before the undo caller resumes. `restarted()` removes A’s entries, leaving B’s. The subsequent poll removes B’s entries and bumps `session`, hiding the refusal again. **Fix:** pass each request’s returned `used` into `restarted(used)`; remove the shared hand-off.

- **Medium — [app.ts:488](/tmp/rv-dt-forget2/apps/design-tool/app.ts:488): empty stacks no longer invalidate outstanding old-run operations.** A first write can succeed on A, with its response delayed until after B’s poll. Because there were no entries to remove, `session` stays unchanged and the delayed success installs A’s obsolete inverse. Proposal acceptance additionally records the now-B `heldToken` at [line 1281](/tmp/rv-dt-forget2/apps/design-tool/app.ts:1281), misidentifying the inverse’s owner. **Fix:** track invalidated runs independently of stack contents, check request ownership after awaits, and store acceptance’s actual `used` token. The same missing transition also leaves completed A notes/errors displayed through [the stack derive](/tmp/rv-dt-forget2/apps/design-tool/app.contract:473) when both stacks are empty.

- **Medium — [app.ts:490](/tmp/rv-dt-forget2/apps/design-tool/app.ts:490): removing old entries invalidates operations of the retained run.** Suppose undo contains A and B entries, and B’s undo succeeds while the first B poll removes A’s entries. The poll preserves B’s entry but increments `session`; [the undo guard](/tmp/rv-dt-forget2/apps/design-tool/app.ts:996) consequently discards the successful response. B’s consumed inverse remains on undo, no redo entry is created, and the answer incorrectly says nothing was written. **Fix:** invalidate pending operations by their run token so forgetting A permits B’s operation to finish.

All callers traced. The successful poll’s `used` identifies its responding run, and redo correctly preserves its entry’s token. Normal proposal buttons require a successful poll, so I found no ordinary UI path creating an empty-token entry.

Source review only; no tests run or files changed.

Verdict: **Needs changes—three Medium issues remain in restart handling.**