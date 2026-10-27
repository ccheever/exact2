# Code review: two Mac flakes' event waits (ScrollEnd, HoverFollowsLayout) and the one EventWaits helper, round 1 of 2, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `8c4bcc97a2`.
- **Method:** one brief (sha256 `76605a99e58448ddc2c33ae5adf8cb9297e086333295f11e30d54d9736dd91aa`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [ScrollEndMacTests.swift:34](host/apple/tests/ExactKitTests/ScrollEndMacTests.swift:34), [HoverFollowsLayoutMacTests.swift:280](host/apple/tests/ExactKitTests/HoverFollowsLayoutMacTests.swift:280):** `took` starts after scheduling. A 20 ms delay before entering `waitFor` leaves 80 ms until a correctly scheduled 100 ms callback, failing the 90 ms minimum. This introduces another load-dependent flake. Measure elapsed time from immediately before the final move/scroll.

- **SHOULD — [SmoothCollectionIOSTests.swift:42](host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:42):** Sampling inside `giveUp` loses the final turn’s sample because [EventWaits.swift:19](host/apple/tests/ExactKitTests/EventWaits.swift:19) checks `done` and the cap first. A late first frame satisfying `driver.drawn` now reports “frames seen 0”; cap failures also omit the final sample. Refresh sampling before producing `facts`, preserving completion-before-budget precedence.

Budget pass/fail ordering is otherwise equivalent. Shared target inclusion, XCTest APIs, and nonescaping mutable captures look valid. No native builds run in this read-only review.
