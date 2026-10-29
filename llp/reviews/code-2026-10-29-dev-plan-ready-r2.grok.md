# Code review: dev loops say ready once listening, round 2 of 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9f95444d5e`.
- **Method:** one brief (sha256 `dc3c0eecf46d8d4b3b30ac1962c115f2065625db17e2853356000e309093da1d`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

No findings above NIT.

The round-2 fixes hold. Ready lines stay queued until both listens and the remote-canvas doors succeed, then flush once. A failed listen on `[::1]` or `127.0.0.1`, and a failed door, exit before that flush. Early mode and a resident child (`--serve-as`) do not drop or double a line. `sayReady` is initialized before the compiler starts, so there is no temporal-dead-zone. The dev-scheduler test is untouched, and the new test cannot pass unless a generation is classified before the URL banner (or the "cannot listen" line) and the connection succeeds at the ready line.

Verdict: Round 2 is sound; nothing above a nit.
