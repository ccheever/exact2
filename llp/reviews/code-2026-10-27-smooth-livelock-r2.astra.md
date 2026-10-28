# Code review: the iOS smooth-correction read-back livelock (SmoothCollectionIOSTests' flake), round 2 of 2, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `0b54dfb664`.
- **Method:** one brief (sha256 `802311195f62c83e84c3f436e701318a6a62b6620eb664b62d4ea110e3896e3e`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD — [SmoothCollectionIOSTests.swift:113](host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:113):** Force 3× display scale, as the reorder test does. On a 2× simulator, the search produces exactly representable half-point offsets, leaving `start` nil and failing `XCTUnwrap` before exercising the fix. The harness permits these destinations through `--sim`/`EXACT_SIM`.

No other findings. The reorder test exercises the intended failure for the documented readback behavior; fractional limits still resolve to 100 at both scales. Reviewed source and checked arithmetic; UIKit tests were not rerun in the read-only sandbox.
