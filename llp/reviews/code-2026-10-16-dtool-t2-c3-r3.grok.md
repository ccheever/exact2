# Code review: LLP 1122 T2 step 3 (scrubs), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ac4952126` (commit `37076e92b` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `0a6be927740cc9cdc37b447158802a0d5e4ed34bd28613cc88420fcf306f9f8f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 2 medium, 1 low).
- **Disposition:** taken, applied without re-review (round 3 was the last): a release (Keep, or a slider let go) is only for the edit the scrub is of, and fixes its value when it is asked: no further frame goes out while it waits, and it writes only once that value is previewed; moving another dial or stepper while one is open cancels that scrub first; a slider let go while the opening frame is still on its way is kept once it is answered; every opening frame waits for a closing scrub, whichever route sent it; a Scrub live press not yet taken up is cancelled with Cancel; ended ids are this server run's only (cleared on a new run; the stub starts its ids again on restart), and an opening answered after its end is cancelled; a Cancel or Keep that did not reach the server keeps the session and says so, to retry (the server no longer having it is a definite end); a frame refused for a value the draft has moved past sends the next one, and one the server no longer has ends the session; "sending" shows only while a frame is in flight; the closing fence now clears itself when the cancel lands (it never did, which a test caught as a hang); the stub's digest after a scrub's write names the value written, and its test checks the kept value (220); new stub cases: another control moved mid-scrub, the first scrub of a new server run. Not tested: a slider let go mid-opening, and a transport failure on Keep or Cancel, in the stub (no hook for either).

---

The happy path holds: one session, one frame in flight, Keep writes the previewed operation as one undo step, Cancel waits out an opening, and a rebuild is blocked before the gesture. Three lifecycle holes remain.

## High

**1. A slider let-go writes whatever scrub is open, including a different edit.** `apps/design-tool/app.contract:520`, `apps/design-tool/app.contract:525`, `apps/design-tool/app.ts:929`

`dialNumber` runs on `input` and retargets the draft to that dial. `dialReleased` then runs on `change` and, because `editTarget` is now that dial, calls `releaseScrub`. `releaseScrub` updates `want` only when the draft is the scrub's edit, then releases anyway when `shown` still equals the previous `want`.

Scrub a token to `#ff0000` (previewing, Keep enabled). Drag the hue slider and let go. The token scrub is released and written; the undo entry is the token, not the dial. The same happens when the open scrub is a different dial. `n` is ignored, and nothing cancels the scrub the way choosing another cell does.

Refuse the release unless the draft's file, kind, target, and branch are the scrub's `of`. Do not retarget the draft from a slider while another edit's scrub is open; cancel that scrub first, as `selectCell` does. Pass `n` into the release.

**2. After a new server run, a new scrub is dropped locally while the server keeps it open.** `apps/design-tool/app.ts:425`, `apps/design-tool/app.ts:1051`, `apps/design-tool/app.ts:1078`

`endedIds` remembers this page's ended scrub ids and is never cleared. A new process numbers from `scrub-1` again (`scripts/tamber-scrub.mjs` `++count`). The epoch change ends the local session and does not clear the map. The next open's success is treated as an end, `endScrub` runs, and no `scrub-cancel` is sent.

Idle, a competing write, or a refused Keep records `scrub-1`. Restart the dev server and press Scrub live. The note says the scrub ended and the copies have the committed value back. The new server's scrub is open and the builds stay on the preview until idle (30 s). The stub hides this: its `scrubSessions` object survives `/__stub/restart`, so ids are not reused.

Clear `endedIds` on an epoch change. If a successful open is ignored because of a stale id, cancel that id.

## Medium

**3. A failed frame drops the value the user already moved to, and the panel says it is still sending.** `apps/design-tool/app.ts:422`, `apps/design-tool/app.contract:960`

On any error the pump returns. It does not send `want` when that has moved on, and it does not end the session if an id already exists. `ready` is false whenever `shown` is not the draft, and the not-ready copy is "sending this value…", even when `sending` is null.

Preview 10, drag to 20 (in flight), then to 30. The preview of 20 fails (builds return 502, or the edit is re-planned as rebuild). 30 is never sent. The thumb is at 30 and the copies still show 10, with "sending" and nothing in flight. Nothing calls `pump` again until the next poll or another input. If that is later than the server's 30 s idle, the scrub ends and 30 is lost.

On error, if `want` is no longer that op, clear the error and keep looping. If the server says the scrub ended, end the session even when it has an id. Tie the "sending" line to `s.sending`.

**4. Keep and a failed cancel drop the only handle to a server scrub that is still open.** `apps/design-tool/app.ts:933`, `apps/design-tool/app.ts:445`, `apps/design-tool/app.ts:949`

`releaseScrub` sets `scrubbing = null` before `scrub-release`. `closeScrub` ignores the cancel result, and `cancelScrub` always reports that the copies are back on the committed value.

Keep's POST fails before the server sees it (or the response is lost after a write). The panel is already gone, so there is no Cancel. The builds stay on the preview until idle. Scrub live again sends an opening `scrub` with no id, and the server treats that as an update of the scrub still open, on the old digests (`scripts/tamber-scrub.mjs` only stores `files` when `open` is null). A cancel that fails the same way shows "Cancelled" while the preview remains. A definite end (409, no such scrub) should still clear the session. A transport failure should keep the handle and say the call did not land.

## Low

**5. The tests that claim the multi-value keep and the cancel-during-open fence do not establish them.** `apps/design-tool/app.test.contract:563`, `apps/design-tool/app.test.contract:668`, `apps/design-tool/stub/tamber-stub.ts:42`

The stub keep types 210, then 215, then 220, and asserts the previewing string and `Undo (1)`. `write` stores a counter digest, not the operation, so a keep of 210 passes. The cancel-during-open test's second Scrub live is after `clock data`, so it starts only once the cancel has settled; `expect disabled "scrub-start"` is `pending(written)`, not a second open joined to the first. The stub also does not reset scrub ids on restart, so finding 2 cannot fail there. The real-server drive does check one kept value, one undo, and cancel restoring the frame.

Record the previewed and released ops in the stub and assert 220 and the undo bytes. Start the second scrub while the first cancel is still unanswered. Reset the stub's scrub counter on restart.

Verdict: request changes — a slider let-go can write a different edit's scrub, and the first scrub after a server restart can be dropped locally while the server keeps previewing it.
