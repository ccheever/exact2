# Code review: the iPad-only Swift tests and the PressFocus sheet flake, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `bfb42b8a7e (before the rebase; the same diff as e93c9cadbe..940ebde15f)`.
- **Method:** one brief (sha256 `79d86a78fb5b143c2a48c452ca762f76f8602f005c062eee2f6ae71c264fbc6b`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**One blocker.** Reviewed both commits read-only; no files changed or tests run.

1. **blocker — [NavigationBarIOS.swift:350](/tmp/rv-ipad-1/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:350): the queued re-enable has no reliable ownership check.**  
   Enabled-before/disabled-after proves the flag changed during layout, not that the disable should expire next turn. A concrete counterexample: the forwarded app `didShow` callback deliberately disables the bar; this block subsequently overrides it. A nil controller coordinator also does not establish that a bar-only items/visibility animation has finished.

   An active subsequent push or interactive pop is protected while its coordinator exists, but that merely abandons recovery; there is no transition identity or retry. This code applies on iPhone too—the original UIKit failure is not established as iPad-exclusive.

   **Fix:** remove the reentrant forced layout from transition teardown. Perform layout followed by cover reporting in a deferred, current-transition-checked pass, preserving the previous covers until then and keeping delegate forwarding synchronous. Leave interaction ownership with UIKit. Cover chained pushes, cancelled pops, bar animations, and an app delegate that deliberately disables interaction.

2. **should-fix — [InspectIOS.swift:296](/tmp/rv-ipad-1/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:296): “outside the selected controller” is too broad to identify tab chrome.**  
   A hatch’s `NodeView` added to `tabs.view` is intercepted before the normal node walk and reported as UIKit’s tab bar. An unrelated collection view there is worse: its cell’s `item` becomes a tab index, regardless of section. Container background also receives the tab-bar message, even when the bar is hidden.

   **Fix:** restrict interception to positively identified, visible tab chrome; otherwise fall through to normal inspection. Prefer item identity; permit ordinal mapping only for a verified tab-only collection with matching sections/count/order. The two-tab fixture does not establish whether sidebar or utility cells preserve that mapping. Add sibling-content and multi-section negative cases.

3. **should-fix — [InspectIOS.swift:147](/tmp/rv-ipad-1/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:147): floating tabs remain impossible to highlight or outline.**  
   Selection now finds a tab, but `place(of:in:)` still returns a closure requiring `tabBar.window != nil`. On the floating path that closure always returns nil. Because the closure itself exists, `InspectOverlay.press` never uses its fallback.

   **Fix:** share the floating tab’s identity/geometry lookup between targeting and placement, resolving the current cell as layout or reuse changes. Assert that the selected tab produces a visible rectangle. Also update [AgentRevealIOS.swift:129](/tmp/rv-ipad-1/host/apple/Sources/ExactKit/IOS/AgentRevealIOS.swift:129), which excludes the floating bar from occlusion calculations for content running underneath it.

4. **should-fix — [NavigationBasicsIOSTests.swift:338](/tmp/rv-ipad-1/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:338): the pushed-route test loses its behavioral assertion on iPad.**  
   With `insets == false`, neither disappearance nor restoration is checked. The remaining `hidesBottomBarWhenPushed` assertion verifies the requested flag, so the test passes even if the floating bar stays visible.

   **Fix:** always verify effective bar visibility through Home → Detail → Chat → Home; condition only the bottom-inset assertions. Also establish that floating chrome actually exists before treating a detached `tabBar` as the legitimate alternative—an accidentally missing bar should fail.

5. **should-fix — [InspectIOSTests.swift:154](/tmp/rv-ipad-1/host/apple/tests/ExactKitTests/InspectIOSTests.swift:154): the scroll exclusion weakens inspect-mode coverage.**  
   Excluding UIKit’s floating collection is reasonable when counting the route’s content scroller. Applying that exclusion during inspection and restoration means the test no longer detects UIKit chrome retaining `scrollsToTop`, or failing to regain it afterward.

   **Fix:** separate the route-scroller baseline assertion from the complete set of eligible scroll views. Verify that inspection disables all of them and restores their original states.

6. **should-fix — [PressFocusIOSTests.swift:598](/tmp/rv-ipad-1/host/apple/tests/ExactKitTests/PressFocusIOSTests.swift:598): the adjacent positive test retains the same timing race.**  
   This remaining issue predates these commits: the presentation can exceed the timer’s second, invalidating the assertion that `note-text` is still absent.

   **Fix:** take the clock before pressing here too, verify the presented sheet and retained handoff, then explicitly advance through the timer and verify autofocus consumes the handoff.

The changed dismissal test’s clock takeover is sound: it leaves UIKit presentation animation enabled and does not consume the handoff. The NavigationTabs visibility helper, FaceSymbol trait correction, and 40-row fixture change also look sound. No-tab sessions bypass the floating lookup; compact layouts with an attached bar retain the existing path, and ordinary sheet/popover hits outside `tabs.view` are not intercepted.
