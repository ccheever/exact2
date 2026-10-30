# Code review: two iOS test flakes, NavigationBar's sceneless press and FrameHealthReport's nanosecond (main red, quick review) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `c9eef96ba9`.
- **Method:** one brief (sha256 `7807e276163068ebca7553b17bf61340203a4fc09d667d40941b917796c162dd`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.

## Brief

Quick review (main-red hardening, test only) of TWO commits, `git log -p HEAD~2..HEAD` on x2/navbar-sceneless off origin/main. Read-only: no checkout, restore, stash, reset or edits.
1. b44be37ebc NavigationBarIOSTests (NavigationBarItemsIOSTests.swift): the animated bar items test failed in every full parallel iOS run and passed alone. Diagnostics from the failing run (a scratch XCTFail message): sent=false (UIApplication.sendAction's return), the BarPress target and its host present, no act queued or drained (ElementHatches.later never called), the session ready, the window key, connectedScenes.count == 0 (the parallel worker was a bare xctest process). So UIKit refused the send and nothing was called; a 30 s wait failed the same way. The fix: where sendAction returns false, call the item's own target/action (`(more.target as AnyObject?)?.perform(more.action)`), which is what UIKit's send calls (BarPress.press, NavigationBarIOS.swift ~566). With it the test passed in a full parallel suite and 5 of 5 alone.
   Check: that the fallback is faithful (the same selector on the same target; `press` takes no argument), that it can't double-press when the send succeeds, and that the sibling share test (NavigationBarIOSTests.swift ~1066, which guards on hostedWindowScene()) needs nothing.
2. c9eef96ba9 FrameHealthReportTests: the tail test failed once with gap_ns 24000001 vs 24000000; the fixture's arrivals are CACurrentMediaTime() + n*0.024 s, so the doubles' difference converts a nanosecond either side. Now asserted to within 1 ns. Check that the gap's source (host/apple/src/frame_health*.rs or FrameHealth.swift) does compute it from those doubles and that ±1 ns hides nothing real; it passed 5 of 5 on iOS.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings.

- [NavigationBarItemsIOSTests.swift:37](/tmp/iosfix2-rv-nb-astra/host/apple/tests/ExactKitTests/NavigationBarItemsIOSTests.swift:37): fallback invokes the same target and selector. `BarPress.press()` takes no arguments; successful sends skip the fallback, preventing double dispatch.
- [NavigationBarIOSTests.swift:1067](/tmp/iosfix2-rv-nb-astra/host/apple/tests/ExactKitTests/NavigationBarIOSTests.swift:1067): the share test requires a hosted window scene, so it needs no corresponding fallback.
- [FrameHealthReportTests.swift:44](/tmp/iosfix2-rv-nb-astra/host/apple/tests/ExactKitTests/FrameHealthReportTests.swift:44): Swift converts each arrival double to integer nanoseconds; Rust subtracts consecutive arrivals. The ±1 ns allowance accommodates that conversion without masking meaningful timing errors. The Rust fixture retains its exact integer-gap assertion.

Read-only source review; iOS tests were not rerun.

VERDICT: CLEAN
