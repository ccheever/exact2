# Code review: LLP 1122 T1 commit 6 (agents' edits and held proposals), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `905b0a69b` (commit `549504343` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `3a8affbbf9cb7e740308170120c20a726e97210072c5bf834034310252699b11`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 3 low).
- **Disposition:** taken but 4 and 5 in part: the proposer is taken before the accept is awaited (else from the reply), so redo is theirs whichever comes first; a change is measured between the nearest reads around it (its own states, else the closest older and newer read), cell by cell, matched by modes and scheme and mapped to the grid's columns, so a missing intermediate read never lights a cell that did not move; a hand edit is the tokens that moved in its stretch that no other change claims, and the walk goes on past it; a row names every client still lit on it. 4: highlights fade oldest first, one each 3 s, and a file switch restarts the timer (a per-highlight deadline needs a clock the view does not have). 5 is taken as above, from the tool's reads, not the server's. Round 3 was the last.

---

The round-1 and round-2 fixes that are still right: an accept copies the proposal's operations before it waits, a late accept whose server run changed does not push an undo step, a new run can outline again, colours follow first appearance, item labels name the file and an axis value, and a stale accept's new diff is what the inbox shows. Four attribution holes are still open.

## Medium

**1. Redo of an accepted proposal is attributed to whoever pressed Redo, whenever the events poll lands first.** `apps/design-tool/app.ts:977` (operations are copied earlier, at `app.ts:968`).

`acceptHeld` reads the proposer from `held` only after `accept` returns. The server removes the proposal before the write (`scripts/tamber-server.mjs:498`) and answers the waiting poll from inside that write (`scripts/tamber-server.mjs:436` then `:330`), before `handle` writes the accept body (`:353`). `waitChanges` replaces `held` with a list that no longer contains the proposal. `held.find` is then undefined, the undo entry stores no client, and `redoLast` sends `top.client ?? client` (`app.ts:785`), which is `design-tool:<person>`. The bytes are the proposer's operations; the new changeset row, colour, and outline are the person's. The transaction reply already carries the proposer as `ok.client` (`contract/cli/src/tamber_serve.rs:820`) and is ignored.

The tool keeps a poll in flight (`after(1, advance)`), so this is the normal accept, not a rare interleaving. `scripts/design-tool.test.mjs:546` expects `agent:claude`, but only if that poll is applied after `acceptHeld` reads `held`. Delivered in the order the server sends, the assertion fails.

Scenario: accept `agent:claude`'s proposal while the long poll is waiting. Changes says `agent:claude, accepted by design-tool:Ada`. Undo, then Redo. The restored edit says `design-tool:Ada`.

Fix: copy the proposer next to the operations, before the await, and use `reply.ok.client` if you have it. Point the redo assertion at a run where the events response is applied before the accept result.

**2. A change whose two digests are not both cached outlines every column of the token.** `apps/design-tool/app.ts:878`.

`moved` is true for every column when either read is missing. The later event then overwrites every cell of that token, including columns that client did not change and columns an earlier client did.

Scenario: the grid has been read at digest D0. Alice sets only the dark side of `surface.card` (after D1), and Bob sets only the light side of `ink.on-brand` (after D2), and both events arrive before the next read. The read stores D2 only. Alice's light cell and Bob's dark cell are outlined. D0 and D2 are both cached, and those two cells did not change between them.

Fix: for each token, compare the newest cached read at or before the event with the oldest cached read at or after it, and outline only columns whose `css` differs. Do not treat a missing intermediate digest as "the whole row".

**3. A hand edit announced with a write still outlines nothing, and it also clears older outlines on tokens it may not have touched.** `apps/design-tool/app.ts:867` and `:590`.

`measure` names a tokenless event only when both of that event's own digests were read. The write path publishes the hand edit first with `changed: null` (`scripts/tamber-server.mjs:427`), then the write. One poll delivers both, and the grid is read once, at the final digest. The walk includes the write, then `break`s at the hand edit, so the hand edit's cells get no border and no "changed by" line, and every older outline on that file is dropped with them. Changes still says the file was edited.

The commit treats this as needing the server to keep file texts. The tool already has the read before the batch and the read after it. Tokens that differ between those two reads and are not in the write's `changed` list are the hand edit's.

Scenario: Alice's dark-only edit of `surface.card` is on screen. A hand edit of `tone.warm` is saved, and an agent write is published in the same breath. The next grid shows Alice's new dark value with no outline, and `tone.warm`'s new value with none either.

Fix: measure a tokenless event between the nearest cached reads around it. Outline the tokens that moved there and that no later event in the batch already claimed. Keep walking older events for tokens that pair did not move.

## Low

**4. The "changed by" line is still one client for the whole token.** `apps/design-tool/app.ts:874` and `apps/design-tool/app.contract:647`.

Borders are per cell when both reads exist. `rows` keeps only the latest client to touch the token, and the caption uses that one name. Alice sets only the dark side of `surface.card`, Bob then sets only the light side: the dark border is Alice's colour and the line reads "changed by Bob". Put the client on each outlined cell, or name every client still showing on that row.

**5. Leaving the file restarts the 3 s fade.** `apps/design-tool/app.contract:390`.

The cursor is per file, so one file's fade no longer rewinds another's. The timer exists only while `lit.names.length > 0`. Switching to a file with no outline drops it; switching back arms it from that moment, same key or not. Look at `app.tamber` (sequence 11), open `brand.tamber` for 10 s, and come back: sequence 11 is outlined again for another 3 s. Store the deadline when the outline appears, and fade from that deadline on whichever file is open.

**6. The new assertions do not cover these cases.** `scripts/design-tool.test.mjs:521` and `:546`.

The dark-only check is one client and one token, so finding 4 stays green. The redo check does not apply the events poll before the accept result. Nothing writes a second client onto the other side of a token, switches files during a fade, or saves a hand edit in the same poll as a write and checks which cells are outlined.

Verdict: request changes — redo of an accepted proposal is still the person's when the poll lands first, and a change without both digests cached still outlines cells that client did not change.
