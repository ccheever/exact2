# Code review: ElementScrollTests' motions observed by bounds changes (a Mac flake), round 1 of 2, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `6c83a7dc2b`.
- **Method:** one brief (sha256 `a444373b7278368f98c1de4cd80b97259d256c2291ca0ff8a298cbf1cd3018a0`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**BLOCKER — [ElementScrollTests.swift:297](host/apple/tests/ExactKitTests/ElementScrollTests.swift:297): the slowest-turn measurement misses the completing turn.** `waitFor` checks `done()` before `giveUp()`. If a stalled turn delivers the sole bounds change and animation completion, `giveUp` never measures it, so the approved one-frame exemption still fails. A run-loop reproduction measured a 443 ms completing turn while this measurement remained 0 ms. Measure after every run-loop call, including the final turn.

Observer cleanup and synchronous mark timing look correct. Both macOS/iOS branches pass Swift parsing; full compilation was blocked by sandbox module-cache writes. Per-frame AppKit notification delivery remains runtime-unverified.
