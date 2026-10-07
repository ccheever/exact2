# Code review: the status bar's style from state, LLP 1105 (c3840a7b4), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, a detached worktree at `c3840a7b4`, plain output.
- **Method:** one brief (sha256 `ba26e8cd6aa8f8e1453e9536dca172c9d169c74f34c7bb5deb648b63f4d61cc7`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):** 1 taken (astra's 1). 2 taken: `sheetPresentationControllerDidChangeSelectedDetentIdentifier`, pinned by a test. 3 taken: the first covering layer from the top; every presented controller is told. 4 taken (astra's 4). 5 taken in part (astra's 7). 6 taken.

---

DO NOT LAND

1. **BLOCKING** — A presentation’s end never resolves, so the bar keeps the pre-transition style. `resolveStatusBar` returns while `modals.inTransition` (`StatusBarIOS.swift:21-22`). That flag is still set in the callbacks that are supposed to clear it. `Presentation.presenting` starts true (`ModalIOS.swift:136`) and is cleared only in the `present` completion (`ModalIOS.swift:423`), which UIKit runs after `viewDidAppear`. `viewDidAppear` calls `resolveStatusBar()` (`ModalIOS.swift:67-71`) and returns. The completion does not resolve. On a device, the batch that presents also resolves too early: `present` has already set `inTransition`, so the outermost `apply` defer (`PresenterIOS.swift:762-764`) returns too. A fullscreen or zoom route with `light-content` over a `dark-content` screen keeps dark text for as long as that route stays up. Dismiss is the same from the other side: `closeTop` appends to `retiring` before `apply` returns (`ModalIOS.swift:470`), `viewDidDisappear` bails (`ModalIOS.swift:62`), and `drainRetired`’s completion clears `retiring` without resolving (`ModalIOS.swift:481-495`). A drag-dismiss does `closeTop` plus a synchronous Back batch while `retiring` is still non-empty (`ModalIOS.swift:517-530`). `settled: true` bypasses only `navigation.transitioning`, which is why a pop, a cancelled swipe, and a push landing do resolve (`NavigationIOS.swift:766-767`). Zoom uses this same present and dismiss path.

**Fix:** Resolve after the flag drops. Call `resolveStatusBar()` in the `present` completion after `layer.presenting = false`, and in `drainRetired`’s completion after `retiring.removeAll`. Drop the calls in `viewDidAppear` and `viewDidDisappear`, or they keep hitting the guard.

2. **BLOCKING** — A sheet detent drag never re-resolves. The delegate method is `sheetPresentationControllerDidChangeSelectedDetent` (`ModalIOS.swift:552-555`). UIKit calls `sheetPresentationControllerDidChangeSelectedDetentIdentifier` (`UISheetPresentationController.h`). The optional method does not match, so it is never invoked. `presentationController?.delegate = self` (`ModalIOS.swift:417`) would deliver it if the name matched. Dragging an iPhone sheet from `medium` to `large` moves the bar onto the sheet and leaves the style of the screen behind until some later batch. Programmatic detent changes still go through `apply`.

**Fix:** Rename the method to `sheetPresentationControllerDidChangeSelectedDetentIdentifier(_:)`.

3. **MATERIAL** — Only the top presented layer is in scope, and only that layer is invalidated. `statusBarRoute` returns nil as soon as `layers.last` does not cover (`ModalIOS.swift:230-236`), and `inScope` then drops every presented route (`StatusBarIOS.swift:53-56`). A medium sheet over a fullscreen lightbox therefore resolves to the screen behind the lightbox, which is not under the bar. `statusBarChanged` notifies only `layers.last` (`ModalIOS.swift:239`). A `.pageSheet` does not capture the bar, so UIKit is asking the `.overFullScreen` controller underneath, which is never told to re-read.

**Fix:** Walk `layers` from the top and take the first route that covers (`.overFullScreen`, or a compact-width sheet at `large`). Send `setNeedsStatusBarAppearanceUpdate` to the root and to every live `ModalController`.

4. **MATERIAL** — A context-preview row counts as showing. `shows` allows a hidden navigation header in `controller.lifted` and otherwise treats “in a window and not hidden” as showing (`StatusBarIOS.swift:39-51`). A preview row is reparented onto `PreviewController` (`ContextMenusIOS.swift:256-263`): it stays in a window, it is not hidden, and it is not inside a presented route, so `inScope` accepts it. A row that declares `status-bar-style` changes the bar for the life of the menu. D3 excludes that row.

**Fix:** Reject a node that is not inside the viewport or a modal route. The preview’s host view is neither.

5. **MATERIAL** — The tests pin the steady state and miss the paths above. `testAScrollThresholdFlipChangesTheBarInItsOwnBatch` (`StatusBarIOSTests.swift:84-102`) fails if the flip waits until after `apply`, and the paint, hidden, and `auto` cases match D3 and D4. Nothing presents a controller, drives a transition, changes a detent, or reads `preferredStatusBarStyle`. The wrong detent selector, the in-flight early return, and invalidating only `layers.last` all leave this file green. `fade` is checked as a flag, not as the `setNeedsStatusBarAppearanceUpdate` call sitting inside the 0.3 s animation block (`StatusBarIOS.swift:30`).

**Fix:** One test that presents an `.overFullScreen` controller, runs the appearance completion, and sees `preferredStatusBarStyle` change on that controller in that turn. One that dismisses it and sees the screen behind come back. One that calls the real detent selector. One where a non-covering sheet over a fullscreen leaves the fullscreen’s style on the controller UIKit asks.

6. **MINOR** — A bound value outside the set is dropped with no log. Literals are refused (`values.rs:884-900`). At runtime the candidate filter (`StatusBarIOS.swift:57`) skips anything else and does not log, which D1 requires.

**Fix:** Log the node id and the string once when a declared value is not `light-content`, `dark-content`, or `auto`, then treat it as unset.

The steady state matches D2–D4 for a single screen: descendant over ancestor, later sibling over a deeper earlier branch, `display: none` and `visibility: hidden` excluded, projected navigation headers included, `auto` from the winner’s `overrideUserInterfaceStyle` or else `.default`. With no declaration, `carrying` is empty (`ChromeIndex.swift:11`, `StatusBarIOS.swift:35-36`) and the batch does not call UIKit. When a resolve does run, the root and the top modal are updated in that turn, and `fade` wraps only that update. Web and JS skip both props (`element.rs:821-822`, `emit.rs:1151-1157`). The overrides are inside `#if os(iOS)`; tvOS and macOS still compile. This diff does not touch the idle-tick path, so it does not reopen those skips. The new early return is the same shape of hole: the catch-up runs while the guard is still true, and nothing runs after it. Navigation `didShow(settled: true)` is the catch-up that actually lands. File size, comment density, and `rules/RULES.md` are fine.
