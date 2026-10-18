# Code review: LLP 1122 T2 step 3 (scrubs), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `b41b0ff40` (commit `37076e92b` and its round-1 fixes).
- **Method:** one brief (sha256 `7679bb54928fa93f13d3f139201b8a8777623abb81ef4ef30c85afba5a683748`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium, 2 low).
- **Disposition:** taken but 1 (astra) and 8 (astra), 6 (grok) in part: Cancel now answers only when the server has let the scrub go (an opening still in flight is answered, then cancelled), and a new scrub opens only after any closing one, so it never joins the scrub being cancelled (Scrub live stays off meanwhile); a gap's end does the same; an opening refused says why ("No scrub: <the server's reason>"); Keep and Cancel use the session's own client, and changing the name cancels a scrub first; an end the server announces before the opening's answer is kept by id, and the late answer is taken as ended; a slider let go before its last frame is answered keeps that value once the server previews it (Keep drains the sender, and refuses if the last value is still not previewed); a refusal belongs to its value, so the draft back on a previewed value clears it; an empty draft says it is not previewed, and Keep stays off; the real drive checks the kept design on the frame (the same build, no reload); the stub test now cancels during an opening and shows Scrub live waiting for it, and a refusal clearing. Not taken: two tabs as the same person are one client by the RFC's naming (`design-tool:<person>`), and the server keeps one scrub per client; the stub's planner and writes stay canned (no digests, no design contents), and its presence never lapses.

---

The scrub's happy path matches the server: one session, one frame in flight, Keep writes the previewed operation as one undo step, and a rebuild is blocked before the gesture. A second scrub started before the previous server session is gone does not.

## High

**1. A new scrub joins the server session the previous one has not finished cancelling, and the late cancel ends it.** `apps/design-tool/app.ts:411-414`, `apps/design-tool/app.ts:912-919`, `scripts/tamber-scrub.mjs:89-95`, `apps/design-tool/app.ts:1045`

Cancel while the opening frame is unanswered returns immediately, because `s.id` is still empty (`app.ts:919`). The pump later sends `scrub-cancel` for whatever id that response carries (`app.ts:413`). Scrub live is enabled again as soon as that mutation settles (`app.contract:598-601` only waits on `pending(written)`). The server keeps a single scrub: an opening `scrub` that omits the id is an update of the one still open (`tamber-scrub.mjs:89-95`), and it keeps the first session's digests.

Press Scrub live, press Cancel before the preview returns, and press Scrub live again while that compile is still running. The second open attaches to the first server scrub. The first response's cancel then ends it. If the second frame has already been answered, its id matches, the `cancelled` event is ignored (`app.ts:1045`), and the panel stays on "Previewing" while the builds show the committed design. Keep then 409s and the local session is already cleared. If the cancel wins first, the second open fails and `endScrub(s, '')` (`app.ts:416`) drops the refusal, so the click does nothing and says nothing.

The same fence is missing when a gap ends a scrub whose id is known: the cancel is `void call` (`app.ts:1022-1025`), and Scrub live does not wait for it.

Hold the next open until the previous server scrub is confirmed gone. `cancelScrub` should await the in-flight open and then await `scrub-cancel` before it returns, and `scrubFrame` should await that same close before it sends an opening `scrub`.

## Medium

**2. A refusal of the opening frame is discarded.** `apps/design-tool/app.ts:416`

On an error before an id exists, the pump sets `s.error` and then `endScrub(s, '')`. `scrubFrame` is still awaiting that pump, so it returns `scrubState` with an empty error and an empty `ended`. The note under the proposal is blank.

The button hides the common cases (no dev loop, a rebuild chip). What still gets through is a 409 the chip did not show: the client is not present yet, another client's scrub is open, the opening digests are stale, or the server re-plans the edit as rebuild. The server drops the session (`failed()` in `tamber-scrub.mjs:57-58`). The tool shows Apply again and no reason.

Pass the refusal text into `endScrub` (the same string the panel already shows for a review that is missing) so `scrub-note` carries it.

**3. Keep and Cancel name the view's current client, and changing the name leaves the server scrub up with no panel.** `apps/design-tool/app.ts:904`, `apps/design-tool/app.ts:920`, `apps/design-tool/app.contract:505-509`

Frames and the gap-cancel use `s.client`, the name captured when the scrub opened. `scrub-release` and `scrub-cancel` send the `client` argument from the view, which is `design-tool:${person}`. The server accepts a release or cancel only for the client that opened it (`tamber-scrub.mjs:119-123` and `:140-143`).

Open a scrub, press the name in the header. `changeName` sets `renaming` and the contract replaces the editor with the name form (`app.contract:737`), so Keep and Cancel unmount and nothing sends `scrub-cancel`. The builds keep the preview. Save a new name and press Keep: the tool has already set `scrubbing = null` (`app.ts:900`), the server answers "no such scrub open for this client" and leaves the session open, and the copies stay on the preview until the old client is absent (35 s) or the scrub idles.

Send `s.client` on release and cancel. Cancel the open scrub in `changeName` before the editor unmounts.

**4. The tests that claim the cancel-during-open and the post-Keep frame do not establish them.** `apps/design-tool/app.test.contract:654-666`, `scripts/design-tool.test.mjs:868-873`

The stub test's comment says the cancel happens before the server answers. `clock data` on line 661 waits until the scrub resource settles, which is after the 300 ms preview has returned and the deferred cancel has been fired. The next Scrub live runs after that, with `slow-preview` set back to 0. A restart that is still inside the opening round trip is never sent. The real-server drive never cancels during an open either.

After Keep, `look()` stores the frame and the dev-loop snapshot in `kept`, and the assertions use the file bytes and `changes.undo` only. A reload on release, or a frame left on the pre-Keep design, still passes. `during` and `after` do check the build id and `page connected`.

Start the second scrub while the first open is still unanswered (hold the stub preview, and do not `clock data` past it). Assert `kept.frame.id`, `kept.connects`, and `kept.frame.design` against the snapshot that contains the kept value.

## Low

**5. Clearing the draft sticks the panel on "sending" while the copies show the last preview.** `apps/design-tool/app.ts:876-890`, `apps/design-tool/app.contract:952-953`

`scrubbable` requires a non-empty value. Delete the token text and leave the field. `op` is null, so no frame is sent, `want` stays on the last preview, and `ready` is false because `shown` is compared with null. The label is the not-ready string, "sending this value to the running copies…", and nothing is in flight. Keep stays disabled. Cancel still works.

Send the empty edit, or say that the value is empty and a frame is not going out, and keep the ready/sending text tied to `s.sending`.

**6. The stub's session machine is the server's; its planning, restore, and presence are not.** `apps/design-tool/stub/tamber-stub.ts:42-50`, `apps/design-tool/stub/tamber-stub.ts:169`

`scrubSessions` is the real one, so coalesce, idle, compete, and a refused release's `by` behave. `ask` and `write` ignore the opening digests, `restore` is a no-op, and `present` only grows, so a client going absent never ends a scrub. The server drops it (`scripts/tamber-server.mjs:208`, `scripts/tamber-scrub.mjs:148`). A wrong digest or a preview left on the builds passes the stub tests.

Verdict: request changes — one high (a second scrub started before the previous server session is cancelled is joined to it and then killed, or left showing "Previewing" over the committed design).
