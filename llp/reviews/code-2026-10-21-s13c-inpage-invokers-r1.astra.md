# Code review: iOS in-page custom invokers: UIKit's pull-down only for a menu-shaped popover (LLP 1021 D3), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f7cb634c2f`.
- **Method:** one brief (sha256 `b13bdf92e2b13c9bb841825b3c8310fb41d7a667dcd74d8aceae7a73663afd57`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** both taken in 4fa06b94ef (the guard iOS-only; CustomInvokerIOSTests' overlay test opens a menu)

VERDICT: WRONG

1. **blocker — [MenusIOS.swift:187](/tmp/iosfix-rv-ip1/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:187): tvOS loses its activation path.** The guard also removes overlays on tvOS, where `MenuHost.invoke` sets `custom = false` at line 283 and `MenuHost.activating`’s fallback is iOS-only. A handlerless invoker cannot activate through [RemoteTVOS.swift:90](/tmp/iosfix-rv-ip1/host/apple/Sources/ExactKit/IOS/RemoteTVOS.swift:90); adding a press handler still leaves default invocation rejected. A mixed text-and-button popover therefore loses its previously available menu actions. Restrict this guard to iOS or implement the corresponding tvOS activation.

2. **should-fix — [CustomInvokerIOSTests.swift:382](/tmp/iosfix-rv-ip1/host/apple/tests/ExactKitTests/CustomInvokerIOSTests.swift:382): an existing test now fails.** `testAContactWhosePressDropsTheOverlayLeavesNoPressLook` requires an `InvokerButton`, but its fixture’s `choices` popover contains only text. The new guard prevents that overlay, so `XCTUnwrap` fails before testing cleanup. Give this test a menu-shaped target to preserve its intended coverage.

The other requested paths look sound statically on iOS: handlerless touches invoke through `MenuHost.activating` even without calling `Presenter.press`; keyboard and VoiceOver use that helper, and `NodeView` retains its label and button trait. Obsolete overlays are dismissed and removed, invoker touches are excluded from light dismiss, toggle closes the lifted content, confirmations retain their branch, and `agentPaints` bypasses overlay creation before the new guard.

No files changed or tests run.
