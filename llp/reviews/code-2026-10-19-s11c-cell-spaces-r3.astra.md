# Code review: Stage 11 (c): the terminal's spaces in whole cells, laid out by the kernel (Env::cell_spaces), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `370578281f`.
- **Method:** one brief (sha256 `15b6f8ec1943a92e5e44cdd7cffbd665ae9c395cfc49f75357d6163c42883bd0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking findings; one should-fix (a calc() tree with 0% unrounded), one nit (1101's ranges by magnitude)
- **Disposition:** both taken in e5b50d2eb1

No blocking findings.

- **Should-fix — [kernel/src/style/cells.rs:55](/tmp/rv-s11k3/kernel/src/style/cells.rs:55):** Zero-percentage rounding only handles compact `Dimension::Calc`. `padding-top: calc(0% + 14px)` becomes 16 px, but the equivalent `calc(0% + 14px + 0vw)` remains `Dimension::Compare` and reaches layout as 14 px: tree evaluation still requires a basis for `0%`. Handle zero-percentage expression trees under `cell_spaces` too. Add a regression asserting both forms resolve to 16 px with the rule enabled and remain unchanged when disabled.

- **Nit — [llp/1101-terminal-apps.rfc.md:458](/tmp/rv-s11k3/llp/1101-terminal-apps.rfc.md:458):** “Up to 8 px is 0 rows” is inaccurate for negative margins. Describe the **magnitude** ranges and state that the sign is preserved, with zero normalized to `+0`.

Validation: all 12 `scale-check` tests passed. Rust/host execution and release-byte/conformance checks were not rerun under the no-build constraint. No files changed.
