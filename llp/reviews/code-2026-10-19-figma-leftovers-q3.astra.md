# Code review: the Figma plugin's leftovers, quick re-check after round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ece9b38f5f`.
- **Method:** one brief (sha256 `9c6e934581fb6f52d60f63f33fbe8856e86060ea74fcd50afb08d91193474b62`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **Correct:** `wanted` now includes the excess above capacity.
2. **Partial:** `roomRefusal` is fixed. The replacement refusal still says to delete every personal mode when those are all that remain; Figma prevents deleting the last. [code.ts:344](/tmp/rv-fp-3/tools/figma-tamber/src/code.ts:344).
3. **Partial:** clearer wording, but the diagnostic can concern a dependent token’s conditional even when all Figma Light modes already agree. The advice still need not resolve it. [mapping.ts:382](/tmp/rv-fp-3/tools/figma-tamber/src/mapping.ts:382).
4. **Correct:** the stray import is removed.

No new correctness or data-loss problems found.
