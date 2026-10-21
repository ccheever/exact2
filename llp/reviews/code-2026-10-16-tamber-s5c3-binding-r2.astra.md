# Code review: the binding core (Tamber stage 5, commit 3), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `600c538e2f867a29d54216a573fce0cee66fc2c8bb2f1bccc4122dbf897fd136`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (reported platform colours missing from inputs; empty edits moved them).
- **Disposition:** in the second-round commit.

---

- **P2 — System-colour changes miss invalidation** — [runner/src/runner/tamber.rs:117](/tmp/rv-s5c3-2/runner/src/runner/tamber.rs:117). `runner.roles().generation()` excludes `roles::reported_generation()`. For a token referencing `AccentColor`, changing macOS’s system accent updates the reported colour without changing modes, scheme, contrast or font size. `inputs` stays identical, so a binding caching resolved colours receives no notification. Include the reported generation.

- **P3 — No-op token writes invalidate every binding** — [runner/src/runner/theme.rs:63](/tmp/rv-s5c3-2/runner/src/runner/theme.rs:63). The counter increments for `set_tokens(&[])` and repeated identical `set_token` calls. Both change `inputs` without changing any value. The added no-op test exercises only `set_modes`, missing this regression.

The P2 should be fixed before sign-off. The other round-1 fixes look sound on inspection.

Tests could not run: the read-only sandbox rejected Cargo’s output directory under `/tmp`. No worktree files were changed.
