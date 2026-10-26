# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `05d1f7159f`.
- **Method:** one brief per round (sha256 `10a024d48255ebd52518ce92259daf08afdcf94e8d8b4042157cea367fa2ce29`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 3 SHOULD-FIX (a handler's jump cleared by the bubble ack; the pending count no measure of delivery; the viewport's events at the document)
- **Disposition:** all taken in 9deb6cfc1b

- **SHOULD-FIX — [host/web/collection-glue.js:438](/tmp/iosfix-rv-ss1/host/web/collection-glue.js:438): A handler’s new scroll mark can be erased.** Authored listeners attach before the lazy controller. If a scroll handler synchronously jumps the same list, `move()` marks that new scroll owed, then `s.scrolled` clears it while handling the original event. An in-memory probe reproduced a 200→300 jump leaving `owedScrolls() === 0`. The wasm loop can then return before the next event. Acknowledge the current event before authored handlers run, preserving marks they create.

- **SHOULD-FIX — [host/web/collection-glue.js:623](/tmp/iosfix-rv-ss1/host/web/collection-glue.js:623): Pending-count reduction does not measure delivered events.** If each delivered scroll is followed by another correction, the count stays constant. The probe delivered eight events and returned `false` with one still owed. Both callers can consequently report settlement with work outstanding; wasm also skips the wait on its final round. Track event delivery separately from pending work, and report unsettled when the bound is exhausted.

- **SHOULD-FIX — [host/web/collection-glue.js:166](/tmp/iosfix-rv-ss1/host/web/collection-glue.js:166): Viewport scroll marks stick.** `portOf()` can return `document.scrollingElement`, but the clearing listener attaches to that element. Viewport scroll events target `Document`, so the mark survives delivery and every subsequent settle spends all eight frames, including lists without authored handlers. Attach and remove viewport listeners on `Document`. [CSSOM View](https://drafts.csswg.org/cssom-view/#scrolling-events)

The shared controller is the appropriate layer, and normal waits are batched across lists. Line caps pass. The added test covers one correction without an authored handler; it misses these cases and the agent integration.

Verification used read-only inspection and in-memory probes, not browser suites. No files changed.
