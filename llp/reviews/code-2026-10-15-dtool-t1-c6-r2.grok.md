# Code review: LLP 1122 T1 commit 6 (agents' edits and held proposals), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f42acb24d` (commit `549504343` and its round-1 fixes).
- **Method:** one brief (sha256 `8c24e13f445550be51df30e5f79bee9552815834fe9c8b6aaa80c2595a5bef53`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 2 low).
- **Disposition:** taken but 2: per-cell outlines; a file switch no longer drops or rewinds a fade (per-file cursor, oldest first); redo of an accepted proposal is its proposer's; axis items name their value; tests for the stale diff, the redo's client and a dark-only outline. 2 (a hand edit announced with a write, its cells unlit) is not: the server would need to keep the files' texts to name its tokens.

---

The simple path is fixed and covered: an agent's write shows on the changeset and on the token, an accept keeps its operations for redo, and a new server run can highlight again. Four attribution cases are still wrong.

Round-1 items that hold at `f42acb24d`: redo copies the proposal's operations before the accept call, and the real-server drive checks the file bytes after undo and redo (`scripts/design-tool.test.mjs:536`). A late accept whose session changed pushes nothing (`apps/design-tool/app.ts:893`). The fade mark is tied to the server run (`fadedEpoch`), and the stub restarts and expects the outline back (`apps/design-tool/app.test.contract:289`). Agent events that already name their tokens are included behind the newest read of that file. Colours follow first appearance, a row adds "accepted by" when that client differs, item labels name `file` when the operation has one, and `unset-token-when` is words rather than JSON.

## Medium

**1. The outline is the whole token row, so a cell is credited to a client who did not change it.** `apps/design-tool/app.ts:809` and `apps/design-tool/app.contract:619`.

`loadLit` keeps one client per token name. Every column of that row takes that client's border and the same "changed by" line. A mode that did not move is outlined with the ones that did. A later edit of another mode replaces the client for every column.

Alice sets only the dark side of `surface.card`. Bob then sets only the light side. Both cells draw Bob's colour and say "changed by Bob". The light cell is Bob's; the dark cell is Alice's. The changeset still has both rows. The real-server test only checks `lit.names` and `lit.whos` for one token (`scripts/design-tool.test.mjs:519`), so this stays green.

Keep the changed columns: when the two reads exist, compare each column, and when they do not, keep the event's token only for columns whose displayed value differs from the previous read. A column keeps the newest client who changed that column.

**2. A hand edit announced in the same breath as a write still outlines nothing.** `scripts/tamber-server.mjs:427` and `apps/design-tool/app.ts:566`.

The write path publishes the hand edit first with `changed: null`, then the write. One poll delivers both. The grid is read once, at the final digest. The walk does include that older row, but the row has no tokens until `measure` sees both of its digests, and the intermediate digest is never stored. The write's tokens outline. The hand-edited cells get no border and no "changed by" line; Changes says the file was "edited".

Have the server name the hand edit's tokens in that first event (it already has both texts). The walk will then outline them the same way it outlines an agent's `changed` list.

**3. The 3 s fade is dropped by a file switch, and a fade on an older file brings a newer outline back.** `apps/design-tool/app.contract:359` and `:508`.

The timer runs only while the file on screen has outlines. Leaving for a file with none drops it, and `faded` stays where it was, so the outlines return on the way back and start another 3 s. A fade on screen records that file's sequence only. An older sequence on the other file moves the mark backwards relative to a newer change, which is then outlined again.

You are looking at `app.tamber` (sequence 11). You open `brand.tamber`, which has no outline. The timer is discarded. You return: sequence 11 is outlined again. Or brand's latest change is sequence 10, its fade sets `faded` to 10, and app's sequence 11 is outlined again.

The same timer also restarts whenever the newest outlined sequence changes, so one busy token keeps every other outline up. That part was left open on purpose; the file switch is a separate hole in this timer.

Store the deadline when an outline appears, per file or per sequence, and let it fire on whichever file is open. Record the highest sequence that deadline covered.

**4. Redo of an accepted proposal is attributed to the person who pressed Redo.** `apps/design-tool/app.ts:899` and `:738`.

Accept writes as the proposer (`scripts/tamber-server.mjs:541`), and the undo entry stores the operations without that client. Redo sends them as `design-tool:<person>`. The bytes match, which the test checks. The new changeset row, the colour, and the outline are the person's.

Accept `agent:codex`'s proposal. Changes says `agent:codex, accepted by design-tool:Ada`. Undo, then Redo. The restored edit says `design-tool:Ada`, in Ada's colour.

Keep the proposer's client on the undo entry and send that client on redo, as accept already does.

## Low

**5. Two axis-value items on one axis read as the same checkbox.** `apps/design-tool/app.ts:508`.

`add-axis-value` and `remove-axis-value` carry `axis` and `value`. The label uses `name` or `axis`, and `values` (the `add-axis` list), so the value is dropped. "Remove navy" and "remove olive" on `scheme` both read "remove axis value scheme". Accept picked cannot be chosen from the label. Put `value` in that arm.

**6. The new tests miss the cases above.** `scripts/design-tool.test.mjs:492` and `apps/design-tool/stub/tamber-stub.ts:79`.

The stale accept edits `tone.warm` while the proposal edits `accent.fill`, and the assertion checks `replaces` and the client, not the diff. A recompute that reused the old diff still passes. The stub's partial accept copies the old proposal, diff and token list included (`tamber-stub.ts:79`). Nothing asserts "accepted by", which cells are bordered, the redo event's client, or a fade that survives a file switch.

Stale-edit the proposed token and assert the new diff. Give the stub's replacement its own diff and tokens. Assert the redo row's client and one dark-only outline.

Verdict: request changes — the single-write path is attributed, but outlines, batched hand edits, redo, and the 3 s fade still credit the wrong cell or the wrong client, or bring the outline back.
