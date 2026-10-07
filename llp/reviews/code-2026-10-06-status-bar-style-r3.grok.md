# Code review: the status bar's style from state, LLP 1105, round 3 (7e3b9a827), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, a detached worktree at `7e3b9a827`, plain output.
- **Method:** one brief (sha256 `0fb4874a5337132cc7b5c43c2368e37ee9b0c1d53f1b29bb172ee6e4ca82bde6`), shared with astra. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken as proposed: `resolveStatusBar(settled: true)` from the reappearance, at once when `animate` returns false.

---

LAND WITH CHANGES

1. **MATERIAL** — A cancelled sheet drag or zoom dismiss still leaves the bar on the pre-gesture style. `viewDidAppear` is the reappearance after that cancel (`ModalIOS.swift:66-76`). When `transitionCoordinator` is set it only schedules `resolveStatusBar()` from `animate(alongsideTransition:)`'s completion, and it ignores the `Bool` that method returns. That completion runs while the coordinator is still installed (UIKit releases it after the last callback). `resolveStatusBar()` then returns immediately (`StatusBarIOS.swift:22-23`) because `navigation.transitioning` is any presented navigation whose `transitionCoordinator` is non-nil (`NavigationIOS.swift:554-557`), and the sheet's navigation controller is still in `presentedNavigations` and is a child of the dismissing controller. `didShow` already passes `settled: true` for this winding-down coordinator (`NavigationIOS.swift:770`); the reappearance path does not. If `animate` returns false, which is the same window the zoom alpha path refuses to trust (`ModalIOS.swift:459-460`), the completion is not guaranteed and the `else` branch is skipped, so nothing resolves. Concrete input: a large sheet (or zoom) showing `light-content`, a props batch sets the header to `dark-content` while `isBeingDismissed` is set, the user lets go. The header is dark; the root and the `ModalController` keep returning `.lightContent` until some later full `apply`. `StatusBarIOSTests` never presents or cancels, so this stays green.

**Fix:** From that reappearance, call `resolveStatusBar(settled: true)` so only a newly started push or pop holds the style, and if `animate` returns false run that call immediately, as the zoom path does when `!taken`.
