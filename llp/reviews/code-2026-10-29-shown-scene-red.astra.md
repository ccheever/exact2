# Code review: ShownIOSTests main red (scene guard per test), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `34802a8ca5`.
- **Method:** one brief (sha256 `8721837b290c758f0338f031c657f2e8b860e9bc8df2abacca785ccc46e398e7`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

All 16 tests declare the scene requirement, and the source scanner lists exactly those 16. The helper’s session/window setup is preserved; assertions, teardown, and the availability guard are unchanged. No unrelated changes.

Read-only review; no tests or builds run.
