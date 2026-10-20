# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `6929efc40`.
- **Method:** one brief (sha256 `0ab64d8ca20ba1279a6042b152c469ceed74b1354c75d0d2e3adee8bfc286764`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** in part; this was the last round. The single click and its own outline (the gate's case) hold, and any frame not on the stamped site keeps D4's scroll. Two cases are left, because closing them needs the outline frame to name the report it answers (a token through `selected`, the server's join and `outline`), which is a wire change for theme2 to decide:
  - another outline of the same site arriving while the stamp is live (a second click, or the tool's Outline of a token that node uses) takes the stamp first, so the click's own reply scrolls;
  - a click report that draws no outline (409 with no retry, no map) leaves the stamp for up to 10 s, and a tool Outline of that site in that window does not scroll.
  The worst outcome of either is the old behaviour (a scroll) or one missed scroll. The e2e still counts calls on the fixture's single copy.

---

- **Medium — [host/web/dev.js:637](/tmp/rv-dt-scroll-r3/host/web/dev.js:637): Site, plan and age still cannot identify the selection’s reply.** Click a later copy of A, then receive an already-pending tool Outline or refresh outline including A. That frame consumes the marker and skips D4’s first-copy scroll; the actual selection reply then scrolls. A refused report producing no outline also leaves the marker eligible for ten seconds, because `report()` ignores the outcome. An explicit Outline during that window can incorrectly skip scrolling. **Fix:** identify the originating page and selection request in automatic outline frames, suppress only that request’s reply, and retire requests on refusal or failure.

- **Medium — [host/web/dev.js:627](/tmp/rv-dt-scroll-r3/host/web/dev.js:627): Clearing the marker does not cancel its pending reply.** If `clear` arrives after the click report is queued but before its outline, it deletes `clicked`; the eventual selection outline then restores the marks and scrolls the first copy. Expiry has the same problem: the ten-second age starts before the serialized report queue, whose registration wait is unbounded and whose individual POSTs each allow ten seconds. A valid reply can therefore arrive after suppression expires. **Fix:** make cancellation invalidate identifiable pending replies and ignore them; a still-valid pointer-selection reply should retain its no-scroll policy regardless of transport delay.

- **Low — [scripts/tamber-server.test.mjs:482](/tmp/rv-dt-scroll-r3/scripts/tamber-server.test.mjs:482): The test proves only the serial call-count case.** Counting calls catches the old unconditional scroll, but the test selects one visible copy, waits for its outline, then outlines different sites. Even failing to consume the marker could pass that sequence. Later clear, refresh and reload operations have no scrolling assertions. **Fix:** use repeated copies with the first offscreen, control reply ordering, and assert both scroll-container offsets and the tool’s first-copy reveal. Cover refusal, clear, delayed replies, reload and unmount.

The resolved paths hold by inspection: only successful pointer selections arm the marker; refresh and unsupported reports do not overwrite it. Different-site, empty-site and foreign-plan frames preserve it. Unmounting the clicked view no longer prevents suppression. Leaving inspect mode clears it, and a full document reload starts without it. An in-place reload, however, preserves it through `outlineNone()` at [dev.js:433](/tmp/rv-dt-scroll-r3/host/web/dev.js:433); a generation retaining the same plan remains exposed to the first finding.

Tests were not run under the read-only restriction.

Verdict: **Request changes—reply identification and cancellation still permit selection jumps and suppress required tool scrolling.**