# Code review: the image error event on Linux, Android and Windows, round 2, re-check 7 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e44b32a574`.
- **Method:** one brief (sha256 `1245301feee0677de814b7558300cfb491c2af20d935249fba3553c494de68ba`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [remote.rs:54](host/linux/src/image/remote.rs:54), [RasterRetry.swift:99](host/apple/Sources/ExactKit/RasterRetry.swift:99): **ASCII whitespace still lets query tails escape.** Both helpers turn `url("https://h/a?x=a &hm=secret")` into `url("https://h/a &hm=secret")`, preserving the signature. This reaches Linux logs through SVG’s verbatim `@import` warnings at [svg.rs:200](host/linux/src/image/svg.rs:200); trimming the image’s source name does not sanitize URLs inside those warnings. Redact complete URL values before formatting, or handle quoted URL boundaries across whitespace. Add regressions for spaces and tabs inside queries.

**Non-blocking**

none.

The U+00A0 and combining-mark fixes address the previous findings. Static review only; no tests or builds run.
