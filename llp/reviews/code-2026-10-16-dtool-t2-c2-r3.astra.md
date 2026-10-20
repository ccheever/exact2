# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `59d1da348` (commit `39d166329` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `327e34eb9d0ed5facc74afd97d81cf36bbd7c74e0eceafd39403c203f9d20764`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken, but 1 (astra) and 2 (grok) in part, applied without re-review (round 3 was the last): an `unsupported` report may name its pin frame's `revision` (both web hosts now do), and the refusal is that frame's, kept while the status stays there, dropped when the status has passed it; a status older than the one kept is dropped, so a late report cannot rewind the row; a status naming a revision this registration was never sent (from before a restart, or queued) is kept with revision null and acknowledges nothing; a build with no pin is told so at revision 0, and a pin to no build sends nothing and keeps the revision, so a never-pinned build is never shown handing back; "handing back" shows only when the status names a revision older than the last frame; the pin form's modes while another file is open come from the app's own design read again as changes move (its own resource), not a copy taken on leaving it; the Build comment and the README say what the server does (the last frame from any client; about 40 s after the tool closes: the 35 s lease and the 5 s sweep). In part: Apple and Linux still report a refusal without its revision (the next status binds it, as before); the canvas host's refusal and status posts are not serialized; a pin draft naming an axis the app no longer has is not pruned (the server refuses it).

---

Read-only review completed; no files changed or tests run. Four findings remain:

1. **Medium — Refusals can still attach to a later pin.** [scripts/tamber-builds.mjs:222](/tmp/rv-dt-t2b-r3/scripts/tamber-builds.mjs:222)  
   The server assigns a refusal to whichever status arrives next. Apple coalesces statuses, and web hosts send reports concurrently. If revision 1 refuses a mode and revision 2 succeeds before status 1 is reported, the refusal becomes revision 2’s and survives its successful status. The new test explicitly supplies status 1, avoiding this case. **Fix:** carry the originating pin revision in refusal reports and events; test skipped and reordered statuses.

2. **Medium — A previous server’s revision can falsely acknowledge hand-back.** [apps/design-tool/app.ts:463](/tmp/rv-dt-t2b-r3/apps/design-tool/app.ts:463)  
   After a server restart, a surviving build can register with a new nonce while reporting its previous revision, say 7. Hosts retain that revision across reconnects; Linux can resend queued reports before fetching new commands. The new server sends `pin:null` at revision 0, so `7 >= 0` marks the build caught up before it processes the hand-back. Clearing the tool’s map does not prevent this fresh report. **Fix:** invalidate old acknowledgments on reconnect and qualify them by server/registration identity. Test a server restart with the build process still running.

3. **Medium — App-axis choices stop updating while another file is open.** [apps/design-tool/app.contract:639](/tmp/rv-dt-t2b-r3/apps/design-tool/app.contract:639)  
   `appAxes` is captured only when leaving the app’s design. Subsequent events refresh the selected file, leaving this cache unchanged. Open `brand.tamber`, then have another client add an app axis or remove a value: rebuilt builds use the new design, but the pin form retains obsolete choices indefinitely. **Fix:** refresh the app’s axes independently of the selected file, keyed by its digest and epoch, and remove invalid draft selections. Test an axis change while another file remains open.

4. **Low — The 35-second hand-back deadline remains unenforced.** [scripts/design-tool.test.mjs:765](/tmp/rv-dt-t2b-r3/scripts/design-tool.test.mjs:765)  
   The test accepts 45 seconds. The server’s 35-second lease plus five-second sweep can retain pins for almost 40 seconds after closure, contradicting the brief and README. Closing just after a sweep exposes this without unusual scheduling delays. **Fix:** expire at the lease deadline and assert 35 seconds with a small scheduling allowance.

Verdict: Request changes for refusal attribution, restart acknowledgments, stale axis choices, and the remaining deadline gap.
