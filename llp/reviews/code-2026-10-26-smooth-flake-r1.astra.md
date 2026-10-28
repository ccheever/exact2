# Code review: SmoothCollectionIOSTests' frame-bounded waits, round 1 of 2, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `59a7b09de6`.
- **Method:** one brief (sha256 `381c588dc38e981cd656b767e17825c5b5a520c5f55e43bcf6cc685ded97c616`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

CLEAN.

- Timestamp sampling bounds this driver: `FrameClock` dispatches synchronously, and `OffsetDriver` advances using `targetTimestamp`. First/last exclude stale pre-wait time; counts and gaps reflect polling observations.
- Async start waits at lines 87, 98 and 261 can reach the 30 s cap without frames. Acceptable failure-delay tradeoff under the approved plan.
- Attachment calls match the installed iOS XCTest headers; `.keepAlways` preserves successful-test evidence ([Apple documentation](https://developer.apple.com/documentation/xctest/adding-attachments-to-tests-activities-and-issues)).
- All behavioral assertions remain intact.

Read-only review; iOS tests not run.
