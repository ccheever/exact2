# Code review: a click's token on selected and outline (LLP 1122 T2b), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a34b3b4ab`.
- **Method:** one brief (sha256 `24a73699da0d4278ebb5e9fa3e2b8c052f7607e8694b4a6379f0c9bfac4d1119`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 2 low).
- **Disposition:** taken.
  - The page keeps the last 32 click tokens it made in an inspect session and no longer retires one when its outline arrives. An answer naming any of them leaves the page where it is: a refresh that re-sends a token while its first report is in flight, or both reports accepted. Abandoned tokens are bounded.
  - `click` is tamber/1.7 (the PROTOCOL constant, the tests, the version list, the `*1.7*` marks), and the `outline` shape lists `click?`.

---

- **Medium — [host/web/dev.js:642](/tmp/rv-dt-click/host/web/dev.js:642): an automatic re-ask can still cause a scroll.** If click A’s report is stale and click B happens before its re-ask arrives, `refresh()` re-reports B with B’s token—even when B’s original report is already queued. Both B reports can be accepted. The first outline consumes B’s token; the second scrolls the first copy. **Fix:** correlate re-asks with the rejected click and avoid duplicating a newer selection already in flight. Add a regression covering this ordering.

- **Low — [host/web/dev.js:491](/tmp/rv-dt-click/host/web/dev.js:491): abandoned tokens accumulate indefinitely.** The token enters the set before selection succeeds. An unsupported selection, dropped POST, join without a source map, or refresh finding the node gone produces no outline to remove it. Repeated failures grow the set until inspect ends or the document reloads. **Fix:** track report outcomes and retire abandoned tokens, preserving active retries; cancelled replies must not subsequently trigger scrolling.

- **Low — [docs/tamber-protocol.md:46](/tmp/rv-dt-click/docs/tamber-protocol.md:46): the new field is attributed to an already-landed version.** HEAD~1 also advertises `tamber/1.6` but does not echo `click`. A consumer checking `hello` cannot distinguish support. Optionality allows older readers to ignore the field; it does not identify servers implementing its behavior. **Fix:** advance the protocol minor version and update its constant, documentation, and assertions.

The ordinary two-click case and an intervening tool Outline work by inspection. The queue does **not** coalesce outlines, and the native relay preserves the complete frame. Validation rejects nonstrings, empty strings, and strings exceeding 64 UTF-16 code units. Leaving inspect clears tokens; a full reload starts fresh. Native inspection remains reserved. Tests were not run under the read-only restriction.

Verdict: **Request changes — the automatic retry race still permits the scroll this commit aims to prevent.**