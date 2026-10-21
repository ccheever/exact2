# Code review: caps: collection/tests.rs back under 1500 (a pure move), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `5d69fb92e2dafb84a76962d9b19f2cc74cca131dd57aa331e76537098260fc3f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** ship (both).
- **Disposition:** none needed

---

No MAJOR or MINOR findings.

- Both tests and their helper are byte-for-byte unchanged in [key_reads_tests.rs](/tmp/rv-caps/runner/src/instance/collection/key_reads_tests.rs:6). All 32 original tests remain; no assertions or cases were weakened.
- [Module wiring](/tmp/rv-caps/runner/src/instance/collection/tests.rs:7) is correct, inherits `cfg(test)`, and preserves helper access through `use super::*`.
- `caps` and `git diff --check` pass. Files count as 1,438 and 83 lines under the cap checker.

Rust tests were not run: the sandbox is read-only with no build artifacts. No files changed.

VERDICT: ship
