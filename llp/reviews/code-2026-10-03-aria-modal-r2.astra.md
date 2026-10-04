# Code review, round 2: aria-modal r2 (cc6a3efc1), 2026-10-03 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `bd1dacae6`. The reviewed commit, cc6a3efc1, is its parent's parent.
- **Method:** one brief (sha256 `f8a3896d543c7f10e02909f7c75d08e8a70b8b661f0c1fa1a3806d59bd4c3036`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):**
  - 1 (a destroyed modal skips the post): taken. `announcedModal` is now the view id, and the test destroys the last modal.
  - 2 (an accessibility-hidden ancestor): taken. Every UIKit ancestor is checked, and the test hides the root.
  - 3 (deferred containment): taken. `syncModal` runs after navigation's deferred sync, after `didShow` and after sheet presentation and dismissal. It holds the post while a transition runs.
  - 4 (a modal inside a suppressed branch): taken. Winners are resolved shallowest first, and a candidate under a branch another winner hides is skipped. The test covers the three-modal topology.
  - 5 (the agent reports the outer modal): taken. The walk keeps the innermost modal as the boundary.
  - 6 (the focus target is a container): taken. The post names the modal's first accessibility element, or nil.

---

Static review only; no files changed or tests run. The direct sibling, display-toggle, and exit fixes are sound, but notification and nested-modal handling still need fixes.

1. **Should-fix — destruction can erase the previous modal before the close notification.**  
   [PresenterIOS.swift:19](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:19) stores `announcedModal` weakly. Ordinary destruction removes the view from both the map and hierarchy before reconciliation. If it deallocates, [Accessibility.swift:201](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:201) compares `nil` with `nil` and skips `.screenChanged`. Conversely, reusing the same object for another modal incarnation can incorrectly compare equal.

   **Fix:** retain an immutable notification identity, including incarnation, independently of the weak view reference. Test destruction without a retained test reference and replacement through reuse; assert notification decisions, not merely `announcedModal == nil`.

2. **Should-fix — accessibility-hidden ancestors do not exclude a modal.**  
   [Accessibility.swift:188](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:188) checks only the candidate’s `accessibilityElementsHidden`. `accessibilityVisible` walks ancestors for `isHidden` and inertness, but never their accessibility-hidden flag. Consequently, a modal beneath `aria-hidden=true` remains selected. This also affects native sheets: [ModalIOS.swift:265](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:265) hides the background navigation view through precisely that missing property.

   Background updates can therefore announce an inaccessible modal while a sheet is open, and hiding/revealing an ancestor does not produce the intended modal transition.

   **Fix:** check `accessibilityElementsHidden` on every UIKit ancestor, including non-`NodeView` wrappers. Cover authored hidden ancestors and a modal retained beneath a native sheet.

3. **Should-fix — native containment changes can leave modality stale after the batch.**  
   The batch calls accessibility reconciliation at [PresenterIOS.swift:860](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:860), but navigation can defer containment until a transition finishes. [NavigationIOS.swift:429](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:429) and [NavigationIOS.swift:628](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:628) subsequently run navigation reconciliation without accessibility reconciliation. `fit()` does not guarantee another batch when geometry is unchanged.

   A modal excluded because its route was detached can therefore become attached without acquiring modality or being announced. Conversely, dismissal restores background containment before the outgoing presentation finishes, allowing an announcement too early.

   **Fix:** reconcile modality after deferred native containment completes; defer and coalesce focus notifications while that transition is unfinished. Test presentation/dismissal with unchanged geometry and no subsequent app update.

4. **Should-fix — a modal inside a suppressed sibling can become the notification target.**  
   [Accessibility.swift:190](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:190) resolves only immediate sibling conflicts. Consider sibling modals A and B, with B in front, and a newer modal C inside A. The algorithm selects B and C, clears A’s flag, and [Accessibility.swift:200](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:200) announces C because its ID is greatest. But B’s sibling exclusion hides A’s entire subtree, including C.

   **Fix:** resolve winners through the hierarchy and discard candidates inside branches suppressed by another winner before choosing the announcement target. Add this three-modal topology to the tests.

5. **Should-fix — the agent reports the outer modal when an inner modal is active.**  
   [AgentAccessibility.swift:197](/tmp/x9-review/host/apple/Sources/ExactKit/AgentAccessibility.swift:197) records only the first modal encountered. With nested authored modals, that is the outer one, while `syncModal` ordinarily announces the newer inner one. [AgentAccessibility.swift:470](/tmp/x9-review/host/apple/Sources/ExactKit/AgentAccessibility.swift:470) then evaluates leaks against the outer boundary.

   For an inner modal inside a wrapper, a control elsewhere inside the outer modal remains exposed but is incorrectly marked `outsideModal=false`.

   **Fix:** preserve nested modal boundaries and evaluate/report the effective active boundary consistently. Test an inner modal with an exposed control outside its wrapper but inside the outer modal.

6. **Should-fix — the notification target is usually a container, not an accessibility element.**  
   [Accessibility.swift:203](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:203) passes the modal `NodeView` itself. An ordinary dialog column is not an accessibility element; its children are. Apple documents the optional target as the accessibility element VoiceOver should focus. [Apple documentation](https://developer.apple.com/documentation/uikit/uiaccessibility/notification/screenchanged?changes=__8&language=objc).

   The assertion at [AccessibilityTreeIOSTests.swift:135](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:135) checks only the stored pointer, so it does not establish the claimed focus handoff.

   **Fix:** pass an exposed accessibility element within the modal, or `nil` for UIKit’s selection. Validate the notification argument and unchanged-batch suppression separately from bookkeeping.

Round-1 accounting:

| Findings | Status in r2 |
|---|---|
| Astra 1, 4, 5; Grok 2; remaining macOS portions | Addressed by removing the implementation and explicitly deferring macOS. |
| Astra 2, iOS exit behavior | Resolved: exit removes candidates and clears retained modal flags. |
| Astra 3; Grok 1 | Display/prop transitions improved; notification lifecycle remains incomplete as above. |
| Grok 3 | Wrapper and route-local limitations are documented. |
| Grok 4 | Direct siblings resolved; nested cases remain. |
| Grok 5 | Partially resolved: clear/display/exit assertions added; destruction, reuse, conflicting z-order, and actual notifications remain uncovered. |

The main-thread guard is sound. Style-only batches reach reconciliation; the collection-snapshot shortcut changes no modal exposure. `"false"` remains indexed but is rejected correctly, while cleared props are cleaned through `modalViews`. The iOS `zPosition` comparison follows the presenter’s effective `usedZIndex`, with subview order breaking ties. The moved web test is registered through `mod aria`.

Verdict: LAND WITH FIXES
