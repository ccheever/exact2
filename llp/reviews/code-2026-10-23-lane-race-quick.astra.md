# Code review: the exact-linux executor flake (x2/lane-race), round quick (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5c75e4e053`.
- **Method:** one brief (sha256 `93ab056229d789fdaf1f6e4b6144715e9a28fe71ba1a58f25616f1d847fe822c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

- **Blocking:** None.
- **Should-fix:** None.
- **Nits:** [executor_async_tests.rs:862](host/apple/src/executor_async_tests.rs:862): “the lane wasn't released” overstates the evidence. The assertion proves only that transport activation wasn’t observed within the hang bound; a reader failure or stalled worker could also cause this. Remove the causal suffix.

By inspection, production paths remain unaffected and test meaning is preserved. The fourth ask enters the held response before cancellation; the added wait removes the observed ordering race. Other diagnostics match their conditions.

`async_tests` could not start because the read-only sandbox denied Cargo’s temporary-directory creation. `long_poll` was not run.
