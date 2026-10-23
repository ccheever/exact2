# Code review: the macOS split-pane popover, narrow re-check 4, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `339fdffe37`.
- **Method:** one brief (sha256 `f3dd624014a967ea202981f66b228b047595bd47b4882175ddb5ab486655e65b`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 58652aca8a

**WRONG — one remaining hit-testing bug.**

[PopoversMac.swift:102](/tmp/iosfix-rv-pane6/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:102) now places the layer in `BlockView`, which lacks the `raisedHit` handling that reconciles hit order with paint depth.

Concrete failing case:

1. Declare a painted `popover="manual"` outside the route stack and open it from detail route A.
2. Push detail route B, retaining A. `MenusMac.sync()` permits this manual popover to remain open.
3. [SplitMac.swift:298](/tmp/iosfix-rv-pane6/host/apple/Sources/ExactKit/Mac/SplitMac.swift:298) appends B after the popover layer. Layout leaves the layer there because its owner hasn’t changed.
4. The popover’s higher `zPosition` paints it above B, but `BlockView`’s ordinary hit test visits B first. Clicking a visible popover control hits the route underneath.

A file-free AppKit probe confirmed the underlying behavior: the later, lower-depth sibling wins `hitTest`. The pane needs hit testing consistent with its paint order, or subview ordering that keeps surviving popover layers above newly mounted routes.

**The assertions do not fail without every fix.** [SplitMacTests.swift:899](/tmp/iosfix-rv-pane6/host/apple/tests/ExactKitTests/SplitMacTests.swift:899) passes without the fourth commit’s `setPaintForeground()` call because another page re-ranking masks its removal; the commit message explicitly records this. The test also never appends a detail route beneath a surviving popover, so it misses the failure above.

No further blocker found in the remaining requested paths. Full XCTest and mutation runs were not performed under the read-only constraint.
