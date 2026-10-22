# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `614159b9e1`.
- **Method:** one brief per round (sha256 `5fb781cf96130bdc6d4d06b2d310ff8592151dc36094b2a14f131f46dde5b0b6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 SHOULD-FIX (exhaustion with a scroll in the last round's frame), 2 NIT
- **Disposition:** all taken in 7b3cb2e06f

- **SHOULD-FIX — [host/web-js/agent.js:415](/tmp/iosfix-rv-ss3/host/web-js/agent.js:415):** `lastScroll` only records events delivered during `fireOwedScrolls()`. Round 15 can subsequently move a list in `exact.advance()`, deliver its scroll during the rAF at line 407, and let the handler start another animation. With the mark acknowledged, this returns `settled: true` without a quiet round. An in-memory harness of this settle branch reproduced that result. Require a quiet round before reporting success at the bound.

- **NIT — [scripts/agent.mjs:1232](/tmp/iosfix-rv-ss3/scripts/agent.mjs:1232):** The diagnostic incorrectly says the browser has not fired the event when `lastScroll === 15` can mean it already fired. “Waited 8 frames” also understates repeated waits across rounds.

- **NIT — [host/web-js/conform.mjs:502](/tmp/iosfix-rv-ss3/host/web-js/conform.mjs:502):** After boot, `driveAt` still names the previous comparison when settle fails. Pass the current step explicitly; use `reference`/`other` instead of hardcoded `wasm`/`js` for cross-browser runs.

No files modified; browser tests were not rerun.
