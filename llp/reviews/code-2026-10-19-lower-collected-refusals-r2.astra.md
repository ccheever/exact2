# Code review: Lowering keeps collected refusals beside a fatal one, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `cc7055f272`.
- **Method:** one brief (sha256 `30de7ef9a0aedbf531b22a23bbd20660892ee6b3700790327b32639329c7bd22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one P3 (lower_body's comment could say an empty Err)
- **Disposition:** taken in the next commit

No blocking findings. Both round-1 fixes are correct: the borrow is fixed, and the test asserts the exact refusal pair plus `compile_path`’s first error. No other ownership or reference changes in `lower_body` alter meaning.

1. **P3 — documentation clarity:** [The helper’s comment](/tmp/rv-fatal-2/contract/lower/src/lib.rs:363) could explicitly mention that collected-only failures return `Err(Vec::new())`, with diagnostics retained in `l.errors`. The outer comment correctly describes collected-before-fatal ordering, subject to `MAX_REFUSALS`.

Read-only inspection; no builds or tests run.
