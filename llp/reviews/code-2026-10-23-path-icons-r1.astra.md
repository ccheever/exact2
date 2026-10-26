# Code review: x2/path-icons: the packages' nonzero path icons as icon tokens, round 1 of 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `e58cbaf963`.
- **Method:** one brief (sha256 `3eb8cfe98044f486fb6fae59158759a008a6d56fd9dbd24e835073220a5898c6`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**MAJOR — Restyle reads still target deleted SVG children.** [scripts/s13c-restyle.mjs:87](scripts/s13c-restyle.mjs:87)

`ProfileHeader.alert` still reads `fill` at `2/2/{0..3}/0/0`. The migration replaces each `svg > path` with an image leaf at `2/2/{0..3}/0`, whose colour is `text_color`. Both the “ProfileHeader tokens” and “ProfileHeader params” gate cases therefore encounter four missing nodes.

**Fix:** change those four entries to `2/2/{0..3}/0 text_color`, retaining the adjacent label reads. Replaying the new tree structure through `readExact` reproduced four failures per case; correcting those entries in memory removed them.

Reviewed the combined diff and all four commits. No other findings: all 15 SVGs and test reference strings match the old paths byte for byte; viewBoxes, nonzero fills, sizing, accessibility and toggle branches are preserved. Namespace/override handling is consistent, and no stale deleted-function or `sample-ar-saved` references remain in active code.

Two selected Bun tests passed. Rust, XCTest and full host gates weren’t run because this read-only checkout lacks build artifacts and dependencies. No files changed.

Verdict: REQUEST CHANGES — repair the four stale ProfileHeader restyle reads.
