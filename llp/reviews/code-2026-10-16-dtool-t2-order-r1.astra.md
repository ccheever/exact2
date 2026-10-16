# Code review: LLP 1122 T2, web dev hosts' report order, round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `af0124f48`.
- **Method:** one brief (sha256 `e0832acb95c341b018e23be60cef3f0764eaf6a19b22cda1db580e4d453b264d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium).
- **Disposition:** taken: a report is bound to the stream it was made on (its nonce and registration taken when it is queued) and goes on that stream's own queue, so a reconnect starts a fresh queue that nothing on the old one holds up, and a report from before it is dropped, not moved to the new registration; each post gives up after 10 s (the registration wait stays unbounded, as a slow page's first status must not be lost); the wasm page no longer holds its command chain on a reserved command's report. Grok's first run died after its preamble (no findings); it was run again for round 2.

---

1. **Medium — Queued reports can migrate to a replacement stream.** [host/web/dev.js:333](/tmp/rv-dt-t2d/host/web/dev.js:333), [host/web-js/dev.mjs:372](/tmp/rv-dt-t2d/host/web-js/dev.mjs:372). Both helpers now capture the nonce and registration when dequeuing, rather than when called. If report A is awaiting its response, report B queues, and the page reconnects before A finishes, B acquires the **new** nonce and passes the stale-stream guard. The server resets status/refusal state on registration, so old statuses or refusals can populate the replacement registration. This breaks the intended stale-report dropping behavior. **Fix:** capture the nonce and registration when enqueuing, then discard the item if either has changed.

2. **Medium — A hung request blocks reporting across reconnects.** [host/web/dev.js:332](/tmp/rv-dt-t2d/host/web/dev.js:332), [host/web-js/dev.mjs:372](/tmp/rv-dt-t2d/host/web-js/dev.mjs:372). The queue waits indefinitely for registration and each POST, and its tail survives nonce changes. For example, registration A remains pending while EventSource reconnects and registration B succeeds; B’s reports still wait behind A forever. The stale-stream check cannot help because it runs after that wait. A hanging ordinary report produces the same blockage. In wasm, a reserved command also awaits `report()` through `designing`, potentially blocking subsequent pins and batches. **Fix:** bound registration/report requests, cancel obsolete-stream work, and start an independent queue when the stream changes.

The existing pin, media/resize-driven status, batch-refusal, and reserved-command reports use the helpers; there is no `selected` sender yet. Ordinary registration and fetch rejections are already handled, and no caller consumes a resolved return value. These findings are from static inspection; no tests or writes were performed.

Verdict: Request changes for reconnect isolation and queue liveness.
