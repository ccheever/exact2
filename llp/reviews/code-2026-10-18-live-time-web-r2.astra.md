# Code review: a web live time's fallback children and its reserved box (conform native-fixture), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9690220c5`.
- **Method:** one brief (sha256 `5d5c04a3ee1c075b9c6555aa83ad9174a56683d96a1b2093afe41df1d682ccd0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two blockers (the JS renderer's VM has no Node; region anchors before an adopted time's text) and three should-fix (adoption only when formatted; an authored min-width; a time nested under a run or link).
- **Disposition:** taken in `432fb21af` except the authored min-width, declined: the JS target writes min-inline-size the same way, so the targets agree.

Two blockers remain.

1. **BLOCKER — [host/web-js/live-time.js:77](/tmp/rv-time-2/host/web-js/live-time.js:77): JS server rendering throws.** The renderer’s VM has no global `Node`. This initializer raises `ReferenceError: Node is not defined`, including for interval progress before its special branch. Reproduced in memory. **Fix:** compare `nodeType` with `3`, as the runtime already does elsewhere, or supply the DOM constant.

2. **BLOCKER — [host/web-js/live-time.js:77](/tmp/rv-time-2/host/web-js/live-time.js:77): conditional fallback still duplicates an adopted timestamp.** When fallback children start with `if`/`for`, their adoption inserts comment anchors before the rendered text. `firstChild` is then a comment, so another timestamp is prepended. Reproduced `3 min. ago2 min. ago` after advancing the clock. **Fix:** identify and adopt the formatted node despite anchors, keeping it outside fallback regions so later region changes cannot remove it.

3. **SHOULD-FIX — [host/web-js/live-time.js:77](/tmp/rv-time-2/host/web-js/live-time.js:77): unformatted leading text is treated as owned formatted text.** For the requested case with bare fallback text first, an invalid instant deletes that fallback. A valid instant overwrites it but never sets `data-time-shown`, leaving fallback elements visible. **Fix:** qualify adoption with the formatted marker, preserve bare fallback text separately, and set the marker whenever formatting succeeds.

4. **SHOULD-FIX — [host/web/live-time-glue.js:38](/tmp/rv-time-2/host/web/live-time-glue.js:38): reservation can override an authored minimum.** With `min-width:300px` and no width/maximum, a smaller reservation writes a later `min-inline-size`, superseding that physical minimum in horizontal writing. **Fix:** preserve the authored minimum and combine it with the reservation rather than replacing it.

5. **SHOULD-FIX — [host/web/live-time-glue.js:26](/tmp/rv-time-2/host/web/live-time-glue.js:26): nested inline runs are misclassified.** In `text → text → time`, the immediate parent lacks `data-exact-text`; the WASM helper reserves the time while the JS compiler disables reservation. This changes sizing when its authored display makes the minimum effective. **Fix:** use explicit inline-run information or recognize the enclosing text-run chain.

Later width writes, deferred text set/clear, and the cache fix passed targeted in-memory checks. No files changed; no builds run.
