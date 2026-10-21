# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fdc8ad486d`.
- **Method:** one brief (sha256 `28b223457c6d75d6d041a5c5021f2e526b3126d9a9ce1c4a66c22d72c4dfb3bb`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I would block this commit on three visibility bugs. This was a read-only review; I did not run tests.

1. **blocker — Returning to a retained tab can leave its bar visible.** [NavigationBarIOS.swift:331](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:331), [NavigationTabsIOS.swift:230](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:230). Push Chat in tab A, programmatically select tab B’s visible root, then return to A. B clears the shared hide. Returning to A changes neither its stack nor Chat’s cached `tablistHidden`, so the “on top already” branch skips it. Switching to an existing stack does not supply the push/pop callback this helper requires.
   **Fix:** Reconcile the selected stack’s effective visibility explicitly after tab selection. Add the A→B→A regression, including selection while A’s push is running.

2. **blocker — `followTablist` undoes the new ancestor-based hide.** [NavigationTitleIOS.swift:545](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:545), interacting with lines 482–494. With `Root → A(hides) → B(shows)`, `willShow` correctly hides because A remains below B. The next settled `followTablist` call sees B’s `hidden == false`, shows the bar, and clears `tablistHidBar`. Changing B’s wish in place has the same inconsistent calculation.
   **Fix:** Use the same effective stack-prefix decision in the floating-bar helper, the “on top already” branch, and the stored-hide cleanup. Test an ordinary update after pushing B, in-place changes, and popping to an intermediate route.

3. **blocker — Cancellation can bypass restoration.** [NavigationTitleIOS.swift:544](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:544). For `Root → Detail(shows) → Chat(hides)`, beginning an interactive pop shows the bar. During cancellation, UIKit can send `willShow(Chat)` while the popped source is still absent from `viewControllers`; the existing transition code documents that rollback timing at [NavigationIOS.swift:972](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:972). This guard then returns. Completion reconciles only roots, leaving Chat visible with the bar.
   **Fix:** Account for the transition’s source controller when computing the returning stack, and reconcile the actual pushed top at completion. Add a real percent-driven cancellation between routes with different tab-bar wishes; the existing Photo→Chat cancellation hides tabs on both routes.

4. **should-fix — Size-class handoff lacks coverage.** [NavigationTitleIOS.swift:543](/tmp/rv-hide-1/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:543). Width is sampled only in `willShow`; the trait observer merely refreshes tab images. Add regular→compact→regular and compact→regular tests while a pushed route hides tabs, followed by intermediate/root pops. Reconcile visibility on adaptation if UIKit recreates the bar without preserving the intended hide.

The changed [test at line 352](/tmp/rv-hide-1/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:352) preserves the iPhone visibility and inset assertions and adds meaningful disappearance/restoration checks on a regular-width iPad. Basic root restoration, router-driven pop-to-root, and a hidden root followed by a visible push look consistent by inspection.

The iOS 18 availability gate and pad/regular distinction are appropriate for the default container. I found no documented prohibition against animated `setTabBarHidden` in `willShow`; cancellation handling above is the concrete transition concern. [Apple’s API documentation](https://developer.apple.com/documentation/uikit/uitabbarcontroller/settabbarhidden(_:animated:))
