# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 3, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `438f19b96d`.
- **Method:** one brief (sha256 `7a3324b39d786efd87c5a3c73efb1fbeb1d61e39e4d39be1027b8203808b61c7`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Nothing blocks by inspection.** Read-only review; I did not run tests. The named feature is `fdc8ad486d` (`HEAD~3` here); I reviewed it and both fix commits.

1. **should-fix — The tab-switch test does not prove the race occurred.** [NavigationBasicsIOSTests.swift:956](/tmp/rv-hide-3/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:956). Stack depth three does not establish an active push, and `spin(0.6)` does not establish its completion. The test can pass without exercising the late inactive-stack callback.
   **Fix:** Assert that Second becomes selected while Home’s coordinator still exists, then observe Home’s transition completing before checking visibility. Reuse the pattern at [NavigationTabsIOSTests.swift:550](/tmp/rv-hide-3/host/apple/tests/ExactKitTests/NavigationTabsIOSTests.swift:550).

2. **should-fix — Animated size-class adaptation still lacks regression coverage.** [NavigationTitleIOS.swift:588](/tmp/rv-hide-3/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:588). The new completion path is untested, particularly its assumption that `nav.transitionCoordinator` is already nil. Neither added test changes width.
   **Fix:** Test compact→regular and regular→compact→regular with Chat hiding the bar, using an active resize coordinator, then pop to an intermediate route and root. Verify the completion actually reconciles visibility; if the finishing coordinator remains exposed, defer reconciliation instead of discarding it.

3. **should-fix — The ancestor-hide repair remains untested.** [NavigationBasicsIOSTests.swift:945](/tmp/rv-hide-3/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:945). `Root → Detail(shows) → Chat(hides)` tests cancellation across different wishes, but never exercises a showing route above a hiding ancestor—the earlier `followTablist` regression.
   **Fix:** Add `Root → Chat(hides) → Detail(shows)`, an ordinary subsequent update, in-place wish changes, and intermediate/root pops. Assert the ancestor keeps the bar hidden until removed.

The previous inactive-stack blocker is addressed by the selected-controller guard. The stack-prefix calculation now consistently governs the “on top already” branch and stored-hide cleanup. Ordinary root/intermediate pops, `popToRoot`, and hidden-root→visible-push restoration look consistent.

The iOS 18+/iPad/regular-width gate preserves the existing phone, compact-width and iOS 17 paths. The changed visibility test strengthens iPad coverage and retains the iPhone inset assertions.

I found no documented prohibition against animated [`setTabBarHidden`](https://developer.apple.com/documentation/uikit/uitabbarcontroller/settabbarhidden%28_%3Aanimated%3A%29) from `willShow`. The new cancellation test checks the settled result; animation tracking during the gesture remains unverified.
