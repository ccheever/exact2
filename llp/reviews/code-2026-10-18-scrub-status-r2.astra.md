# Code review: scrub frames' status deferred and transitions off through a drag, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `6cb6f9402`, `9cc1fa38f`.
- **Method:** one brief (sha256 `6a8425d76f02523979ee1c54c5c91cbd385797b36a1501add1597efa71b405d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the app's own commits, a generation mid-drag; a stylesheet mutation, same-value writes).
- **Disposition:** fixed in the next commit (see its message); the suppression sheet's one insertion status is left.

---

1. **Should-fix — [host/web/dev.js:415](/private/tmp/rv-st-2/host/web/dev.js:415).** `statusLater` suppresses **all** commit reports during a scrub. An app’s own `setScheme`, `setMode`, or `setRootFontSize` can therefore leave the tool’s facts stale throughout a continuous drag. Reproduced with a scheme change. **Fix:** suppress the commit hook specifically around the scrub’s `x.send(n)`; let unrelated commits report normally.

2. **Should-fix — [host/web/dev.js:308](/private/tmp/rv-st-2/host/web/dev.js:308).** Transition suppression survives a generation replacement. A generation applied 20 ms after a scrub frame still has `data-x-scrub` until the old 150 ms timer expires, suppressing transitions in the replacement app. **Fix:** clear the suppression timer and marker before presenting the new generation, alongside deferred-status cleanup. Reproduced through the generation client.

3. **Nit — [host/web-js/dev.mjs:695](/private/tmp/rv-st-2/host/web-js/dev.mjs:695), [697](/private/tmp/rv-st-2/host/web-js/dev.mjs:697), [700](/private/tmp/rv-st-2/host/web-js/dev.mjs:700).** JS still produces extra reports: inserting the suppression stylesheet on the first scrub triggers `moved()` immediately; an immediate pin/media/inspection status also leaves `liveStatus` queued. The later report reads current facts, so this duplicates rather than rewinds status. **Fix:** ignore suppression-sheet mutations and clear/null `liveStatus` inside `status()`, preserving prompt reports for genuine app changes.

4. **Nit — [host/web/dev.js:305](/private/tmp/rv-st-2/host/web/dev.js:305), [host/web-js/modes.js:243](/private/tmp/rv-st-2/host/web-js/modes.js:243).** Both helpers still set the root attribute every frame. Same-value writes generate [attribute mutation notifications](https://dom.spec.whatwg.org/#concept-element-attributes-change); while press feedback is active, [its root observer](/private/tmp/rv-st-2/host/web/input-glue.js:284) consequently invokes repaint work and computed-style reads. **Fix:** set the attribute only when absent, while continuing to reset the expiry timer.

Three targeted live-batch tests passed; timer scenarios were checked in memory. No files changed.
