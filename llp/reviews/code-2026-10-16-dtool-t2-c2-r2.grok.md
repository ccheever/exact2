# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `dae314424` (commit `39d166329` and its round-1 fixes).
- **Method:** one brief (sha256 `795d8e78afa42e9260eeb86e7ed80c4a28415b2618ff2e452aab959145b70780`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 3 medium, 2 low).
- **Disposition:** taken but 6 (grok) in part: retyping the name keeps the tool's client (it polls as the saved name until the new one is saved, and says a new name is a new client whose pins are handed back); a poll a newer one replaced changes nothing when it answers, and a failed hello read during a resync keeps the list and is read again each poll until one succeeds; a build's last refusal is now on its server record (`unsupported`, tamber/1.3, scripts/tamber-builds.mjs): the pin's that the build reports next, kept while its status stays at that revision and gone once it moves on, so a delayed refusal is not pinned on a later pin, and hello and both build events carry it; the pin form's modes are the app's own design's axes whichever file is open, and the per-row preview is gone: Preview in the form sends one axis to one build, enabled only for exactly that; a root font size outside 4-64 is not sent and keeps Pin off; the stub replaces build records instead of editing them, and keeps a refusal on its record; the real drive keeps the tool open over 36 s after the pin with no hand-back, and the hand-back after it closes lands between 10 and 45 s. 6 in part: a hand-back to a build that has not reported since it reloaded shows "Not reported yet" rather than "handing back" (its record cannot tell a hand-back from no pin).

---

The round-1 list and catch-up holes are closed on the success path. Two presence and resync failures still drop real pins or the whole build list, and a few lag and test gaps remain.

## High

**1. Editing the name hands every pin back while the tool is still open.** `apps/design-tool/app.contract:378`, `apps/design-tool/app.contract:470`, `apps/design-tool/app.ts:896`, `scripts/tamber-server.mjs:200`.

The long poll and `pin` both use `` `design-tool:${person}` ``. `changeName` sets `person` to `""` before a new name is saved, so the next poll is `design-tool:` and the client that owns the pins is no longer present. The server clears that client's pins about 35 seconds later. No unload is sent, and the page is still running.

Click the name in the header, retype the same name, and continue. Within about 35 seconds every build that this tool pinned is handed back, including one whose name did not change.

Keep polling as the current person while the name field is only a draft. On save, if the string changed, send the current pins again as the new client before the old presence lapses.

**2. One failed `hello` during a resync empties the build list for the rest of the session.** `apps/design-tool/app.ts:924`, `apps/design-tool/app.ts:939`.

The first poll, a gap, and a new epoch all set `anew` and then `takeBuilds` from `hello`. That `hello` uses the 15-second `call` timeout, not the poll's 40 seconds. On failure the code sets `buildsNow` to an empty map and still advances `place` to the new sequence and epoch. The next poll matches that epoch and is not a gap, so `anew` is false and `hello` is never read again. Build rows only change when a later `builds` or `build-status` event arrives.

Open the tool against a dev loop that already has a phone connected, while `hello` is slow or fails once (a cold design server, a restart between the poll and `hello`). The events poll succeeds. The panel says "No build is connected" and stays there even after `hello` would succeed, until some build connects, drops, or reports again.

On failure, keep the previous list and retry `hello` on the next poll. Do not treat a failed read as an empty server list.

## Medium

**3. A refusal is attached to whatever pin is current when the event arrives, and a missed log drops it entirely.** `apps/design-tool/app.ts:935`, `scripts/tamber-server.mjs:399`.

`unsupported` carries no revision. The client stamps it with `pinRevision` at arrival. `pin` announces `builds` before the host reports, so a second pin's snapshot often lands first. A missed poll sends no events at all (`events: []`), and `hello` has no last refusal, so a reload after the log has fallen off the start never shows one.

Pin contrast on the web build (it answers `pin.contrast`), then pin scheme before that report is applied. The scheme row says "Not applied: pin.contrast" even though the scheme pin has no contrast. Reload the tool after the server has kept more than 256 events: the contrast pin is still there and the refusal line is gone.

Put the last refusal on the build record next to `pinRevision` (hello and both build events), and ignore an `unsupported` event whose pin is no longer that revision.

**4. Preview choices are a stale snapshot, and the pin form follows the open file.** `apps/design-tool/app.ts:763`, `apps/design-tool/app.ts:462`, `apps/design-tool/app.contract:713`, `apps/design-tool/app.contract:736`.

`appAxes` is replaced inside `loadDesign`, including when that read is already overtaken (`number !== issued` still assigns it). Each build row stores that array only when `waitChanges` returns. The pin dropdowns read `design.axes`, which is the file on screen, not the app the builds are running.

Open the tool and let the first poll return before the design read. The rows have no "Preview only…" control until the next poll, up to 25 seconds, while the pin form already lists the axes. Open a second design file: the pin form offers that file's axes, and a preview can still show the previous file's axes after a stale read.

Drive both controls from the app design's axes in the view, and assign `appAxes` only for the read that is still current.

**5. The real drive does not show that a pin survives one presence lease.** `scripts/design-tool.test.mjs:715`, `scripts/design-tool.test.mjs:753`.

The "open past one lease (35 s)" stretch is four `clock +2000 real` gaps, 8 seconds, inside about 18 seconds of real clocks for the whole drive. `states[2]` only checks that the pin is still listed. That passes at 10 seconds. `clock data` sometimes blocks on a 25-second idle poll and sometimes returns immediately when a build report wakes it, so the wait is not a lease. The hand-back after close is a real 10–50 second bound. The "holds while open" half is not.

After the pin is caught up, wait until the wall clock has advanced more than 35 seconds with the page still open, then assert the pin is still on the build. Keep the existing bound for the hand-back after close.

## Low

**6. A hand-back is invisible while the build has not reported.** `apps/design-tool/app.contract:706`.

"handing back" is rendered only when `reported` is true. A reload sets `status` to null until the new document reports, so a clear in that window shows only "Not reported yet".

Reload a pinned build and press Hand back before its next status. The row never says the clear is in flight. When the status arrives already caught up, it looks as if the build was never pinned.

Show "handing back" whenever the pin is null and `status.revision` has not reached `pinRevision`, including when `status` is null.

**7. Root font sizes outside 4–64 are sent.** `apps/design-tool/app.contract:741`, `apps/design-tool/app.ts:1178`.

The field's `min` and `max` do not stop `choosePinFont`. Any `pinFont` other than 0 enables Pin, and `sendPin` forwards every value greater than 0. The server then refuses.

Type 3, or 80, and pin it. The round trip fails with `` `rootFontSize` is px, 4 to 64 ``.

Leave `rootFontSize` out unless it is between 4 and 64, and keep Pin disabled for those values.

Verdict: Request changes — a rename drops live pins, and a failed resync hello leaves the build list empty.
