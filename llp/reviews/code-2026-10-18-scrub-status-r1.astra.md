# Code review: scrub frames' status deferred and transitions off through a drag, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `6cb6f9402`.
- **Method:** one brief (sha256 `e131df4bc7d2d761af4d0bdef2d220ca4302ccdcf4c239889058d8fdecf7c415`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the commit hook, the applied frame's mark, timers; the per-frame flush).
- **Disposition:** fixed in `9cc1fa38f` (see its message).

---

1. **should-fix — [host/web/dev.js:285](/private/tmp/rv-st-1/host/web/dev.js:285): the commit hook bypasses the debounce.** `x.send()` invokes `exact.committed()` through `glue.js:900`; its zero-delay callback at `dev.js:405` still reads `facts()` and reports each changed digest. An in-memory clock check produced reports at 0, 16 and 32 ms, plus a duplicate at 182 ms. **Fix:** route scrub-driven commit notifications through the same debounce while preserving immediate reporting for other changes.

2. **should-fix — [host/web-js/dev.mjs:695](/private/tmp/rv-st-1/host/web-js/dev.mjs:695): snapshot recovery uses the original frame’s scrub mark.** A stale scrub batch can fetch an unmarked snapshot containing a subsequent ordinary edit. That edit’s status is incorrectly deferred; its queued batch need not flush it because the digest already matches. **Fix:** classify the frame actually applied, including a fetched snapshot, using its own `scrub` mark.

3. **nit — [host/web/dev.js:236](/private/tmp/rv-st-1/host/web/dev.js:236), [host/web-js/dev.mjs:702](/private/tmp/rv-st-1/host/web-js/dev.mjs:702): pending timers survive generation changes and reload initiation.** Immediate pin reports also leave them pending: a pin at 60 ms is followed by another status at 150 ms. These report current facts, so the observed effect is redundant work. **Fix:** cancel and null the timer on immediate reports and generation/reload boundaries.

4. **nit, existing cost — [host/web/dev.js:301](/private/tmp/rv-st-1/host/web/dev.js:301), [host/web-js/modes.js:241](/private/tmp/rv-st-1/host/web-js/modes.js:241): every scrub frame still forces a synchronous style/layout flush.** Transition suppression toggles across the subtree, followed by `root.offsetWidth`, even for colour scrubs. **Fix:** keep suppression active across a burst and flush once before releasing it, preserving the no-transition behavior.
