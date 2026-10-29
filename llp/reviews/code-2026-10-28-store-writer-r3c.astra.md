# Code review: Linux and Android store writes on a thread of their own, round 3 re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5a08c65211`.
- **Method:** one brief (sha256 `611f27c77be048030ee0d52672ba10af2a93648b0ab8869a50914f018a13f40a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. Both blockers are resolved:

- Windows drains after `run_app` returns, covering bounded runs and `App::fail`, then reports leftover warnings.
- Android queues the drain after hide, pad releases, and owner turn cancellation.

The roots test’s `#[cfg(unix)]` guard matches its HOME/XDG assumptions.

**Non-blocking**

none.

Static review only; no files changed, network used, tests or builds run.
