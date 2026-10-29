# Code review: the image error event on Linux, Android and Windows, round 2, re-check 10 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `506c874585`.
- **Method:** one brief (sha256 `5271edb5a3d2937ece2609e514145ef5afc0bb38299cbd81b5391bd5154bb1ad`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/image/remote.rs:100](host/linux/src/image/remote.rs:100): **Apostrophes can expose query tails.** A bundled stylesheet containing `@import url("https://h/a?x=' &hm=secret");` in CDATA produces a warning beginning `the stylesheet's`. That first apostrophe opens a span; the apostrophe inside the query ends it. Scanning resumes with `cut = false`, preserving `&hm=secret`. The subsequent word pass cannot remove it because `?` is already gone. Redact source values before formatting diagnostics, or suppress those excerpts; add a regression using the actual warning prefix.

2. [host/linux/src/image/remote.rs:91](host/linux/src/image/remote.rs:91): **Backtick-delimited parse errors still leak.** A bundled SVG containing `<svg width="1" height="1"></https://h/a?x=a &hm=secret>` produces a closing-tag error containing that URL in backticks. This helper ignores backticks, so the word pass leaves `&hm=secret`, which `svg.rs` logs. Redact the offending source field before interpolation, or suppress it; cover this malformed-SVG case.

The original example is covered, but the blocker is not fully resolved. Findings are from static inspection; no tests or builds were run.

**Non-blocking**

none
