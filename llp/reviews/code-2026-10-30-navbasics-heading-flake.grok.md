# Code review: NavigationBasicsIOSTests' heading-subtitle test waits for the pop to settle (main red, quick review) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `098e743223`.
- **Method:** one brief (sha256 `3973df32036f0dfc37542c7dd967ed6085e1a5dc7b25b61de4b9960b3dfce9ff`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.

## Brief

Quick review (main-red hardening, test only) of ONE commit, `git show HEAD` on x2/navbasics-flake off origin/main: NavigationBasicsIOSTests.testAHeadingTheHeaderHoldsDirectlyTakesTheFirstTextAfterItAsItsSubtitle failed once in a full parallel iOS run with "No Back control" at its last assertion (Home's navigationItem.subtitle after popping Plain) and passes alone. The commit replaces a fixed `spin(0.3)` after `nav.popViewController(animated: false)` with an `until` on the settled state (one controller, no transition coordinator, !session.advanceInFlight, !Agent.nativeInFlight()) and adds the stack's titles/subtitles to the failure message. Read-only: no checkout, restore, stash, reset or edits.
Check: that the wait conditions are the right signals that the session has answered the pop (look at how the host reconciles a UIKit pop with the runtime: NavigationBar/Navigation code, RouteController, the delegate proxy), whether the until could pass before the state the assertion reads is final (e.g. the host re-pushing Plain if the runtime hasn't processed the pop yet), and whether the original failure could instead be a host bug the wait would hide (Plain's subtitle written onto Home's item). The `if #available(iOS 26.0, *)` placement. Nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No findings.

`popViewController(animated: false)` updates the stack and calls the delegate proxy's `willShow`/`didShow` before it returns. With no coordinator, `NavigationHost.didShow` runs `transitionEnded` immediately: `recordPop` shortens the written stack, then `goBack` → `hostBack` → `apply` → `batchApplied` → `sync`. That sync sees the post-pop tree, so it projects Home and does not put Plain back. If a coordinator is still up, `didShow` returns early and the same `transitionEnded` runs from the coordinator completion registered in `willShow`.

The `until` condition stays false across that gap. `nativeInFlight()` is `navigation.inTransition`, which is `changing` (`nativePopPending` or a push/pop coordinator) or `pendingSync`. `nativePopPending` is the shorter prefix of the last written stack, so it is true from UIKit's pop until `recordPop`. A sync that bails because a transition is still running sets `pendingSync` and does not call `setViewControllers`. `advanceInFlight` covers a posted advance, which is the only later batch that could project the bar again. There is no yield between `recordPop` and `hostBack` where the count is 1, the coordinator is nil, and nothing is in flight while a re-push is still queued.

Plain's subtitle is not written onto Home. `richTitle` sets `c.navigationItem.subtitle` from that route's own header, and Plain's "No Back control" text lives only on Plain. The old failure is what `XCTAssertNil` prints when `topViewController` is still Plain: on iOS 26 Plain's `titleView` is nil, so the `HeaderTitleView` check passes and the subtitle check reports "No Back control". A stuck cross-write would still fail this assertion after the wait. The new message lists each controller's title and subtitle inside the same `if #available(iOS 26.0, *)` that guards `.subtitle`, which is the availability the rest of the host uses. The title-view check stays outside it. `git show HEAD` is that hunk only.

VERDICT: CLEAN
