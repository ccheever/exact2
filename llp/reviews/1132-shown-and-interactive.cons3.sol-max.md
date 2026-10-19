# LLP 1132 — consensus round 3 by Sol (gpt-6.1-sol, max), of 6a4804a897, 2026-10-17

No critical findings. The `replace` correction and removal of naming-process history resolve those findings. One major issue remains in the hidden-route rewrite.

1. **Major — D1, Visits: UIKit’s configured presentation style does not establish that the presenter remains drawn.**

   [D1:56](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:56) says UIKit keeps drawing beneath its sheets, and that “No host stops drawing a screen beneath a dialog.”

   The cited [ModalIOS.swift:29](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:29) establishes `.overFullScreen` for explicit fullscreen routes, but ordinary sheets use `.pageSheet`. Dialog sheets also use [`.pageSheet`](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/DialogSheetIOS.swift:34). UIKit can adapt those sheets to fullscreen. The host explicitly recognizes this: [NavigationIOS.swift:57](/Users/admin/Developer/observe-lab/exact2/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:57) says a sheet adapted to fullscreen stops leaving its presenter shown, and `viewDidDisappear` clears `appeared`.

   **Scenario:** previously shown A has a pending mark. A sheet adapts to fullscreen and removes A from presentation. At 300 ms, A’s expression becomes true and its text changes. The actual-drawing definition makes A hidden, so D2 records 300 ms. The blanket UIKit assertion instead directs a presentation wait, producing a later time or waiting until dismissal.

   **Fix:** qualify the UIKit and dialog assertions by their actual presentation state. Retain the `.overFullScreen` example, but explicitly include adapted fullscreen sheets in the existing hidden-visit rule. Reuse the host’s appearance information and add this case to the covered-screen test. This requires no pause rule or general occlusion detector.

2. **Minor, nonblocking — D1, Visits: the stacked-sheet summary drops the web split-view exception.**

   [D1:56](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:56) first preserves the web’s split-view first route beneath nonfullscreen sheets, then says two stacked sheets hide the routes beneath the lower one.

   With split-list A, modal B and modal C, [navigation.js:316](/Users/admin/Developer/observe-lab/exact2/host/web/navigation.js:316) still preserves A: `split > 0 && index === 0` applies independently of the immediate predecessor.

   **Fix:** qualify the stacked-sheet sentence with the already stated split-list exception. This merits a sentence correction, not another rule.

3. **Minor, nonblocking — Tests: scope “at once” to a screen the host has hidden.**

   [Tests:453](/Users/admin/Developer/observe-lab/exact2/llp/1132-shown-and-interactive.rfc.md:453) ends with an unconditional statement that a readiness change gives the covered screen an interactive time “at once,” including a pixel change. Under the revised UIKit example, a retained, drawn presenter instead waits for its changed frame—316 ms in scenario 3.

   **Fix:** begin that sentence, “While the screen is hidden, …”. The existing frame rule already covers the drawn presenter.

NOT READY