# Code review: ResetGate's tables after #1141 and #1144, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `60fe90927d`.
- **Method:** one brief (sha256 `65d8b161630c7cfed021804f2394a3f67e2ab00c00166d05799ea651213b2a80`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/apple/tests/ExactKitTests/ResetGateTests.swift:336` (same call at line 367). Both new tests end the loop with `window?.close()`. On iOS `window` is a `UIWindow`, which has no `close()`, and this file is compiled for iOS (`#if os(iOS) || os(macOS)`). `tearDown` in the same file already hides the window on iOS and only calls `close()` on macOS. The iOS test class fails to compile, so neither recovery test runs there. Use that same split: `window?.isHidden = true` on iOS, `window?.close()` on macOS, then `window = nil`.

**Non-blocking**

1. `host/apple/tests/it/reset_gate.rs:213`. A holding `on` closure is detected with `closure.contains("read(")`, which is true of any call whose name ends in `read`, including `Thread(` and `already(`. Nothing in the tree hits this today (the scan test passes). Treat `read(` as a whole word: the byte before it must not be an identifier character.

2. `host/apple/tests/ExactKitTests/ResetGateTests.swift:364`. The admitting call is `check(frame: true, update: true)`. The update callback (`TimerAdmissions.swift:98`) is `check(update: true)`, and `check` only posts to the owner when `frame` is false (`TimerAdmissions.swift:144`). The update bit is still set, so the wait is admitted and the held async check is named earlier in the test. The async iteration never shows that a posted update check after the restart admits. Call `check(update: true)`.
