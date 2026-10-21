# Code review: LLP 1122 T1 commit 6 (agents' edits and held proposals), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ca47c46f2` (commit `549504343`, with two later commits' fixes).
- **Method:** one brief (sha256 `a1057a4813756343ae6ea56044a29c2dedb824a3288cf717c8dfc291f2f97a2a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 3 medium, 1 low).
- **Disposition:** taken: redo of an accepted proposal writes its operations; the fade's mark is per run; the highlight walks back from the newest change a read has shown; accept's result is dropped when the run changed; tests accept, undo, redo, fade and a restart; colours go to clients in the order they appear, a row says who accepted a proposal, and items have words for `unset-token-when` and axis operations.

---

An accepted proposal cannot be redone, and a new server run turns cell outlines off for the rest of the session. The single-write attribution path and the recompute-on-accept path hold at HEAD. `be00071d7` and `ca47c46f2` did not touch either bug.

## High

**1. Redo of an accepted proposal restores nothing and drops the step.** `apps/design-tool/app.ts:798` (the map is replaced at `app.ts:703`).

`acceptHeld` reads `heldOps` only after `accept` returns. The server deletes the proposal before the write (`scripts/tamber-server.mjs:491`) and answers the waiting long poll from inside that write (`scripts/tamber-server.mjs:429` and `:532`, then `:336`), before it writes the `accept` response. `waitChanges` installs the new list, which no longer contains the proposal, and the undo entry is stored with `ops: []`.

Undo still applies the inverse, so the file does return. Redo sends the empty list (`app.ts:670`). An empty transaction writes no file and returns a null inverse (`contract/cli/src/tamber_serve.rs:804`). `redoLast` then pops the redo entry and pushes nothing (`app.ts:676`), and the button reports success.

Scenario: accept `agent:codex`'s proposal, press Undo, press Redo. The file stays undone, and both stack counts are 0.

Fix: copy that proposal's ops into a local before the `accept` call, and push the copy with the inverse. Drive accept, undo, and redo against a real server and compare the file bytes.

**2. A new server epoch disables cell highlights for the rest of the session.** `apps/design-tool/app.contract:287` and `:432`; `apps/design-tool/app.ts:730`.

`faded` is the last outlined sequence number. `loadLit` keeps a token only when `n.seq > faded`. A new run starts its sequence at 1 and clears the changeset (`app.ts:715`). It leaves `faded` where it was.

Scenario: a highlight fades at seq 12, then the design server restarts (the tool already tells you undo starts again). An agent writes. Changes names them, with the colour dot. The grid never outlines the cells, because seq 1 is not greater than 12. A page reload resets `faded`. Reconnecting does not.

Fix: when `changes.epoch` changes, set `faded = 0` before the next `loadLit`.

## Medium

**3. Outlines follow the one digest that was read, and the fade mark can skip events that were never drawn.** `apps/design-tool/app.ts:730` and the returned `seq` at `:731`; the timer is `app.contract:302` and `:432`.

A row is outlined only when `reads` holds that event's exact after-digest. The seq handed to `fade` is `changeset[0].seq`, including a row that was not outlined.

Scenario A: a hand edit is published in the same poll as the write that follows it (one `announce` publishes the hand edit, then the write). The grid is read once, at the final digest. The write's tokens outline. The hand edit's `changed` is null and its intermediate digest was never stored, so those cells get no outline and no "changed by" line.

Scenario B: a read fails ("The design kept changing") while an older outline is still up. `design.answer` still changes, `loadLit` returns the unread row's seq together with the older names, and three seconds later `faded` moves to that seq. Try Again shows the new values with no outline.

Fix: walk newest first. Once a row's after-digest is in `reads`, include that row and every older unfaded row (latest client per token). Return the greatest seq actually included, and leave a newer unread row above the watermark. The comment at `app.ts:527` describes this as work `loadDesign` does; the decision is in `loadLit`.

**4. Accept does not drop its result when the run changes during the call.** `apps/design-tool/app.ts:791` and `:798`.

`writeEdit`, `undoLast`, and `redoLast` capture `session` and, after the await, return the restarted stacks without pushing (`app.ts:624`, `:640`, `:669`). `acceptHeld` calls `sameRun` only before the request. `be00071d7` added that pre-check here and the after-check on the other three writes.

Scenario: `accept` is in flight to server A. The poll observes server B, `forget` clears the stacks and bumps `session`, then A's reply arrives and pushes A's inverse onto B's stack. Undo on B says the inverse is gone. After finding 1 is fixed, Redo would write A's operations into B.

Fix: the same `session` check after the await, returning the restarted stacks and pushing nothing.

**5. The tests do not prove undo, redo, or fade.** `scripts/design-tool.test.mjs:513`; `apps/design-tool/app.test.contract:319`; `apps/design-tool/stub/tamber-stub.ts:56` and `:72`.

The real-server drive does prove the gate: one `tamber op` write shows up on the changeset and in `lit` within one poll, and a partial accept and a stale accept come back as a new proposal whose diff is the new one (`design-tool.test.mjs:524`). It never presses Undo or Redo, and it never advances the clock three seconds.

The contract test checks that the undo count is 1 and never presses Undo. The stub's `transaction` returns an inverse for every body, including `ops: []`, so a redo there would look successful. Its recomputed proposal is the old object with a new id, so it still carries the old diff and the old token list; the contract test reads neither. Drop is exercised only on the stub (`app.test.contract:320`).

Fix: on a real server, accept, undo, redo, and compare the file; `clock +3000` and expect `lit.names` empty; restart, write again, and expect an outline. On the stub, give the recomputed proposal its own diff and tokens.

## Low

**6. Shared colours, `acceptedBy` dropped, axis items as JSON.** `apps/design-tool/app.ts:446`, `:470`, and `:454`.

`tintOf` hashes into eight colours. `agent:claude` and `agent:cursor` both land on orange, so their outlines match; the changeset text still has the name. An accepted write's event carries `acceptedBy` (`scripts/tamber-server.mjs:536`). `noted` keeps the proposer's `client` and drops `acceptedBy`, so Changes cannot say who accepted it. `opText` has no arm for `unset-token-when` or the axis operations a revert proposes, so those checkboxes render as JSON. The diff above them is still the proposal's diff.

Fix: assign colours in order of first appearance in the session. Show `acceptedBy` on the row when it differs from `client`. Add `opText` arms for the remaining operations.

Verdict: request changes — redo of an accepted proposal silently drops the edit, and a restarted server leaves cell highlights off for the rest of the session.
