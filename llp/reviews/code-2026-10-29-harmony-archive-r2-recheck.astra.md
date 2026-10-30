# Code review: Harmony production archive (development-only grant lines), round 2, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6df77db73c`.
- **Method:** one brief (sha256 `d30be7cfbbac6484e8827243991358ec646d90f1adf01d4095fdb3f95b517d64`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. The origin-rewrite, keyed-validation, and Rust-diagnostic findings are resolved. No new code defect found by static inspection; no tests or builds run.

**Non-blocking**

1. HEAD commit message, lines 6 and 19: still says “before” backend selection and “no CallKit.” Change these to “after” and “no `device.calls` grant, hence no `voip`/`audio` background modes.” The calls implementation remains linked.
