# Code review: Stage 11 C2: the Android reader held to the Material column, round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `21410de336`.
- **Method:** one brief (sha256 `7eef4aeb7b359084d0125139c0160c6ddf8e3bec6195516943fbd15ec757b0e6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; the same P3
- **Disposition:** taken in the next commit

Nothing blocking.

1. **P3 — the narrowed-page test does not lock a right-only bar.** In `scripts/scale-check.test.mjs`, `narrow` is drawn at `x: 18`, `w: 108` against layout box `a` (`x: 10`, `w: 60`) at scale 2, so the expected left is 20 and the expected right is 140. The recovered left is about 18, already 2 px off, which the old left-edge guard (`> 1`) would refuse by itself. A bar that holds the left edge and moves only the right (or a fill run too short for the 90% search, so `left` is null) is what the new term adds, and that case is not in the test. The formula itself does refuse it.

The other checks hold.

`perf`'s `plan` is the SHA-256 of the running plan's bytes on every host this drive uses. `runner.inspection_digest` hashes `plan.encode()`, and a decoded plan re-encodes to the same bytes. Web wasm boots those bytes with `exact.reload` of `/__plan` before any op; macOS and iOS boot the `EXACT_PLAN` file; Linux reads `EXACT_PLAN`; the reader copies `x_plan` into `EXACT_PLAN` before the library loads and answers `perf` through the same Linux handler. A baked plan that won instead names a different digest and the drive throws before `JSON.parse` of the layout.

The regex's first hit is that `perf` reply. `perf` is the first `--json` line, and `plan` is written before `sites`. Later lines do not win while that field is present. Layout's `planDigest` does not match `"plan":"…"`. A host whose `perf` names no plan fails every drive: a successful reply with no 64-hex `plan` throws `a plan it does not name`; a `perf` error exits non-zero and the drive fails before anything is measured.

The right edge is the same coverage estimate as `edge()`, on the end of the run: `k - (1 - row(k - 1)) + row(k)`, with `k` the first pixel under half ink. For a fill over the ground that recovers the left edge, it recovers the right edge, and `across` is the worse of the two against a 1 px tolerance.
