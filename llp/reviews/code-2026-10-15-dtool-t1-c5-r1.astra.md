# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `81011fafc`.
- **Method:** one brief (sha256 `824dcc2cda00345b9179cb25323dfcfd1b97797c9c6a7032c0ded610a64a8504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 8 medium, 1 low).
- **Disposition:** all taken, partly in commit 6 (549504343), which reworked the measurement, and the rest in this round's fix: before and after are measured between the reads of the exact file states either side of each event (each event carries the digests after it), so several writes between two reads are each measured or left named, never given the window's net change; an entry is measured only against its own file (hello now names the design file); a hand edit, which names no tokens, lists every token that moved; the tool's own write and its event are matched by client and the digests after the write, in either order, and every own write (undo and redo too) shows the diff its reply gave; a new server epoch starts the changeset again; Commit first asks the server for a dry run (`commit {dry: true}`, new, which answers the files and edits it would take) and composes the message from that scope, so writes from before the tool opened are named; a commit marks committed only what was written before it was sent (the `committed` event, what came before it); only committed entries are let go past 200; history pages ten at a time with `skip` (Newer and Older); Commit is off when the session has nothing open. Not tested: the own-write race (its order is not reproducible in a drive).

---

1. **High — Commit can include edits absent from the changeset.** [apps/design-tool/app.ts:571](/tmp/rv-dt-5/apps/design-tool/app.ts:571)  
   The first poll’s backlog is discarded, but `commit` commits the server’s entire outstanding session. If an agent edits `brand.tamber` before the tool opens, then Ada edits `app.tamber`, Commit includes both files while its composed message and returned edit list describe only Ada’s edit. Reconcile the server’s outstanding commit scope before composing the message and presenting what will be committed.

2. **Medium — Own-write attribution races the event response.** [apps/design-tool/app.ts:494](/tmp/rv-dt-5/apps/design-tool/app.ts:494)  
   `mine` is populated only after the transaction reply. The server publishes its event before replying, so an outstanding poll can deliver that event first. It becomes `mine: false`; the queued diff then attaches to the next event with that client name. Undo and redo share this race. Correlate replies and events by transaction identity or matching file digests, and reconcile either arrival order.

3. **Medium — Several writes receive the same aggregate before/after values.** [apps/design-tool/app.ts:375](/tmp/rv-dt-5/apps/design-tool/app.ts:375)  
   Every event between two reads is compared against the same snapshots. For successive edits `8 → 12` and `12 → 16`, both entries say `8 → 16`, attributed to their respective authors. An add followed by removal between reads loses both descriptions. Reads can also include writes newer than their `since` argument. Obtain revision-bound before/after data per event; do not present an interval’s aggregate difference as each individual edit.

4. **Medium — Values can come from the wrong file.** [apps/design-tool/app.ts:376](/tmp/rv-dt-5/apps/design-tool/app.ts:376)  
   `noted` discards each changed token’s file, and `valuesFor` never checks whether the entry belongs to the file being read. If two designs contain `radius.md`, reading one can permanently assign its values—or an empty result—to the other’s event. Unvisited files never get a preceding baseline either. Preserve `(file, token)` identities and measure each edited file against its own revision.

5. **Medium — Hand edits never get token-level descriptions.** [apps/design-tool/app.ts:347](/tmp/rv-dt-5/apps/design-tool/app.ts:347)  
   Real watcher events carry `changed: null`. This becomes an empty token list, which `valuesFor` treats as successfully measured. Even changing, adding, or removing a token in the displayed file produces only `app.tamber: edited · changed on disk`. Compare the relevant snapshots’ token union, or supply token differences in watcher events.

6. **Medium — A new server epoch retains incompatible changeset state.** [apps/design-tool/app.ts:554](/tmp/rv-dt-5/apps/design-tool/app.ts:554)  
   Restart clears undo/redo and `mine`, but retains `changeset` and `measured`. New sequence numbers collide with old row keys; measurements are skipped until the new sequence exceeds the old baseline; old entries can enter the new server’s commit message. Scope entries by epoch and sequence, reset measurement baselines, and separate prior epochs from the active commit scope.

7. **Medium — A delayed commit reply marks later writes committed.** [apps/design-tool/app.ts:578](/tmp/rv-dt-5/apps/design-tool/app.ts:578)  
   Success marks every current entry touching a returned file. If another write lands after the server commits but before its response reaches the tool, that later write is marked committed despite remaining outside HEAD. Its description is then excluded from the next message. Mark entries using the acknowledged commit’s epoch and event boundary, rather than filename alone.

8. **Medium — The changeset silently drops outstanding edits after 200 writes.** [apps/design-tool/app.ts:348](/tmp/rv-dt-5/apps/design-tool/app.ts:348)  
   The unconditional slice removes uncommitted entries. After 201 writes, the UI, composed count, and success edit list omit the oldest write, although the server still commits it. Retain outstanding edits, using paging if necessary; do not truncate the authoritative session record.

9. **Medium — History cannot advance beyond 100 commits.** [apps/design-tool/app.ts:584](/tmp/rv-dt-5/apps/design-tool/app.ts:584)  
   “Older” increases `limit`, capped at 100, without supplying `skip`. With 101 design commits, further clicks repeatedly fetch the same 100 while `more` remains true. Use the server’s paging parameters and append or replace pages appropriately.

10. **Low — Undo and redo omit their diffs.** [apps/design-tool/app.ts:511](/tmp/rv-dt-5/apps/design-tool/app.ts:511), [apps/design-tool/app.ts:527](/tmp/rv-dt-5/apps/design-tool/app.ts:527)  
    Both explicitly record `diff: ''`, so an edit followed by Undo and Redo shows lines only beneath the original edit. This misses D9’s review detail for the tool’s own writes. Retain the actual diff returned by each successful operation.

The real-server tests assert meaningful guard refusals, staging preservation, unchanged remote refs, and basic history wording. However, [scripts/design-tool.test.mjs:388](/tmp/rv-dt-5/scripts/design-tool.test.mjs:388) finishes the first write before advancing the polling timer, masking finding 2. The stub keeps resolved values unchanged and emits no transaction events, so its assertions cannot establish value measurement or own-write matching. The new revert assertion checks only a proposal ID; strengthen it with unchanged file contents, an actual held proposal, and refusal rendering. The implementation does correctly call `revert` without accepting it.

Read-only review; builds and tests were not run.

**Verdict: Request changes.**
