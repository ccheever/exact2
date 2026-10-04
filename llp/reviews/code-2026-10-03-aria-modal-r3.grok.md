# Code review, round 3 (final): aria-modal r3 (d212f7652), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f3e676311`.
- **Method:** one brief (sha256 `5b7c113dd67b385fe682e817e49757c0d723abd10382bd71fe55994be88eca9a`), shared with astra. Round 3, the last, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the round-2 table, findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r4, landed):**
  - 1 (a modal nested under a covered view): taken.
  - 2 (an exiting modal covers the one behind): taken, with an overlapping test.
  - 3 (the focus search): taken by passing nil; naming an element is deferred.

---

## Round 2

| Finding | Status |
|---|---|
| Grok 1, Astra 1 — destroy never posts | Resolved. `announcedModal` is the view id (`PresenterIOS.swift:19`). A destroyed modal is gone from `candidates`, so the id changes and `.screenChanged` is posted, including to `nil`. The new test destroys the last modal. |
| Grok 2 — sheet over a root modal; dialog in a covered stack | Resolved for those two cases. A sibling that paints in front disqualifies that view, and `accessibilityElementsHidden` is read on every ancestor, so a dialog inside the stack `ModalIOS` hides is dropped. |
| Grok 3 — schema and RFC | Resolved. The schema comment says macOS is deferred, and the proof points at `host/web/tests/it/aria.rs`. |
| Astra 2 — accessibility-hidden ancestors | Resolved. The ancestor walk includes non-`NodeView` wrappers (`Accessibility.swift:188`). The test hides the root. |
| Astra 3 — modality left stale after deferred containment | Resolved. `syncModal` runs from `NavigationIOS.swift:433` (after the deferred sync), `NavigationIOS.swift:637` (`didShow`), and `ModalIOS.swift:303` and `ModalIOS.swift:355` (sheet present and dismiss). Flags update immediately; the post waits while `navigation.inTransition` or `modals.inTransition` (`Accessibility.swift:209`). |
| Astra 4 — modal inside a branch another modal hides | Resolved when the thing in front is itself a modal. Shallowest-first, then skip a candidate whose ancestor has a sibling winner (`Accessibility.swift:196`). A, B, C in the new test get flags `[false, true, false]`. |
| Astra 5 — agent keeps the outer modal | Resolved. The walk replaces `modalView` when the new modal is a descendant (`AgentAccessibility.swift:198`), and `markModalLeaks` uses that boundary (`AgentAccessibility.swift:472`). |
| Astra 6 — focus target is the container | Resolved for a normal column of text and buttons. The post takes the first descendant with `isAccessibilityElement`, or `nil`. What that search misses is Nit 3. |

## Findings

### 1. Should-fix — A modal nested under a covered view stays modal

`host/apple/Sources/ExactKit/Accessibility.swift:191`

`exposed` only asks whether a sibling paints over the candidate. A sheet’s navigation view is added beside the root overlay (`NavigationIOS.swift:334`, brought to the front at `NavigationIOS.swift:351`). That disqualifies the overlay and does not disqualify a dialog inside it: the sheet is not that dialog’s sibling, and the overlay is not `accessibilityElementsHidden`. The inner dialog stays `accessibilityViewIsModal` and stays `announcedModal`, so VoiceOver remains in a dialog the sheet is covering. UIKit’s sibling rule then hides only the inner dialog’s siblings, not the sheet.

The new test builds this and does not catch it. After B is destroyed, C (id 8) is the innermost modal inside A. The sheet is added as a sibling of A (`AccessibilityTreeIOSTests.swift:196`). The assertion checks A only (`AccessibilityTreeIOSTests.swift:199`). C’s superview is A, so C stays exposed and `announcedModal` stays 8.

Fix: treat a candidate as not exposed when any ancestor has a visible sibling painted over that ancestor (same `zPosition`, then subview order, then frame intersection). In the test, assert `c.accessibilityViewIsModal == false` and `announcedModal == nil` while the sheet is up; remove the sheet, call `syncModal`, and assert A and C return.

### 2. Should-fix — An exiting modal still covers the one behind it

`host/apple/Sources/ExactKit/Accessibility.swift:183`

`painted` is `!isHidden && alpha > 0` and not `display: none`. `beginExit` hides the view from accessibility, clears its modal flag, and brings it to the front without changing alpha (`PresenceIOS.swift:28`). It stays a subview until `destroy`. For two overlapping modals, the one leaving is a visible sibling in front of the one that should take over, so `exposed` rejects the survivor for the whole fade. No modal flag is set, and the page is reachable while the leaving dialog still covers it. Round 2’s rule was that the survivor becomes modal as soon as the exit starts.

The exit assertion still passes because the two frames only meet at an edge: the box is `y: 100, h: 100` (`AccessibilityTreeIOSTests.swift:33`) and the front modal is `y: 200, h: 100` (`AccessibilityTreeIOSTests.swift:151`). `CGRect.intersects` is false for a shared edge, so the cover check never runs. Full-screen overlays do overlap.

Fix: ignore `accessibilityElementsHidden` views in `painted`. An exit sets that flag before `syncModal`; a sheet’s navigation view does not (the covered stack does). Add an overlapping pair to the exit test and assert the survivor’s flag is true on the exit batch, while the leaver is still at full alpha.

### 3. Nit — The focus search is not UIKit’s accessibility order

`host/apple/Sources/ExactKit/Accessibility.swift:214`

`first` walks `subviews` and returns the first view with `isAccessibilityElement` that is not `isHidden`. It keeps a `display: none` button (zero frame, `isHidden` still false) and a child with `accessibilityElementsHidden`. A paragraph whose only elements are `UIAccessibilityElement`s (`TextInteraction.swift:72`) yields `nil`, because those elements are not subviews. The tests assert `announcedModal` and never the argument passed to `.screenChanged`.

Defer. When this is picked up, take the first element from `accessibilityElements` or `accessibilityElement(at:)`, skip `accessibilityElementsHidden` and `display: none`, and assert that argument in the existing test.

Deferred, no change this round: macOS (D3); keyboard containment and Escape (§4); same-id reincarnation in one batch (not shown; ids differ across a pool rebind); making the presented navigation view itself modal (the background hide is enough once finding 1 lands).

Verdict: LAND WITH FIXES
