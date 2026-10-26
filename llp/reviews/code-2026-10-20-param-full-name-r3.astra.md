# Code review: the Tamber server writes a library component's param by its full name (LLP 1139 D8), round 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9725e28c85`.
- **Method:** one brief per round (sha256 `3dfedcfb6f4b58d8476694bfffa97791957c4d33c8d9fc7746b8c7990ed9ae96`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** SHOULD-FIX: a conditional unset-param of a short line leaves the parent short
- **Disposition:** taken: `respell` on unset-param for a `when` line's removal

**SHOULD-FIX** — [contract/cli/src/tamber_ops.rs:163](/tmp/iosfix-rv-pf3/contract/cli/src/tamber_ops.rs:163): Conditional `unset-param` skips the repair. Given a legacy `Card.inset = 16px` with `when density=compact: 8px`, unsetting only that branch leaves the parent spelled short. The transaction succeeds, but compilation still fails with `tamber-param-short-library`. Respell the retained parent for conditional unsets, and cover compilation and byte-exact undo in a regression test.

No other findings. Static review only; no files modified or tests run.
