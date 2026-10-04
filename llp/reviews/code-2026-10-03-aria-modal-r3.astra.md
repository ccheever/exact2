# Code review, round 3 (final): aria-modal r3 (d212f7652), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `f3e676311`.
- **Method:** one brief (sha256 `5b7c113dd67b385fe682e817e49757c0d723abd10382bd71fe55994be88eca9a`), shared with grok. Round 3, the last fix round, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r4, landed):**
  - 1 (overlapping exits): taken. A leaving, accessibility-hidden view covers nothing, and the exit test now overlaps. A cover must be content: a node, or a controller's view (a sheet's stack). The host's own helper layers in the viewport otherwise covered every modal in the fixture.
  - 2 (nested modals under a covered root overlay): taken for the authored side. A cover on any ancestor disqualifies, and the test asserts C as well. Making a presented stack itself the boundary is not done: `ModalIOS` hides the background, and the overlay's flags now clear.
  - 3 (the focus helper): taken. The post passes nil, and naming an element is DEFERRED (LLP 1080.003 §4).
  - 4 (reloads alias ids): taken. The key is id plus incarnation.
  - 5 (agent vs presenter across branches): taken. The agent keeps the deepest modal, the later of equals.
  - 6 (the post untested): taken in part. A test counts posts for a change, for an unchanged batch and for a destroy with no reference kept. The hold during a real transition is DEFERRED; the bare presenter has no transitions.

---

Static review only; no files changed or tests run. No Blocker found, but the following Should-fixes remain before landing.

1. **Should-fix — overlapping exit animations disable the surviving modal.**  
   [Accessibility.swift:183](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:183) treats an exiting, accessibility-hidden view as a covering sibling. [PresenceIOS.swift:28](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenceIOS.swift:28) deliberately leaves that view visible and brings it forward. With overlapping modal siblings, exiting the front modal therefore disqualifies the modal underneath, exposing its background until the exit finishes.

   **Fix:** exclude retiring views from cover exclusion. Add an overlapping-siblings exit test; the existing test places the two modals in disjoint rectangles at [AccessibilityTreeIOSTests.swift:151](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:151), so it misses this regression.

2. **Should-fix — native sheets still lack complete exclusion of root overlays and their nested modals.**  
   [Accessibility.swift:191](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:191) checks only the candidate’s immediate siblings. In the new covering-sibling test, covering A clears A’s flag, but C inside A remains modal and `announcedModal` remains `8`. The assertion at [AccessibilityTreeIOSTests.swift:199](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:199) checks only A.

   Clearing A’s modal flag also does **not** hide A’s accessible children. [ModalIOS.swift:265](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:265) hides the previous navigation controller, while root overlays travel with the viewport into the sheet at [ModalIOS.swift:288](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:288). Those overlays remain inside the presented accessibility hierarchy.

   **Fix:** establish the presented navigation view as an accessibility-modal boundary, or explicitly hide and later restore the root branches outside it. Resolve authored candidates against that boundary, including descendants of excluded branches. Test a real native sheet over a root overlay containing another modal, asserting both exclusion and the announcement target.

3. **Should-fix — the new focus helper can select an inaccessible element.**  
   [Accessibility.swift:214](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:214) ignores `accessibilityElementsHidden`, inertness and `display: none`. It also descends into an `isHidden` container. Thus an inaccessible button before the visible close button can become the notification argument. Walking only `subviews` also bypasses declared accessibility elements.

   **Fix:** the smallest final-round fix is to pass `nil` and defer explicit first-element selection. Otherwise enumerate the exposed accessibility hierarchy and prune hidden branches before descending. Apple describes the optional argument as the accessibility element to receive focus. [Apple documentation](https://developer.apple.com/documentation/accessibility/accessibilitynotification/screenchanged). Test the actual argument with a hidden first child.

4. **Should-fix — numeric notification identity survives a restart and aliases the replacement modal.**  
   [Accessibility.swift:211](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:211) compares only the wire ID, while [PresenterIOS.swift:298](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:298) replaces every view without resetting or invalidating that identity. Reloading a plan whose modal receives the same ID therefore skips the announcement despite replacing the focused element.

   **Fix:** retain an immutable identity containing the existing `NodeView.incarnation`, which changes on construction and pool rebind. This preserves the destruction fix without conflating replacement views. Test reset/recreation with the same numeric ID, alongside destruction and pool reuse.

5. **Should-fix — the agent and presenter still disagree across separate modal branches.**  
   [AgentAccessibility.swift:198](/tmp/x9-review/host/apple/Sources/ExactKit/AgentAccessibility.swift:198) replaces the boundary only with a descendant of the previously encountered modal. Conversely, [Accessibility.swift:210](/tmp/x9-review/host/apple/Sources/ExactKit/Accessibility.swift:210) chooses maximum depth, then ID, across all winners.

   For an outer modal containing two wrappers, each with an inner modal, both inner flags can remain set. The presenter announces the deeper/newer one; the agent keeps the first visited branch and computes `outsideModal` against it.

   **Fix:** select the agent boundary from all exposed modal views using the same depth/ID ordering, then mark leaks. Test both the original nested-wrapper leak and two independent inner branches.

6. **Should-fix — notification and transition fixes remain untested.**  
   Assertions such as [AccessibilityTreeIOSTests.swift:135](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:135) inspect bookkeeping, not notification count or argument. The destruction test retains A, B and C at [AccessibilityTreeIOSTests.swift:182](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeIOSTests.swift:182), so it would not catch the original weak-reference failure. Its bare `Presenter` fixture exercises no native navigation transition.

   **Fix:** record notification decisions and arguments in tests. Require one close notification after destruction without retained view references, no duplicate for unchanged reconciliation, no notification during a transition, and one final notification after completion with unchanged geometry and no subsequent app update. Assert the innermost agent boundary directly.

Round-2 accounting:

| Finding | Resolution in r3 |
|---|---|
| Astra 1 / Grok 1: destruction and reuse | Ordinary destruction and rebinding to a different ID are fixed. Incarnation remains incomplete; finding 4. |
| Astra 2: accessibility-hidden ancestors | Resolved: every UIKit ancestor is checked. |
| Astra 3: deferred containment and notifications | Code paths appear resolved: deferred sync, `didShow`, presentation and dismissal reconcile again, with transition guards. Regression coverage remains missing; finding 6. |
| Astra 4: modal beneath a winning sibling | Original three-modal topology is resolved. Covering nonmodal/native branches remain incomplete; finding 2. |
| Astra 5: outer versus inner agent boundary | Resolved for one nested chain; separate branches still disagree, finding 5. |
| Astra 6: container notification argument | Partially resolved: the helper seeks an element but can choose an excluded one, finding 3. |
| Grok 2: sheet over root modal / covered stack | Covered-stack ancestor exclusion is fixed; root-overlay exclusion remains incomplete, finding 2. |
| Grok 3: schema and RFC references | Resolved. |

**DEFERRED with a note:** macOS modality, root-wrapper/route-local exclusion of native bars, keyboard containment and Escape can retain their documented deferrals. Explicit first-element focus selection can also be deferred if the notification uses `nil`. Physical VoiceOver validation remains owed; the property-tree tests do not establish it.

Verdict: LAND WITH FIXES
