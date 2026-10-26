# Code review: the macOS split-pane popover, narrow re-check 7, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c8ec21be59`.
- **Method:** one brief (sha256 `ec78ceaa3917f7f865a1ab472ddfa8802c98a01de5d3382c6176f1b4b5ccef96`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**RIGHT — no additional blocking defect found in `HEAD~7..HEAD`.**

The owner reconciliation, per-owner ordering, depth refresh, pane Escape guard, and native-menu clamp fit together. Light dismiss uses window hit testing; capture and accessibility include the split tree. Restore uses the saved authored parent. The remaining viewport-based depth lookup assigns pane popovers `−1`, preserving their opening order; the separate columns clip their contents.

**The assertions do not independently fail without every fix.** Two gaps are explicitly recorded in the commits:

- [SplitMacTests.swift:904](/tmp/iosfix-rv-pane9/host/apple/tests/ExactKitTests/SplitMacTests.swift:904): the depth assertion also passes without the explicit re-ranking call because another ranking pass repairs it.
- [SplitMacTests.swift:902](/tmp/iosfix-rv-pane9/host/apple/tests/ExactKitTests/SplitMacTests.swift:902): the source-less ordering assertion also passed with the old owner rule because another pass restored its position.

The commit records report failing mutations for the other regression assertions, including the latest menu clamp (`−101` versus expected `0`).

I did not run XCTests because their harness writes fixture files. An in-memory AppKit probe preserved focus during reordering and movement between owners. No files changed.
