# Code review: iOS in-page custom invokers: UIKit's pull-down only for a menu-shaped popover (LLP 1021 D3), round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f7cb634c2f`.
- **Method:** one brief (sha256 `b13bdf92e2b13c9bb841825b3c8310fb41d7a667dcd74d8aceae7a73663afd57`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** taken in 4fa06b94ef (the test, tvOS); the pressed look tried there, reverted in fc3d755edd and QUEUE'd

**VERDICT: RIGHT WITH NITS**

The iOS guard matches the ruling. A custom invoker’s real touch still opens and toggles a non-menu popover, keyboard and VoiceOver already live on the button, a stale pull-down is removed, light dismiss still ignores the invoker, and confirmations and agent-painted mode stay on their existing paths. Three follow-ups are below.

1. **should-fix** — `testAContactWhosePressDropsTheOverlayLeavesNoPressLook` still requires a pull-down on a text popover. `host/apple/tests/ExactKitTests/CustomInvokerIOSTests.swift:382`

`page()` builds `choices` as a text node (`CustomInvokerIOSTests.swift:101-105`). That popover is not `menuShaped` and not a confirmation, so `pullsDown` is false and `MenusIOS.sync` (`MenusIOS.swift:187`) inserts no `InvokerButton`. The unwrap fails. Menu-shaped suites (`InvokerButtonIOSTests`, `MenuStabilityIOSTests`, `NativeContextsIOSTests`) still have a button row, so they keep the pull-down. The confirmation case at `CustomInvokerIOSTests.swift:573` still gets its overlay. No other test asserts `showsMenuAsPrimaryAction` for a non-menu popover.

2. **should-fix** — A handlerless custom invoker of a non-menu popover no longer shows the pressed look. `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:187`

The overlay’s highlight was what set `invoker.pressed` (`InvokersIOS.swift:25`). With the overlay gone, `touchesBegan` sets `pressed` only for a press handler, a link, a presented closer, or a bubbled native invoker (`PointerIOS.swift:370-371`). A custom button whose only action is `popovertarget` stays visually at rest for the whole press. A button that has a `press` handler still sets `pressed`.

3. **should-fix** — On tvOS the same guard removes the only opener. `host/apple/Sources/ExactKit/IOS/MenusIOS.swift:187`

`invoke` forces `custom` to false off iOS (`MenusIOS.swift:283`). `activating`’s invoker branch is iOS-only (`InvokersIOS.swift:132`). Select presses a node only when it has a `press` handler (`RemoteTVOS.swift:90-92`), and that `Presenter.press` still returns inside `invoke` without opening the popover. Menu-shaped popovers and confirmations still get the overlay, because `pullsDown` is `menuShaped || isConfirmation` (`MenusBarIOS.swift:10`).

4. **nit** — The new test never sends a touch. `host/apple/tests/ExactKitTests/InPagePopoverIOSTests.swift:44`

`p.press` is `Presenter.press` → `invoke`, which already painted a non-menu popover before this commit. The assertion that fails on the old tree is the missing `showsMenuAsPrimaryAction` button (`InPagePopoverIOSTests.swift:42`).

Checks that hold:

- **Real touch.** The hit is the button, or a child such as its text. There is no overlay above them. With no press handler, `touchesEnded` calls `activating(from:pressing:nil)` (`PointerIOS.swift:438-439`), which calls `invoke` (`InvokersIOS.swift:138`). `Presenter.press` runs only when some node on the path `takesPress`. `invoke` then `agentShow`s or, on a second toggle, `drop`s (`MenusIOS.swift:321-324`). With a press handler on the invoker itself, `activating` sees the same node as the press target and calls `Presenter.press` once, and that calls `invoke`.
- **Keyboard and VoiceOver (iOS).** `activateKey` handles a `button` that has an invocation, with or without a press handler (`PointerIOS.swift:491-501`), for Enter and Space. `accessibilityActivate` calls that with focus handoff (`PointerIOS.swift:554`). The button node is the accessibility element: `actsAsButton` sets the button trait (`NodeViewIOS.swift:1166-1170`) and the label comes from the prop (`NodeViewIOS.swift:1164`). A parent accessibility element hides the overlay’s label and trait, so VoiceOver was already on the node.
- **Stale overlay.** The new `continue` skips `live.insert` (`MenusIOS.swift:188`). The cleanup loop `dismissMenu()`s and removes every overlay not in `live` (`MenusIOS.swift:252-258`), including one whose popover has stopped being menu-shaped while its `MenuButton` is open.
- **Light dismiss and toggle.** `PopoverTouch` calls `lightDismiss` on touch-down (`PopoverLayerIOS.swift:99`). A touch on the invoker, including its text, is excluded when any node on that walk invokes this popover (`MenusIOS.swift:830`). The default `popovertargetaction` is toggle (`Invocation.swift:58`), so the touch-up `invoke` drops `agentOpen` (`MenusIOS.swift:323`). A `show` action leaves it up.
- **Confirmation.** `pullsDown` stays true, so the guard falls through to the confirmation branch (`MenusIOS.swift:189-205`): `showsMenuAsPrimaryAction = false`, and `touchUpInside` calls `openConfirmation`.
- **Agent-painted mode.** While `ExactEnv.agentPaints` is set, a non-confirmation never enters the `popovers` map (`MenusIOS.swift:152-158`), so this guard does not run for it. An agent tap still reaches `activating` → `invoke` → `agentShow`.
