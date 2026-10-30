# Code review: NavigationBasicsIOSTests' heading-subtitle test waits for the pop to settle (main red, quick review) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `098e743223`.
- **Method:** one brief (sha256 `3973df32036f0dfc37542c7dd967ed6085e1a5dc7b25b61de4b9960b3dfce9ff`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.

## Brief

Quick review (main-red hardening, test only) of ONE commit, `git show HEAD` on x2/navbasics-flake off origin/main: NavigationBasicsIOSTests.testAHeadingTheHeaderHoldsDirectlyTakesTheFirstTextAfterItAsItsSubtitle failed once in a full parallel iOS run with "No Back control" at its last assertion (Home's navigationItem.subtitle after popping Plain) and passes alone. The commit replaces a fixed `spin(0.3)` after `nav.popViewController(animated: false)` with an `until` on the settled state (one controller, no transition coordinator, !session.advanceInFlight, !Agent.nativeInFlight()) and adds the stack's titles/subtitles to the failure message. Read-only: no checkout, restore, stash, reset or edits.
Check: that the wait conditions are the right signals that the session has answered the pop (look at how the host reconciles a UIKit pop with the runtime: NavigationBar/Navigation code, RouteController, the delegate proxy), whether the until could pass before the state the assertion reads is final (e.g. the host re-pushing Plain if the runtime hasn't processed the pop yet), and whether the original failure could instead be a host bug the wait would hide (Plain's subtitle written onto Home's item). The `if #available(iOS 26.0, *)` placement. Nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings.

- The [wait at line 512](/tmp/iosfix2-rv-nv-astra/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:512) covers the unreconciled native pop, pending navigation sync, transitions and posted advances. The delegate’s Back dispatch applies the runtime response synchronously; I found no early-success window that could subsequently re-push Plain in this fixture.
- Subtitle projection targets each `RouteController`’s own item. The old failure could have read a re-pushed Plain; it does not establish that Home’s item was overwritten. The new wait does not depend on the subtitle, so corruption remaining on Home would still fail.
- The iOS 26 availability block correctly encloses both subtitle diagnostics and the assertion.

Static, read-only review; tests not rerun. The original failure’s exact cause remains unproven.

VERDICT: CLEAN
