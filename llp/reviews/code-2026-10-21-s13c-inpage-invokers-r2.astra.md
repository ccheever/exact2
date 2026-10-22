# Code review: iOS in-page custom invokers: UIKit's pull-down only for a menu-shaped popover (LLP 1021 D3), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4fa06b94ef`.
- **Method:** one brief (sha256 `1243d28e9188285412f87a9a8ba95b373c75d6bacb619f93118ea485abf5303b`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** the blocker (the pressed-look change took the press from an ancestor) answered by reverting it in fc3d755edd; the look QUEUE'd

VERDICT: **WRONG**

1. **blocker — [PointerIOS.swift:373](/tmp/iosfix-rv-ip2/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:373): The pressed-look fix swallows ancestor press handlers.** A handlerless custom invoker now sets `pressed = true`, skipping forwarding to `pressResponder`. On release, `touchesEnded` calls `Presenter.press` on that handlerless button. The popover opens, but the ancestor’s handler never runs, and the runner returns `NoHandler`. Keyboard, VoiceOver, and painted-agent activation still resolve the ancestor through `takesPress`, so input paths disagree. Preserve handler forwarding while highlighting the invoker. [InPagePopoverIOSTests.swift:44](/tmp/iosfix-rv-ip2/host/apple/tests/ExactKitTests/InPagePopoverIOSTests.swift:44) bypasses this path with direct `p.press` calls; cover touch down/up on a handlerless invoker inside a pressable ancestor.

The other requested paths look sound statically: accessibility label/button trait, dismissal and removal of obsolete overlays, light-dismiss exclusion and toggle, confirmations, and tvOS’s retained overlay. I found no remaining test expecting a pull-down for non-menu content.

Static review only; no tests run or files edited.
