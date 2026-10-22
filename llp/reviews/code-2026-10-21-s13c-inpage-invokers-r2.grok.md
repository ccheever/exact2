# Code review: iOS in-page custom invokers: UIKit's pull-down only for a menu-shaped popover (LLP 1021 D3), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `4fa06b94ef`.
- **Method:** one brief (sha256 `1243d28e9188285412f87a9a8ba95b373c75d6bacb619f93118ea485abf5303b`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** its should-fix (the same) answered by the revert

**VERDICT: RIGHT WITH NITS**

The iOS ruling holds. A custom invoker gets UIKit’s pull-down only for a menu-shaped popover or a confirmation; any other popover opens as itself, toggles on a second press, and a touch on the invoker is inside light-dismiss. The tvOS overlay is preserved. One press-ownership bug came in with the pressed-look fix.

1. **should-fix** — A real touch on a handlerless custom invoker swallows the ancestor’s press. `host/apple/Sources/ExactKit/IOS/PointerIOS.swift:373`

`pressed = true` is both the look and press ownership. On a finger-down, the hit is the topmost view (the label, or the button’s own padding). A text child has no handler, so it forwards through `pressResponder`. The button’s new clause matches (`!contact.bubbled`, `isButton`, `invocation.opens`, `!hasMenuOverlay`) and stops the walk. On touch-up, `touchesEnded` calls `activating(from:touched, pressing:self)` (`PointerIOS.swift:452`). `activating` only presses a different target when `source !== target` (`InvokersIOS.swift:133`); here the invoker is the target, so it calls `Presenter.press` on the button (`PresenterIOS.swift:630`). That id has no handler, the ancestor never hears the press, and `invoke` then opens or closes the popover.

That disagrees with every other activation of the same button. Keyboard Enter/Space walks to the first `takesPress` node (`PointerIOS.swift:496`), and a handlerless button does not take a press (`PointerIOS.swift:518`), so the ancestor runs and then the popover toggles. VoiceOver uses that same path (`accessibilityActivate` → `activateKey`, `PointerIOS.swift:556`). The agent’s finger does too (`AgentPanIOS.swift:210`, `283`). A handlerless native button does too (`NativeButtonsIOS.swift:193`). The old pull-down did too: its deferred menu called `pressAsTouched` (`InvokersIOS.swift:90`), which walks `takesPress`. `CustomInvokerIOSTests.page` is this shape — root id 1 has the handler, button id 2 does not — and `testKeyboardAndAccessibilityActivateAHandlerlessInvoker` expects id 1 five times (`CustomInvokerIOSTests.swift:541`). A real `Touch` on the label would open the popover and record no press. `InPagePopoverIOSTests` calls `p.press` (`InPagePopoverIOSTests.swift:44`), so it never takes this path. The same clause also claims a native content-popover button’s padding (`isButton` includes it, and it has no `InvokerButton`), while the control face still goes through `activateNative`.

Give the look without making a handlerless opener the press owner, so a finger still presses the nearest `takesPress` ancestor and then invokes, as the keyboard, VoiceOver, the agent, and a native button already do.

**The other checks hold.**

A real touch still reaches `Presenter.press` and `invoke`, so the popover opens and a second tap toggles it. The node that takes the hit is the text (or the button, on padding); the button then owns the contact.

Keyboard and VoiceOver still activate it. The node stays the accessibility element: `actsAsButton` sets `isAccessibilityElement` and the button trait (`NodeViewIOS.swift:1166`), and the label comes from the node (`NodeViewIOS.swift:1164`, `Accessibility.swift:610`). `accessibilityActivate` still calls `activateKey`.

An open `MenuButton` whose popover stops being menu-shaped drops out of `live` because the guard `continue`s before `live.insert` (`MenusIOS.swift:189`). The cleanup loop dismisses it, removes it, and clears `overlays` and `menuShapes` (`MenusIOS.swift:255`).

Light dismiss runs on touch-down (`PopoverLayerIOS.swift:99`) and skips an eligible button whose invocation target is the open popover (`MenusIOS.swift:833`), including when the hit is the button’s text. Touch-up then toggles (`MenusIOS.swift:326`). `PopoverLayer.hitTest` returns nil off the popover, so the touch falls through to the invoker.

A confirmation still takes the confirmation path. `pullsDown` is `menuShaped || isConfirmation` (`MenusBarIOS.swift:10`), so the guard does not skip it, and sync still builds an `InvokerButton` with `showsMenuAsPrimaryAction = false` and a `touchUpInside` action (`MenusIOS.swift:192`).

In the agent’s painted mode, a non-confirmation popover never enters the `popovers` map (`MenusIOS.swift:152`), so the guard does not change overlay policy. A real finger there shares the ownership bug above, because there is no overlay. The agent’s own tap still presses the `takesPress` ancestor and then invokes.

On tvOS the guard is inside `#if os(iOS)` (`MenusIOS.swift:188`). The overlay stays, which is what Select needs: `remoteSelect` only calls `presenter.press` when the node has a press handler (`RemoteTVOS.swift:90`), and the keyboard invocation branch is iOS-only.

No remaining test expects a pull-down on a non-menu custom popover. The overlay test in `CustomInvokerIOSTests` now puts a button row in the popover (`CustomInvokerIOSTests.swift:381`), so it stays menu-shaped. The menu and confirmation tests still have a menu-shaped popover or an alertdialog, and the native-button tests go through the native path above this guard.
