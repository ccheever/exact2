# Code review: a held proposal's choice, round 3 re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3a1aa2813d`.
- **Method:** one brief (sha256 `9aaf5d6cc72a8f4409608415f74f8b75fa29c47686dc2638ec8575358f7b9aa4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. Both fixes close the reported races. Accept/edit use the displayed card’s generation; edit rechecks after `choose` and sends `choiceAt`. The server checks it and marks the proposal busy without yielding, preventing an intervening `choose`. Omitting `choiceAt` remains unchecked and backward compatible.

No remaining selection mismatch path found in the reviewed code.

**Non-blocking**

none.

Validation: all 5 choice tests passed, plus offline, in-memory server checks for stale refusal, atomicity, and compatibility. Full server/UI suites were not run.
