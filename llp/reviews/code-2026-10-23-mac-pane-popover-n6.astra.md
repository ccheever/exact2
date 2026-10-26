# Code review: the macOS split-pane popover, narrow re-check 6, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6bac86f58e`.
- **Method:** one brief (sha256 `ec48d37c0b29bf7d7952c45d7d4385f983e245f45a0fc21e76b0d2192e92347c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in c8ec21be59

**WRONG.**

[MenusMac.swift:440](/tmp/iosfix-rv-pane8/host/apple/Sources/ExactKit/Mac/MenusMac.swift:440) still clamps native menu placement to `presenter.viewport`, the list column. Concrete case: declare a popover containing a text-button row in the detail route and open it with default `position-area`, outside agent mode. `show()` selects `NSMenu`, bypassing `paint()` and `layerHost()`. With a 320-point list, an invoker at window x=321 and a 100-point menu, `popUpPoint()` computes window x=220: the menu opens in the wrong column.

The [new fixture](/tmp/iosfix-rv-pane8/host/apple/tests/ExactKitTests/SplitMacTests.swift:840) contains an empty popover, so its native branch exercises `NSPopover`, never this menu path.

The assertions also **do not fail without every fix**. Commit `339fdffe37` explicitly reports that the depth assertion passes without re-ranking because another pass supplies it. Commit `6bac86f58e` likewise reports that the source-less ordering assertion passes under the old owner rule. Those assertions therefore do not establish regression coverage for fixes four and six.

I found no additional concrete blocker in the requested painted-layer consumers. This was a static, read-only review; I did not run tests or mutations.
