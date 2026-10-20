# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ca47c46f2` (commit `81011fafc` and its round-1 fixes).
- **Method:** one brief (sha256 `e5086f8a7666df705ce877b078544784eeb6d82e9328c165a8a55736265e8553`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium, 1 low).
- **Disposition:** all taken, as in the astra record.

---

The round-1 fixes that hold are the per-event digest measurement for events the tool actually recorded, hand-edit token text on the open file, history `skip`, Commit turning off when nothing is open, the dry-run message scope, sequence-bounded committed marks, the 200-row cap dropping only committed rows, and undo/redo diffs. The gate tests still drive a real server through the proxy and check dirty, staged, and hand-edited-between-writes refusals, untouched staging, and no push. I inspected those paths and did not run the suite.

## High

**A restart or a missed poll still measures the next edit from the old baseline** — `apps/design-tool/app.ts:715`

An epoch change clears `changeset` and leaves `lastFiles` and `reads` on the previous run. `lastFiles` is updated only while noting an event, or on the first poll (`app.ts:720`). A new epoch answers with `missed` and no events, so the baseline stays. The same hole is a missed poll in one run: `recent` is cleared and `lastFiles` is not moved up to that answer's `files`. The next event's `before` is the stale digest. `measure` (`app.ts:501`) pairs it with whatever snapshot is still stored there.

The tool is open on hue 200. The server restarts (or another app takes port 7740) and the file on disk is hue 180. Ada sets 190. The new row says `tone.base …200 → …190`. Commit writes that sentence. The edit was 180 → 190. After a gap of more than the 256 kept events, the next row is labeled with every token that moved in the gap.

On an epoch change or `missed`, set `lastFiles` from that answer's `files` and drop `reads` before noting anything in the answer. Extend the stub test so a post-restart event's text is the new file's own delta.

## Medium

**Distinct edits of one token share one sentence, and the duplicate is dropped** — `apps/design-tool/app.ts:750`

`words` uses `open.find` and keeps the oldest row with that client, file, and an overlapping token. Every server edit of that token reuses it. `app.ts:753` then puts the sentences through a `Set`. The heading count is `scope.edits.length`, so the count and the bullets diverge. The success list uses the same `words` (`app.ts:763`).

Ada sets hue 200 → 210, then 210 → 220. The dry-run scope has two edits. Both resolve to the first row. The message and the "Committed" list contain one bullet, `…200 → …210`, under "2 changes". The second edit is missing from the text Commit stores, except for the server's own appended token line.

Pair each server edit with one unused row (consume it), and keep one bullet per edit.

**The first file measured freezes a multi-file edit** — `apps/design-tool/app.ts:504`

`values` is set as soon as one edited file has both reads. Token names are a set with no file. A token this file does not hold becomes `null` and is dropped. Later reads skip the entry because `values !== null` (`app.ts:500`).

One transaction changes `radius.md` in `app.tamber` and `surface.card` in `brand.tamber`. The open file is measured first. The row and the commit bullet say only `radius.md 8 → 12px`. `surface.card` is gone, including after switching files. Two files that both contain `radius.md` keep whichever file was measured first.

Leave `values` unset until every path in `n.files` has been measured, and store each file's sentence separately.

**A reply claims the newest unmatched row with the same after-digest** — `apps/design-tool/app.ts:432`

`wroteMine` marks `changeset.find` (newest first) for this client and after-digest. The poll re-asks after 1 ms (`app.contract:305`), so the reply often arrives while the event is still only on the server. The name `design-tool:<person>` is shared by every tab. A missed poll does not drop `mine`, so a queued diff whose event was discarded stays available for a later match.

Both tabs are Ada. Tab B sets hue to 210. The file moves off that value and Tab A sets 210 again, so both events end at the same digest. Tab A's reply lands before its event and attaches Tab A's diff to Tab B's row. Tab A's own row then has no diff.

Put the event `seq` (or the inverse id) on the transaction reply and match on that. Drop `mine` when a poll is `missed`.

## Low

**The dry run and the commit are two requests** — `apps/design-tool/app.ts:745`

The message is composed from the dry-run scope, then a second `commit` runs with no check that the scope is unchanged. The server appends its own lines for whatever it actually committed.

An agent writes `brand.tamber` after the dry run returns and before the real commit. Git's heading still says "1 change" and the bullets omit the agent. The server's suffix lists the extra file, so the object is not silent, and the heading is wrong.

Have the server refuse the second call when the session's edits are not the dry-run set, and compose again.

Verdict: request changes — a restart or a missed poll still measures the next edit from the stale baseline, and Commit writes that text into git.
