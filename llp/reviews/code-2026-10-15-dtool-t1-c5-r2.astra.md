# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ca47c46f2` (commit `81011fafc` and its round-1 fixes).
- **Method:** one brief (sha256 `e5086f8a7666df705ce877b078544784eeb6d82e9328c165a8a55736265e8553`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 8 medium, 1 low).
- **Disposition:** all taken (with commit 6's round-1 fixes, which share the code): the commit goes with the token of the run its dry run read, and with the dry run's `scope`, which the server now checks (`commit {scope}` refuses if the session wrote since; a dry run of files that are HEAD's again says nothing to commit); each server edit is described by its own changeset row, used once; the baseline is the poll's files whenever it starts again (first answer, `missed`, a new run), and a new run drops the kept reads; each edited file is measured on its own, by that file's digest, and an entry is complete per file; the tool's own write and its event match by the server's write number, now on both (tamber-server.mjs); Commit is on when the server's dry run has something or a refusal; tuples are compared by their modes, a mode only one side has said so.

---

Read-only review of both commits against HEAD; builds and tests were not run.

1. **High — Commit can cross server epochs.** [app.ts:757](/tmp/rv-dt-5r2/apps/design-tool/app.ts:757)  
   Both commit requests use token-renewing `call`, without checking the server epoch. If the server is replaced between the dry run and commit, the request can obtain the replacement’s token and commit its outstanding work using the old message. A delayed old-epoch success can also mark new-epoch entries committed because line 762 compares sequence numbers without their epoch. **Fix:** bind both requests to the observed epoch, prevent mutation retries against a replacement server, and reject stale replies before updating rows.

2. **Medium — The final commit is not bound to the dry-run scope.** [app.ts:745](/tmp/rv-dt-5r2/apps/design-tool/app.ts:745)  
   The dry run supplies no revision or precondition that the subsequent commit checks. An agent can finish another write between requests; the server commits both writes under a message composed for one. More seriously, another client can commit the inspected scope and start fresh work, which this request then commits under the old description. The server’s appended edit list does not correct that description. **Fix:** return a scope revision and require it on commit, refusing and recomposing when it changes.

3. **Medium — Repeated edits collapse onto the oldest matching description.** [app.ts:749](/tmp/rv-dt-5r2/apps/design-tool/app.ts:749)  
   `words` repeatedly selects the first row sharing an author, file and any token; it never consumes that match. Ada’s successive `radius.md 8 → 12` and `12 → 16` therefore both select `8 → 12`. `Set` then removes the duplicate, leaving the commit message and success list without the second edit. **Fix:** associate server edits with unique write identities and preserve each edit’s corresponding description.

4. **Medium — Epoch resets and missed polls retain the wrong measurement baseline.** [app.ts:715](/tmp/rv-dt-5r2/apps/design-tool/app.ts:715)  
   Reset clears `changeset`, but neither resets `lastFiles` from the response nor clears the old reads. The assignment at line 720 runs only for an initial request. For example, the old server ends at radius 8, the replacement starts at 10, and its first write sets 12: the entry can say **8 → 12**, attributing the intervening change to that writer. A missed-event gap has the same problem. **Fix:** establish the response’s file state as the new baseline on resets and gaps, invalidate incompatible measurements, and explicitly represent missing history.

5. **Medium — Multi-file events still lose file-qualified values.** [app.ts:500](/tmp/rv-dt-5r2/apps/design-tool/app.ts:500)  
   `noted` still discards each token’s file, and one `values` array represents the whole event. A transaction changing `app.tamber`’s radius from 8 to 12 and `brand.tamber`’s radius from 20 to 24 becomes one radius name. Measuring the displayed file finalizes the entire entry; visiting the other file cannot complete it. Multi-file hand edits likewise retain only the first measured file’s changes. **Fix:** retain `(file, token)` identities and measurement completion separately for every edited file.

6. **Medium — Unrelated file changes prevent otherwise valid measurements.** [app.ts:501](/tmp/rv-dt-5r2/apps/design-tool/app.ts:501)  
   Read keys contain the entire repository-wide digest map. Suppose A is read at `{A:a0, B:b0}`, B changes, then A changes, and A is read at `{A:a1, B:b1}`. A’s event expects a read at `{A:a0, B:b1}`, so its values remain missing even though both exact versions of A were read. **Fix:** key each design snapshot and its event boundaries by that file’s digest, using a canonical filename.

7. **Medium — Digest matching still attaches diffs to older writes when states repeat.** [app.ts:432](/tmp/rv-dt-5r2/apps/design-tool/app.ts:432)  
   Matching searches every unmatched row with the same client and resulting digest. Another Ada tab can write `8 → 12`, an agent then writes `12 → 16`, and this tab writes `16 → 12`. If this tab’s reply precedes its event, it attaches the new diff to the older Ada row ending at 12. The actual new event remains `mine: false`. **Fix:** use a unique transaction/event identity, scoped by epoch; a resulting content digest alone cannot identify a write.

8. **Medium — Reloading the tool can disable Commit despite outstanding server writes.** [app.contract:667](/tmp/rv-dt-5r2/apps/design-tool/app.contract:667)  
   Commit availability comes exclusively from locally recorded entries, while the first poll deliberately discards backlog. Edit a file, reload the page while the server remains running, and Commit stays disabled until another write occurs—even though the new dry-run API would report outstanding work. **Fix:** derive commit availability from the server’s outstanding scope, separately from “changes since this page opened.”

9. **Medium — Mode changes compare different tuples as though they were the same.** [app.ts:487](/tmp/rv-dt-5r2/apps/design-tool/app.ts:487)  
   `moved` compares tuple array positions rather than mode identities. A hand edit inserting `cozy` between `regular` and `compact` compares the old compact value with the new cozy value, then labels that transition as cozy. Exact file snapshots do not make those tuples equivalent. **Fix:** compare values by named mode assignments and represent added or removed tuples explicitly.

10. **Low — Dry run reports work that the actual commit immediately refuses.** [tamber-commit.mjs:168](/tmp/rv-dt-5r2/scripts/tamber-commit.mjs:168)  
    After an edit followed by Undo, the session retains two writes but its files equal HEAD. Dry run returns success before the empty-tree comparison at line 181; the actual commit then reports “nothing to commit.” **Fix:** perform the no-change determination before returning dry-run success, without writing objects or an index.

The round-1 fixes correctly retain outstanding entries beyond 200, page history with `skip`, retain undo/redo diffs, and prevent ordinary same-epoch delayed replies from marking later writes committed. Isolated hand edits now receive token values, and unread intermediate states no longer receive a fabricated aggregate delta.

The real-server assertions meaningfully cover baseline refusals, unrelated staging, unchanged remote refs, history wording, and a held revert. They do not exercise the cases above. The [restart test](/tmp/rv-dt-5r2/apps/design-tool/app.test.contract:275) stops before any post-restart measurement. The [stub hello](/tmp/rv-dt-5r2/apps/design-tool/stub/tamber-stub.ts:43) also omits `design`, causing default-file measurement to be skipped. Extend these tests with changing values and controlled response ordering through the existing proxy.

**Verdict: Request changes.**
