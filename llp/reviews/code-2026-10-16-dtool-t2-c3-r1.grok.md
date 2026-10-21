# Code review: LLP 1122 T2 step 3 (scrubs), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `37076e92b`.
- **Method:** one brief (sha256 `b36065a14aff3433e49444f4f7cc2e917341e1ad2c286940ef5aa725f1a8c854`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high, 4 medium, 2 low).
- **Disposition:** taken, but 6 in part: the scrub is now one session in the data module from Scrub live to Keep, Cancel or an end, bound to its edit (file, kind, target, branch), its opening digests and server run: one frame is in flight at a time, always the latest wanted value; an answer for a session that has ended changes nothing, and a scrub the server opened for one cancelled while it opened is cancelled; Keep writes what the server previews (`ready` only when that is the draft as it is now) and its undo entry is that op; choosing another cell or file, adding, renaming or removing a token while one is open cancels it first, and a frame for another edit is refused with Keep or Cancel offered; an end the server makes is the session's own (no stale end can stop the next one); a new server run or a gap in the events ends it (cancelled if the server still has it) and says so, and a new one is chosen again; the panel shows while the opening frame is on its way, so Cancel is there; a slider sends each value while it is dragged and, let go, keeps a scrub on it once that value is previewed (else Keep does); the stub runs the server's own scrub sessions (scripts/tamber-scrub.mjs) with canned planning and writes, a preview delay, presence, and competing writes; the real drive now looks before the scrub, during each scrub, after Keep and after Cancel: the frame's build reports running the design the loop serves (the scrub's, with its value, then the committed one again), the same build and no page connecting until the rebuild edit applies, the file untouched until Keep writes one line, Undo restores the bytes, and the rebuild edit says why it has no scrub; the stub tests cover a stale end not stopping the next scrub, leaving the cell, Cancel while opening, a gap and a new server run. 6 in part: the scrub opens on Scrub live, not on pressing a control (a control press has no event to the tool); a stepper or text field still needs Keep.

---

The scrub controls and the server session do not stay in step. A server end, a late frame, or a second Scrub live can leave a preview running with no Keep or Cancel, or show "Previewing" when the server has already closed it.

## High

**1. A frame's answer updates `scrubbed` with no generation check, so a late answer resurrects a dead scrub or swallows the server's end.** `apps/design-tool/app.ts:842-851`, `apps/design-tool/app.ts:984-992`

`scrubbed` is set only after the round trip, and only the opening call's captured `opening` flag writes it. `{ended}` clears it unconditionally. The poll records a server end only while `scrubbed` still matches.

- Cancel or Keep during the first preview nulls `scrubbed`, then the opening response lands and sets it again (`opening && !scrubbed`). The server scrub is already gone. The next Scrub live sends `{scrub: <dead id>}` and no `files`. The server answers 409 (`tamber-scrub.mjs:90`). `scrubFrame` leaves `scrubbed` in place on error (`app.ts:846`), so every later move 409s until Cancel.
- A competing write or idle while a frame is waiting answers that frame `{scrub, ended: reason}` (`tamber-scrub.mjs:46`) and emits the `scrub` event. If the HTTP answer is applied first, `scrubbed` is null and the event is ignored. `scrubEndKey` stays empty, the task never runs, and `scrubbing` stays true with `open: ""`: Keep disabled, no "Previewing", no reason. The next value change opens a new scrub by itself, because `scrubbed` is null and `scrubbing` is still true.

Fix: bump a session generation synchronously before the first `await`, ignore any answer whose scrub id or generation is not current, and on `{ended}` write `SCRUB_ENDS[ok.ended]` into the note and clear `scrubbing` there. Do not depend on the event arriving while `scrubbed` is still set.

**2. After a server end, Scrub live is aborted about 1 ms later and the new server scrub is left open.** `apps/design-tool/app.contract:464-467`, `apps/design-tool/app.contract:559-564`, `apps/design-tool/app.ts:850`, `apps/design-tool/app.ts:989-991`

`changes.scrubEndKey` is whatever the last poll returned. `startScrub` does not clear it. Clearing the module field inside `scrubFrame` is invisible until a later poll. The gate is `scrubbing && changes.scrubEndKey != ""`, and a gate that turns true arms `after(1)`.

Scenario: idle or a competing write sets `scrubEndKey` to `scrub-1:idle` and the task sets `scrubbing` false. The user presses Scrub live. `scrubbing` becomes true while the poll still holds the old key, so the task fires again, puts the old "scrub ended" text back, and sets `scrubbing` false. It does not send `scrub-cancel`. The open that is already in flight still completes, sets `scrubbed`, and the builds keep that preview until the 30 s idle. The tool says the scrub ended and shows no Keep or Cancel.

The stub test never moves the clock between that Scrub live and Keep (`app.test.contract:610-614`), so the timer does not fire and the test stays green.

Fix: clear the end key in `startScrub` before `scrubbing = true`, and keep that key in state the action writes, not only in the next poll. `scrubEndedElsewhere` should run only for an end that happened while this session was the one open.

**3. Frames are in flight together, and Keep's undo entry is the draft, not the ops the server writes.** `apps/design-tool/app.ts:835-868`, `apps/design-tool/app.contract:421-426`

The scrub resource starts a new `scrub` POST on every `editValue` and again when `proposal.review` changes. Nothing waits for the previous POST. The server applies arrivals in arrival order and writes `open.ops` — the last preview that finished (`tamber-scrub.mjs:124-132`) — not the body of `scrub-release`. The undo push uses `opOf(...)` from the draft at Keep time (`app.ts:861-867`).

