# Code review: LLP 1122 T2 step 2 (builds panel and pins), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `59d1da348` (commit `39d166329` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `327e34eb9d0ed5facc74afd97d81cf36bbd7c74e0eceafd39403c203f9d20764`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 3 low).
- **Disposition:** taken, but 1 (astra) and 2 (grok) in part, applied without re-review (round 3 was the last): an `unsupported` report may name its pin frame's `revision` (both web hosts now do), and the refusal is that frame's, kept while the status stays there, dropped when the status has passed it; a status older than the one kept is dropped, so a late report cannot rewind the row; a status naming a revision this registration was never sent (from before a restart, or queued) is kept with revision null and acknowledges nothing; a build with no pin is told so at revision 0, and a pin to no build sends nothing and keeps the revision, so a never-pinned build is never shown handing back; "handing back" shows only when the status names a revision older than the last frame; the pin form's modes while another file is open come from the app's own design read again as changes move (its own resource), not a copy taken on leaving it; the Build comment and the README say what the server does (the last frame from any client; about 40 s after the tool closes: the 35 s lease and the 5 s sweep). In part: Apple and Linux still report a refusal without its revision (the next status binds it, as before); the canvas host's refusal and status posts are not serialized; a pin draft naming an axis the app no longer has is not pruned (the server refuses it).

---

Round 3: the build list stays aligned with the server across `builds`, `build-status`, a reload, a new epoch, and a missed log, and the long poll names the same client the pins use. Two catch-up bugs remain. The round-2 fixes (the name stays the saved client while it is only a draft, a failed hello retries, `pinRevision` lives on the server record) are in place.

## Medium

**1. A build that was never pinned shows "handing back".** `apps/design-tool/app.contract:719`, `apps/design-tool/app.ts:463`, `scripts/tamber-builds.mjs:122`.

The row paints "handing back" whenever the pin text is empty, a status has arrived, and `Number(status.revision ?? -1) >= pinRevision` is false. A null revision becomes -1, so it sits behind every `pinRevision`, including 0.

The canvas host posts that status on purpose. On connect it calls `status()` while `ch.revision` is still null (`host/web-js/dev.mjs:259`); `rep` waits for register, then posts the body. The register-time frame `{pin: null, revision: 0}` is applied afterwards and posts a second status (`host/web-js/dev.mjs:258`). Those are separate `build-status` events, and the long poll wakes on the first, so the frame's own row is orange "handing back" until the second status lands. The T2 drive reads the row after two real seconds and expects `caughtUp: true` (`scripts/design-tool.test.mjs:744`), which is already past that window.

A register with no stored pin makes the same label last longer. `pinFrame` uses the global counter when the build has no pin (`scripts/tamber-builds.mjs:122`, stored at `:211`). After any pin or clear that counter is at least 1, so a device that connects later is sent a null pin at that number. `scripts/tamber-builds.test.mjs:77` registers both builds while the counter is still 0. Until the new build echoes the counter, its empty pin reads as a hand-back in progress. `pin()` also increments the counter when it matches no build (`:262`), and Hand back stays enabled on an empty panel (`app.contract:752`), so that press alone moves every later build's `pinRevision`.

Show "handing back" only when the pin is empty and the reported revision is a number below `pinRevision`. On register, when the build has no stored pin, send revision 0 and set `sent` to 0. Leave the counter unchanged when a pin matches no build.

**2. A refusal is stamped onto whichever status arrives next, and an older status replaces a newer one.** `scripts/tamber-builds.mjs:222`, `host/web-js/dev.mjs:258`.

The new stamp assumes the next status is the frame the host just refused: an `unsupported` report stores no revision (`:230`), and the following status copies its own revision onto that record (`:222`). A status at any other revision clears it. The same handler always replaces `b.status` (`:224`). `scripts/tamber-builds.test.mjs:114` sends the older status before the newer one, which is the order that rule needs.

The canvas host does not keep that order. A contrast pin fires `rep({kind:'unsupported', what:'pin.contrast'})` and `status()` without awaiting either, then the pin task ends and the next pin runs (`host/web-js/dev.mjs:258`). Pin contrast (revision 1), then scheme (revision 2), and let the posts complete as status 2, then `pin.contrast`, then status 1:

- Status 2 is stored with no refusal. The row is caught up on the scheme pin.
- The refusal arrives with no revision and the panel adds "Not applied: pin.contrast" to that scheme row.
- Status 1 stamps the refusal as revision 1's and overwrites the status. The row goes back to the old scheme, stays on "pinning" because 1 is behind `pinRevision` 2, and keeps a refusal that belongs to the pin the scheme pin replaced.

If status 2 is the only one that arrives after the refusal, the refusal is stamped as revision 2's and stays for as long as the build remains on that revision.

Ignore a status whose revision is null or below the numeric revision already stored, so a late report cannot rewind the row. Carry the frame revision on `unsupported` and keep the refusal only while the stored status is still that revision. On the canvas host, finish the unsupported post and the status post before the pin task resolves, so the next frame cannot interleave.

## Low

**3. Leaving the app file before its design has loaded leaves the pin form with no axes.** `apps/design-tool/app.contract:639`, `apps/design-tool/app.contract:759`.

`appAxes` is copied only inside `chooseFile`, and only when `file == ""` and `design.ok`. The file select is shown as soon as `hello.files` has two entries, including while the design read is still pending (`:764`). Open brand.tamber during "Reading the design…". The snapshot is skipped, `pinAxes` stays empty for the whole visit to that file, and scheme and contrast are the only facts that can be pinned. The copy has to happen when the app design first loads, as well as when leaving it.

**4. The Build comment names the wrong pin.** `apps/design-tool/app.contract:102`.

The comment says caught up with "the tool's last pin". `buildRows` compares against `pinRevision`, which is the last frame the server sent, including another client's pin and a hand-back (`docs/tamber-protocol.md:621`). Say that.

**5. The README's hand-back bound is shorter than the server's.** `apps/design-tool/README.md:32`, `scripts/tamber-server.mjs:199`.

The README says pins are handed back within 35 seconds of the tool closing. Presence lasts 35 seconds after the long poll is answered, and `absent` runs on a 5-second timer (`:209`), so a close can sit for about 40 seconds. The protocol's "answered in the last 35 seconds" is the lease; the README should include the sweep, which is the bound the T2 drive allows (`scripts/design-tool.test.mjs:765`, under 45 seconds).

Verdict: request changes (0 high, 2 medium, 3 low).
