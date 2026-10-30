# Code review: Apple pass #2: an outline brings the selection's own copy into view, narrow re-check 1, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `07363f4990`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

CLEAN.

`cssVisibilityHidden` reads computed, inherited visibility: [style.rs:984](host/apple/src/style.rs:984) includes it before resolving the node’s style. Hidden ancestors are covered; explicit `visible` descendants remain eligible.

Both predicates and the Mac regression assertion are correct. Outline placement remains unchanged.

Focused XCTest execution was blocked by the read-only sandbox (`permissionDenied`).
