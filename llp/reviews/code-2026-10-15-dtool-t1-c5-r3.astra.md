# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f42acb24d` (commit `81011fafc` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `3a39a300bd957a5be0e5c35b04bd80aa88cb650c21832eeac2a6583029d1b0e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 5 medium, 1 low).
- **Disposition:** taken but 2 in part: the dry run (and the scope Commit's availability reads) goes with the token of the run the poll is on, and the session is checked before the commit is sent; each server edit now carries its write's number (tamber-commit.mjs, tamber-server.mjs) and is described by the row of that write, in that file's words, else its tokens and client, so an edit with no row never takes another's; the success list is the scope's lines as composed; a scope the server could not answer leaves Commit on (pressed, it says why), read again with each poll answer; a mode only the old file had is said removed; history is read again on a new run; the success list is keyed by position. 2 in part: a gap does not reconcile old rows, but with write numbers an edit is only ever described by its own write's row. Round 3 was the last.

---

Read-only review; tests were not run.

1. **High — Commit can still cross server epochs.** [app.ts:837](/tmp/rv-dt-5r3/apps/design-tool/app.ts:837)  
   The dry run uses token-renewing `call`. If the server is replaced while that request retries, it can read the replacement’s scope and then commit its work using the old changeset’s descriptions. Even if polling advances `session` meanwhile, the check at line 854 occurs **after** sending the commit; the UI can report “nothing was written” after committing. **Fix:** bind the dry run to the observed server run and validate that run again before sending the mutation.

2. **Medium — A missed commit event lets old descriptions enter a new commit.** [app.ts:843](/tmp/rv-dt-5r3/apps/design-tool/app.ts:843)  
   A same-epoch `missed` response retains existing rows and their uncommitted flags. Suppose another client commits the recorded `8 → 12` edit during a gap exceeding 256 events, then the same author writes `12 → 16`. The next dry run contains only the latter write, but this author/file/token lookup selects the retained `8 → 12` row. The scope check passes and git receives the wrong description. **Fix:** include write numbers in the server’s scope edits, match by epoch/write/file, and reconcile outstanding rows after gaps.

3. **Medium — A delayed success response can describe a later, uncommitted edit.** [app.ts:859](/tmp/rv-dt-5r3/apps/design-tool/app.ts:859)  
   Success recomputes descriptions from the current uncommitted rows. If polling receives the committed event first, the original rows are excluded. A subsequent same-author/token edit can then supply the success description: committing `8 → 12` is reported as committing `12 → 16`, although the latter remains outstanding. Without another edit, the measured description degrades to token names. **Fix:** retain the descriptions composed for the checked dry-run scope and return those after success.

4. **Medium — A temporary scope failure can leave Commit disabled indefinitely.** [app.ts:820](/tmp/rv-dt-5r3/apps/design-tool/app.ts:820)  
   Every error without a per-file refusal becomes `{open: 0, refused: false}`. For example, write A succeeds and triggers a scope refresh while write B remains in flight; the server refuses the dry run as busy. If B then fails without publishing an event, `since` never changes, so scope is never retried and A cannot be committed through the button. Reconnect also does not refresh scope. **Fix:** distinguish unavailable scope from an empty scope, expose the failure, and retry on a later poll or reconnect.

5. **Medium — Removed modes disappear from the measured changeset.** [app.ts:548](/tmp/rv-dt-5r3/apps/design-tool/app.ts:548)  
   `moved` iterates only the new tuples. Removing `density=compact` and its `radius.md = 4px` branch, while leaving surviving modes unchanged, returns `null` despite both exact file versions being available. A hand edit consequently becomes merely “edited,” and measurement is finalized without the removal. **Fix:** compare the union of old and new mode keys and explicitly describe removed tuples.

6. **Medium — History remains attached to the previous server when sequence numbers match.** [app.contract:340](/tmp/rv-dt-5r3/apps/design-tool/app.contract:340)  
   History depends on visibility, page and `since`, but not `epoch`. Open history at sequence zero, then replace the server with another app whose sequence is also zero: hello and the design refresh, while the previous app’s commits and Revert buttons remain displayed. **Fix:** invalidate history on epoch changes and reset its page for the new run.

7. **Low — The regression fixtures do not establish the new protocol guarantees.** [tamber-stub.ts:63](/tmp/rv-dt-5r3/apps/design-tool/stub/tamber-stub.ts:63)  
   Stub writes omit write numbers and transaction events; resolved values remain fixed across changed digests; commit always returns the same outstanding scope, including after success. These fixtures cannot catch the attribution, measurement or availability failures above. The server’s [scope assertion](/tmp/rv-dt-5r3/scripts/tamber-server.test.mjs:319) tests an arbitrary invalid string, not a previously valid scope invalidated by another write. **Fix:** add changing values and faithful write/scope identities, and use the existing proxy to control event/reply ordering, restart during dry run, and an intervening write.

Verdict: Request changes.
