# Code review: a bar item's menu without the software keyboard (MenuFocusIOS, LLP 1021 §5.1), round 2, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `eeb012e20`.
- **Method:** one brief (sha256 `56639575ecb41f842681ed088b15d519355519b70a7c2c73d9b4a7913dd0c197`), shared with grok, pointing at round 1 and its dispositions. Blind to grok's round-2 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken: a bar touch that blurs a node arms the view to take the focus at the next touch on the page (test). Round 2 of at most two; the fix was verified by the UIKit tests and on the simulator, not by a third review. Fixing it found a second case, also taken: a node that resigns hands the focus to `ExactView` (UIKit's fallback to its nearest ancestor that takes it, measured in the test), so every set-aside now releases the view too, or the menu's input would take it from the view (test: a context menu over a focused node leaves neither with the focus, and the node has it back after).

---

1. **[P2] Blurring a node leaves shortcuts disabled after subsequent page touches.** [MenuFocusIOS.swift:51](/private/tmp/x2-peek-review/host/apple/Sources/ExactKit/IOS/MenuFocusIOS.swift:51) clears both `held` and `untilNextTouch`. Reproduce by focusing an ordinary button, opening and dismissing a bar menu, then touching or scrolling non-focusable content. `touched()` now does nothing, and nothing makes `ExactView` first responder again. Hardware shortcuts remain unavailable until another focusable control takes focus or the view remounts; [UIKit finds key commands through the responder chain](https://developer.apple.com/documentation/uikit/uikeycommand). Preserve the node’s real blur, but schedule `ExactView` as the fallback responder at the next page touch. The assertion at [ContextMenuIOSTests.swift:300](/private/tmp/x2-peek-review/host/apple/tests/ExactKitTests/ContextMenuIOSTests.swift:300) currently accepts this broken recovery state.

Round-1 findings concerning repeated bar touches, rebuilt navigation controllers, and modal content ownership are fixed. The node-blur change fixes the missing blur event, but introduces the recovery defect above. I found no additional concrete defect in recognizer lifetime, touch cancellation/delays, bar-touch exclusion, quiet context-menu/pull-down restoration, or tvOS gating.

The tests now verify repeated holds and actual responder restoration. They still call the helpers directly: they do not prove touch delivery, bar exclusion, modal/rebuilt-stack integration, or keyboard visibility.

Reviewed `1ea898e8b..eeb012e20`, the checkout’s actual HEAD. `git diff --check` passed. UIKit tests were not run because the read-only sandbox prevents build/cache writes. No files edited.

**Verdict: NOT READY.**
