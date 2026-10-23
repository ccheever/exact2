# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), fourth narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e0269c60be`.
- **Method:** one brief (sha256 `inline prompt (the narrow re-check's brief was given on the command line)`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** nothing blocking

VERDICT: RIGHT

1. **Prior P2 resolved** — [contract/lower/src/anatomy/library.rs:649](/tmp/iosfix-rv-bi6/contract/lower/src/anatomy/library.rs:649) records each label’s own loop/index/match bindings; line 869 passes them to `mentions`. Scope restoration and derive expansion remain correct. [label-own-binding](/tmp/iosfix-rv-bi6/contract/cli/tests/it/library_rules.rs:740) covers the reported case.

No blocking findings. Tests reviewed but not run because their fixtures write files.
