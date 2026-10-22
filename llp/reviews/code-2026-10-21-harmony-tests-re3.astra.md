# Code review: Harmony's web tests against a Snapback server of their own (scripts/memory-backend-test.mjs, the async lane), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `c7e84ac59b`.
- **Method:** one brief (sha256 `4b3fd3cc00359a12af91aaed8bcbd5d54eb069a3f9c47f618e862f2cdb1bb122`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Nothing blocks this fix in HEAD (`c7e84ac`).

The grace SIGKILL rechecks `{pid, start}`. The post-exit sweep filters matching identities before expanding descendants, then rechecks identities before each signal.

Static review only. No files edited; no tests, servers, installs, or network commands run.
