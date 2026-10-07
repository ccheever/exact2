# Code review: the status bar's style from state, LLP 1105, round 3 (7e3b9a827), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `7e3b9a827`.
- **Method:** one brief (sha256 `0fb4874a5337132cc7b5c43c2368e37ee9b0c1d53f1b29bb172ee6e4ca82bde6`), shared with grok. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: only a coordinator that moves a controller (one with a `from` controller) holds the style; a rotation's or resize's does not. 2 taken: the modal's reappearance resolves as settled, at once if the coordinator refuses the completion. 3 not taken: a custom tab container that parks an unselected stack on screen, unhidden, lets it compete (LLP 1105 D2 says so). 4 not taken: no session-driven UIKit test. Three rounds reached; landed with these fixes.

---

DO NOT LAND

1. **MATERIAL — Rotation can strand a style change.** [NavigationIOS.swift:555](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:555) treats every coordinator as navigation in flight, including [UIKit’s rotation/resize coordinators](https://developer.apple.com/library/archive/featuredarticles/ViewControllerPGforiPhoneOS/CustomizingtheTransitionAnimations.html). A viewport-dependent style flip is therefore rejected by [StatusBarIOS.swift:23](host/apple/Sources/ExactKit/IOS/StatusBarIOS.swift:23). Rotation triggers neither `didShow` nor modal appearance; unchanged final geometry produces no repair batch. **Fix:** restrict the guard to relevant transitions, or register a completion retry whenever it suppresses resolution.

2. **MATERIAL — The cancelled-dismissal fix can still hit the same guard.** [ModalIOS.swift:73](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:73) retries through ordinary `resolveStatusBar()`. The finishing coordinator can remain available during completion and be inherited by the navigation controller, causing finding 1’s guard to reject this retry. Flip `light-content` to `dark-content` during a zoom dismissal, then cancel: no presentation/dismissal completion subsequently repairs it. The late `animate` registration’s failure is also ignored. **Fix:** identify and exempt only the completed coordinator, handle rejected registration, and test cancellation with an intervening props batch. Round 2’s disposition is incomplete.

3. **MATERIAL — Unselected custom tabs remain eligible.** [StatusBarIOS.swift:58](host/apple/Sources/ExactKit/IOS/StatusBarIOS.swift:58) excludes presentations but never checks selected-stack membership. A supported custom tab container that parks unselected stacks offscreen, without `isHidden`, lets their declarations compete; a later-mounted dark declaration can override the selected light route. **Fix:** restrict route candidates to the scope’s committed top route and selected stack, retaining surrounding root declarations only in primary scope. This acknowledged Round 2 omission still violates D2.

4. **MINOR — Required UIKit behavior remains untested.** [StatusBarIOSTests.swift:117](host/apple/tests/ExactKitTests/StatusBarIOSTests.swift:117) increments `reread` in its own callback and manually reads the getter. Removing UIKit invalidation still passes. The “scroll” test injects props; fade checks only a Boolean; no modal is presented. **Fix:** drive a scroll-bound session and observe UIKit getter reads on root and presented controllers, animation context, and transition completion/cancellation.

The revised covering scope and other earlier fixes hold on inspection. No broader batch skip was introduced; idle-tick dispositions remain intact. Caps, schema digest, agent formatting, and iOS/tvOS/macOS Swift parsing passed. Full builds and UIKit execution remain unverified in this read-only checkout.