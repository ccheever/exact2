# Code review: two iOS test flakes, NavigationBar's sceneless press and FrameHealthReport's nanosecond (main red, quick review) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `c9eef96ba9`.
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

No findings.

**b44be37ebc.** The fallback calls the same target with the same selector, and only when `sendAction` returns false. `more-home` is a plain item (`bar-share.contract`), built with `target: press` and `action: #selector(BarPress.press)` at `NavigationBarIOS.swift:1368-1378`. `BarPress.press` takes no argument (`NavigationBarIOS.swift:566`), and `perform(more.action)` at `NavigationBarItemsIOSTests.swift:37-39` is that zero-argument send. `sendAction` runs first and returns only after the call, so a true result never reaches `perform`. The share test still needs nothing: `NavigationBarIOSTests.swift:1067` returns before `sendAction` at line 1075 unless `hostedWindowScene()` finds a connected `UIWindowScene`, which is the condition the bare worker lacked (`connectedScenes.count == 0`).

**c9eef96ba9.** The asserted `gap_ns` is the integer difference of those doubles. The fixture stores `CACurrentMediaTime() + n * 0.024` (`FrameHealthReportTests.swift:22-24`); `FrameHealth.nanoseconds` truncates `seconds * 1e9` (`FrameHealth.swift:214-216`); `history.rs:188-191` sets `gap` to `arrival - previous`, copied unchanged into `gap_ns` (`history.rs:220`) and the report column (`FrameHealthReport.swift:43`). `0.024 * 1e9` is exactly `24000000`. For a clock below `2^23` seconds (about 97 days), that difference is only `23999999`, `24000000`, or `24000001`, so `accuracy: 1` at `FrameHealthReportTests.swift:44` is that residue. An 8 ms period, a dropped sample, or a 48 ms step is millions of nanoseconds away and still fails.

VERDICT: CLEAN
