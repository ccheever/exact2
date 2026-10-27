# Code review: the iOS smooth-correction read-back livelock (SmoothCollectionIOSTests' flake), narrow re-check 1, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `a2c38bcb87`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

CLEAN on static review.

CoreSimulator access is blocked by this sandbox, so I could not confirm that the 3× override still produces the `5/3` read-back on this simulator.
