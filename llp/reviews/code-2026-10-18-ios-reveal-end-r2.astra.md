# Code review: iOS scrollIntoView past the range's end under a collapsing large title (the reveal test's flake), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9f818d1e9`.
- **Method:** one brief (sha256 `51884fd07c24ca4ccfa7dda513e3425e940dae929693bf6ea53580025507ec79`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; the round-1 fix is right.
- **Disposition:** nothing to take.

Nothing is blocking. I found no actionable BLOCKER, SHOULD-FIX, or NIT findings in either commit.

The round-1 fix in [IntoView.swift:219](/tmp/rv-ios-2/host/apple/Sources/ExactKit/IntoView.swift:219) correctly remeasures both the target rectangle and scroll margins after layout. Inner-to-outer ordering remains intact; non-grouped conversions are repeated only when the top inset changes. The recalculation preserves keyboard insets and handles inset growth as well as shrinkage. I found no introduced feedback loop through scroll observers or held `ElementScrolls` replays.

The revised [tests:520](/tmp/rv-ios-2/host/apple/tests/ExactKitTests/NativeListNavigationIOSTests.swift:520) directly check alignment and range clamping before and after an explicit restore batch, removing the timer race from those assertions.

Deferring the varying-height test is reasonable here. That leaves the remeasurement behavior without direct regression coverage, but I would not require a timing-dependent resize test to land this fix.

Static review only; no files changed, builds, or tests run.
