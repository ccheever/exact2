# Code review: LLP 1122 T1 commit 4 (editing and undo), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `be00071d7` (commit `d3e6ad575` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `57a1a2022b292a9d8308e94aac1413dfd89fdcd0d6748a899ca3b6914a6b24e9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** as in the astra record: the high, the draft and the accept findings taken; the wire's 504 on a write that later lands is not (round 3 was the last).

---

The gate tests hold: a dial edit and a Dim-cell edit are each a one-line diff, undo restores the bytes (including the unformatted `text.primary` line), a hand edit before the second undo says `app.tamber was changed on disk`, a pin of `accent.fill` is applied and undone, and "only this mode" is written, redone, and undone. Round 1 and round 2's branch selection, carried review, cancel, file switch, input detection, and the stuck-inverse block are fixed in the code those tests drive. Three problems remain.

## High

**Redo still writes this session's operation into whichever server now holds port 7740.** `apps/design-tool/app.ts:649` (`redoLast`), `app.ts:394` (`sameRun`), `app.ts:388` (`forget`), `scripts/tamber-server.mjs:337`.

`redoLast` copies the stack entry, then asks `sameRun` whether this is still the same run. `sameRun` compares hello's epoch with the global `minted`, and `forget` sets `minted` to the new epoch while that hello is in flight. The session generation is read after `sameRun` returns (`app.ts:656`), so it already equals the generation `forget` just bumped, and the transaction is sent. A foreign epoch is answered immediately by the wire (`tamber-server.mjs:337`); hello waits on the Rust process, so the poll wins whenever Redo is clicked in that window. The Redo button stays enabled until that poll's reply updates `changes.session` (`app.contract:296`).

Undo in app A, stop A, start app B on 7740. The events poll gets 404, drops the token, and shares one token request with anyone else (`app.ts:36`). Click Redo while that request is in flight, or after the new token is in hand and before the poll applies the new epoch. The poll's continuation runs first, `forget` adopts B's epoch and clears the arrays, hello then sees `epoch === minted`, and Redo sends A's operations with B's current digests (`app.ts:657`). B applies them when the operation is valid there, and the new inverse is pushed onto the new session's undo stack.

`writeEdit` refuses a review that still carries the old epoch (`app.ts:609`). That check misses a review whose epoch was copied from `place.epoch` after `propose` returned (`app.ts:600`), and it skips a review whose epoch is still empty (`read.epoch &&`). Either one sends the old digests to the new server when the bytes match.

**Fix:** stamp the epoch and the session generation on the stack entry and on the review before any await. `sameRun` should refuse when hello's epoch differs from that captured epoch, including when `minted` was updated during the hello. Refuse before calling `transaction`. Stamp `review.epoch` from the server that produced the digests, and treat a missing epoch as unbound.

The real-server tests never swap the process on 7740, so they do not reach this path.

## Medium

**An armed draft is applied to a branch the heading no longer shows.** `apps/design-tool/app.contract:278`, `app.contract:359`, `app.contract:546`; `tamber/src/edit.rs:579`.

The pending-branch window from round 2 is closed: the panel stays hidden until `branch.answer` matches the grid and the branch read is finished (`app.contract:544`). A draft typed before the other client's edit survives. `proposal` is re-run because `since` and `epoch` are its arguments, while `editKind`, `editWhen`, and `editValue` stay. The heading then shows the branch the cell shows now. Apply still sends the old `editWhen`. `set-token` of a `when` that is gone inserts that line (`edit.rs:579`).

Type a new colour on `surface.card`'s Dim cell. Another client deletes that `when`. After the poll, the heading names the default (or whatever branch now wins) and the field shows that branch's value. Apply, without typing again, recreates `when dark-style=dim` with the colour that was typed. That is a new `when` without "Only this mode".

**Fix:** clear `editKind`, `scopeOwn`, and the pin when `design.answer` or the shown branch's `when` changes, or keep Apply disabled unless `editWhen` is still the branch the heading names.

## Medium

**A write the wire gives up on can still land, and the undo that landed cannot be redone.** `apps/design-tool/app.ts:18`, `app.ts:636`; `scripts/tamber-server.mjs:73`, `scripts/tamber-server.mjs:213`.

`WRITE_WAIT` is 130s so the tool will outlast the server. The wire answers 504 `"the design server did not answer"` at 120s (`ANSWER_MS`) and leaves the request in `waiting`. When Rust later replies, `done` still publishes the write. `undoLast` treats that 504 as a failure to retry, so the entry stays. The server has already removed the inverse (`tamber_serve.rs:762`). The next press matches `/^no inverse/`, pops the entry, and does not push the original operations onto redo (`app.ts:636`).

Undo a large edit. The Rust reply takes more than two minutes. The tool says the server did not answer, the file is already restored, and the next Undo says the inverse is gone and sets it aside. Redo never appears. The same 504 on Apply leaves the edit in the file with no undo entry.

**Fix:** keep the HTTP request open until Rust replies, so a 504 is not sent for a write that can still commit. When an undo's reply was lost and a fresh read shows the postimage already gone, move that entry to redo.

## Low

**Accept can record an inverse on the run that replaced the one that issued it.** `apps/design-tool/app.ts:760`.

`acceptHeld` checks `sameRun` and then, with no session generation of its own, pushes `ok.inverse` onto the current stack. A proposal id embeds the wire epoch, so an accept sent to the new server is a 404 and writes nothing. An accept that succeeded on the old server, whose reply is applied after `forget` has already cleared the stacks, pushes that inverse onto the new run. The next Undo says the server no longer has it and sets it aside.

**Fix:** capture the session generation before the accept call, and drop the reply when it differs.

**Verdict: request changes.**
