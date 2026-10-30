# Code review: longpoll.test.mjs main red (a missing file is a 404), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b4b59a01fb`.
- **Method:** one brief (sha256 `3514c004ff54f22abab6a54605a20d573f06edc19cdef77792095a280594586a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The fix is correct and complete for this red. Missing `exact.json` only makes optional source-map discovery unavailable; the [reader catches the HTTP error](scripts/agent-inspect.mjs:97). The unchanged [assertions](host/web-js/longpoll.test.mjs:85) still check retries, cursors, heartbeat suppression, cancellation, and held replies.

`QUEUE.md` removes exactly the reported line. No files edited, network used, or tests/builds run.
