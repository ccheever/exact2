# Code review: dotted deep links round 3, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `387b8f91a3bcc25e4a7492445f23ea7e0e3ad2875650ca4439ac742c4f3b1f1d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

No findings.

HEAD's three fixes hold for a browser navigation and for the clone.

- `scripts/origin.mjs:65-67` — `.hbc`, `.model`, and `.tex` are in `FILE_EXTENSIONS`. A navigation whose `Accept` names `text/html` to a missing one stays absent. `/profile/jay.bsky.team` is still the document, including with a trailing slash, and so are handles ending in `.tech`, `.mov`, `.app`, and `.fm`. `/profile/photos.zip` stays a file, which lines 60–64 already say.
- `scripts/origin.mjs:70-85` — commas and semicolons split only outside quotes, and a backslash inside quotes escapes the next character. Chrome, Firefox, and Safari navigation headers, and the native client's `application/vnd.exact.envelope+json, text/html;q=0.9`, still accept HTML. `*/*`, `text/html;q=0`, and `profile="a,text/html,b"` do not. On every unquoted header checked, the answer matches the previous splitter.
- `host/web/deep-link.test.mjs` — 2 tests, 22 assertions, passed.

READY
