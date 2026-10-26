# Code review: Harmony's seed refuses to empty a server that holds data unless --force (apps/harmony/seed.mjs), narrow re-check 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `5d8e1195c6`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Clean. The comment's 'in those milliseconds' was reworded after it to 'between this one's head read and its seed', as it noted.

Clean — no additional P1/P2 found. The instance header closes the replacement-listener route, and the stand-in’s name matches its assertions.

The same-instance race is the only residual I found. Its window lasts from the head read until the seed commits; “milliseconds” is descriptive, not a guaranteed bound.

Read-only inspection; no tests, servers, seeds, or network.
