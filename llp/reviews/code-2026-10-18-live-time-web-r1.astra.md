# Code review: a web live time's fallback children and its reserved box (conform native-fixture), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ebcf32a66`.
- **Method:** one brief (sha256 `cf07047bbb5412ee505bf80deb13e915417c122da67e22cb077621f9218e0780`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a blocker (an adopted JS page's time duplicates its formatted text) and two should-fix (the wasm reservation overrides an authored width; a deferred install can resurrect cleared text).
- **Disposition:** all taken in `9690220c5`.

There is one blocker.

1. **BLOCKER — Adopted times duplicate their formatted text.** [host/web-js/live-time.js:116](/tmp/rv-time-1/host/web-js/live-time.js:116)  
   A server-rendered `<time>` already has a formatted text node. Since `shown` starts null, initialization prepends another, producing `2 min. ago2 min. ago`. The CSS hides only element children. If the instant later becomes invalid, cleanup removes only the new node, leaving stale formatted text beside the fallback. **Fix:** adopt the existing formatted text node and use that same node for updates and removal.

2. **SHOULD-FIX — Wasm reservation overrides authored sizing.** [host/web/live-time-glue.js:36](/tmp/rv-time-1/host/web/live-time-glue.js:36)  
   A time with `width=40` or `max-width=40` still receives the full reservation as `min-inline-size`. A wider minimum overrides those bounds. The JS compiler explicitly disables reservation for width/max-width bindings, so the targets diverge. **Fix:** carry equivalent reservation eligibility into the wasm path and respect authored sizing through subsequent style writes.

3. **SHOULD-FIX — Deferred installation can resurrect cleared text.** [host/web/glue.js:433](/tmp/rv-time-1/host/web/glue.js:433), [host/web/live-time-glue.js:49](/tmp/rv-time-1/host/web/live-time-glue.js:49)  
   While loading, a text update with fallback children populates `exactTimeText`. If those children disappear, then the instant becomes invalid, the childless clear empties `textContent` without clearing the cache. Installation subsequently restores the old formatted string despite formatting being unavailable. **Fix:** update `exactTimeText` on every time-text set/clear, independently of `childElementCount`.

Ordinary fallback restoration and direct inline-run exclusion passed the in-memory checks. The observer’s equality guard converges after style replacement, and teardown disconnects both observers. Apple/Linux code is unchanged.

Read-only source review and in-memory reproductions only; no builds or file changes.