Scenario: a stepper dial (`input=`, so every change is a frame, `app.contract:1131`) or two token commits before the first compile returns. The later value's request is previewed, then the earlier request arrives and replaces it. Keep is enabled because the latest resource call has settled. The file gets the older value. The undo entry's `ops` and label are the newer one, so redo writes a different edit than the scrub wrote. The same split happens with an empty draft: `!value` returns the old id and sends no frame (`app.ts:839`), Keep stays enabled, the server writes the previous preview, and the undo op has `value: ""`.

Fix: one sender. Set the open id synchronously, send the next frame only after the previous response, and if the draft moved send only the latest. Keep records the ops of the last frame the server acknowledged, and it stays disabled until that frame is the draft.

**4. Leaving the edit mid-scrub drops Keep and Cancel but leaves the server session up. Rename or Remove records a different undo op than the write.** `apps/design-tool/app.contract:506-512`, `apps/design-tool/app.contract:541-569`, `apps/design-tool/app.contract:894-915`

`selectCell` sets `editKind = ""` and does not cancel. The scrub panel is under `if editKind != ""`, so it unmounts. `scrubbing` stays true and `scrubFrame` hits the early return, which keeps `open` but sends nothing. The builds keep the preview for up to 30 s with no "Previewing" and no Cancel.

Rename, Remove, and Add are still clickable. They change `editKind`. The server scrub still holds the `set-token` ops it last previewed. Keep's undo entry is the rename or remove (`releaseScrub` builds `op` from the current draft). Undo then redo applies that other op.

Fix: `selectCell`, rename, remove, add, and a dial change while `scrubbing` should take the same path as `cancelEdit` (send `scrub-cancel` and clear `scrubbing`) before the draft changes. Disable those controls while a scrub is open.

## Medium

**5. A new server run does not end the gesture, and the next proposal opens another scrub.** `apps/design-tool/app.ts:970`

An epoch change nulls `scrubbed` and does not set `scrubEnd`, does not clear `scrubbing`, and does not show "the design server stopped". The `server lost` event is on the old run; a mismatched epoch is delivered with `events: []` (`tamber-server.mjs:399-401`). `proposal` depends on `epoch`, so its new review retriggers `scrubFrame` while `scrubbing` is still true and `scrubbed` is null, which opens a scrub on the new run without Scrub live. A design-server crash inside the same epoch is fine: that `scrub` event is delivered and the task runs.

Fix: on an epoch change while `scrubbing`, set the server-lost note, set `scrubbing` false, and do not send `scrub` until the user chooses it again.

**6. A slider scrub does not send the drag.** `apps/design-tool/app.contract:1128-1131`

D6 is each value of the gesture previewed on the running copies. `input` on a range fires while the thumb moves; `change` fires on release (LLP 1069). The slider is bound with `change`, so after Scrub live a drag previews nothing until it is let go, then one frame. The stepper uses `input` and does the opposite (finding 3). The commit's "each value the draft takes" never happens during the drag, because the draft does not update until release.

Fix: while `scrubbing`, bind the slider's `input` to the frame and leave Keep as the write.

**7. The tests do not prove the claims in the commit message.** `scripts/design-tool.test.mjs:793-856`, `apps/design-tool/app.test.contract:551-616`

- "No reload" compares plan and `page connected` between two looks that are both already inside a scrub. A reload on the first preview is invisible. There is no snapshot from before Scrub live.
- "Offers no scrub" on the real loop only checks `proposal.chip == "rebuild"`. It never looks for `scrub-start` disabled or absent. The stub test does (`app.test.contract:634`).
- Cancel restored is `after.design` not containing `00ff00`. `look()` stores `""` when `/__dev/design` is not 200, and that also passes. It never checks the snapshot is the kept `#ff1100` line.
- "Each value previewing" on the stub types 210, then 215, then 220 and only checks the same Previewing string. It never reads the scrub's ops. Undo is the counter going to 1 and back to 0, not the bytes coming back.
- The server-end test does not `clock +1` before the next Scrub live, so finding 2 does not run.

**8. The stub does not behave like the server, so those tests cannot catch findings 1 and 3.** `apps/design-tool/stub/tamber-stub.ts:68-87`

Every `scrub` is an immediate success. The stub never answers `{superseded: true}` or `{ended}`, ignores `request.scrub` (a stale id still updates the open scrub; the server returns 409), does not require opening `files`, and does not end the scrub on `transaction` or `undo`. The server calls `compete()` before any write (`tamber-server.mjs:519-520`). `/__stub/end-scrub` only appends an event. It does not complete a waiter with `{ended}`.

## Low

**9. `keepScrub` does not repeat the button's `pending(scrub)` guard.** `apps/design-tool/app.contract:565-568` vs `912`

`apply` repeats its disabled conditions in the action. `keepScrub` does not. A Keep that runs while a frame is pending releases whatever the server has previewed, which may not be the draft yet. Put `not pending(scrub)` on the action.

**10. A `missed` poll drops the end.** `apps/design-tool/app.ts:984-992`

On a gap the server sends `events: []`. The fold never hears `idle`, `disconnected`, or `competing write`, and `scrubbed` is cleared only on an epoch change. `scrubbing` stays true and "Previewing" stays up over the committed design. Treat `missed` while `scrubbed` is set as an end: say so, clear `scrubbing`, and `scrub-cancel` if the server still has it.

Verdict: the happy path (one frame, Keep, Cancel, a rebuild chip) matches the server, but the session bookkeeping is wrong enough that a second scrub, a fast stepper, or leaving the cell mid-preview desynchronizes the tool from the builds.
