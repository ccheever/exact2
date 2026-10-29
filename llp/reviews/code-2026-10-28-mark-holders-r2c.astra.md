# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), round 2 re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `344cca6c5a`.
- **Method:** one brief (sha256 `6702413e8a67af33c846b8ab68157af211f9e6b1aa2a870469e889e33cafbcbe`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

NIT — [kernel/src/kernel/intrinsic.rs:101](kernel/src/kernel/intrinsic.rs:101): The tuple eagerly evaluates `mark_key` even for `None`. Linux repeatedly sends `None` for Material/neutral marks, so unchanged reports now unnecessarily resolve inherited font size, potentially walking ancestors. Handle `None` by removing the record directly; compute the key only for `Some`.

Both requested fixes otherwise check out. Caps passed with wasm compilation disabled. Rust tests were not run in this read-only checkout.

VERDICT: findings
