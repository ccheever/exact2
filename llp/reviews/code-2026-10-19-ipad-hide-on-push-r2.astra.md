# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `805aed1da4`.
- **Method:** one brief (sha256 `fd7762685714e581950d0619df8f118dd98ca319dd67c6bb4c94d38c202d3205`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**One blocker remains.** Read-only review; I did not run tests.

1. **blocker — An inactive tab’s transition can overwrite the selected tab’s visibility.** [NavigationBarIOS.swift:354](/tmp/rv-hide-2/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:354), [NavigationTitleIOS.swift:565](/tmp/rv-hide-2/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:565).

   Start pushing Chat in Home, then select Second before the push finishes. Second’s root restores the shared bar and records `tablistRoot = Second`. Home’s late `didShow(Chat)` now hides it again: `followFloatingBar` checks membership in the tab controller, but not selection. Subsequent updates to Second leave it hidden because its root’s wish and arrival are unchanged. `transitionEnded` already rejects inactive stacks; this new call runs before that guard.

   **Fix:** Require `tabs.selectedViewController === nav` before `followFloatingBar` mutates visibility or `tablistHidBar`. Extend the existing mid-push tab-selection test to push Chat and assert Second’s bar remains visible after Home’s transition completes, then return to Chat and assert hidden.

2. **should-fix — Size-change reconciliation is discarded when a coordinator exists.** [NavigationTitleIOS.swift:575](/tmp/rv-hide-2/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:575), called by [NavigationTabsIOS.swift:93](/tmp/rv-hide-2/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:93).

   Push Chat while compact, then expand to regular width during an animated resize. The trait callback calls `refollowFloatingBar`, but its blanket `transitionCoordinator == nil` guard also rejects resize/rotation coordinators. Nothing retries after that coordinator finishes: resizing produces no navigation `didShow`, and Chat’s unchanged cached wish bypasses the “on top already” branch. The floating bar can therefore remain visible.

   **Fix:** Reconcile the currently selected stack after the size transition completes, or distinguish size transitions from push/pop transitions. Test compact→regular and regular→compact→regular while hidden, followed by intermediate/root pops. A trait override without an active coordinator would miss this case.

3. **should-fix — The ancestor and cancellation repairs still lack regression coverage.** [NavigationBasicsIOSTests.swift:352](/tmp/rv-hide-2/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:352).

   The added switch test covers settled Chat→Second→Chat. The existing interactive test checks the navigation bar between Photo and Chat, which both hide tabs; it cannot prove tab-bar rollback across different wishes.

   **Fix:** Add `Root → Detail(shows) → Chat(hides)` with a real cancelled interactive pop, then a completed pop. Also cover `Root → Chat(hides) → Detail(shows)`, an ordinary subsequent update, in-place wish changes, and intermediate/root pops.

The revised test meaningfully checks disappearance/restoration on iPad and retains the iPhone inset assertions. By inspection, the ancestor correction, ordinary root restoration/pop-to-root, and hidden-root→visible-push paths now look consistent. The iOS 18+/pad/regular gate preserves the existing phone, compact-width and iOS 17 paths. I found no separate API-use blocker in calling animated `setTabBarHidden` from `willShow`; interactive animation timing remains unverified.
