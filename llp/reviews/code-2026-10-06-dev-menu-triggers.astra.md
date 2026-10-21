# Code review: the dev menu opens on a phone and over a presented controller (68de9e396), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `68de9e396`.
- **Method:** one brief (sha256 `6cce621e7395582d31d22e79aacc063f225f2137abe8c06d88d06102cd79bb28`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken: one `DevMenu.present` for the sheet, Save Trace's alert, the share sheet and Open Project; it waits while the presenter is presenting, dismissing or still holds a dismissing child, re-resolving the presenter each try, up to eight tries, and journals the wait and a give-up. 2 taken: one pending presentation at a time (a ticket); a reload or closing the menu drops it. 3 not taken: a baked-plan reload replaces the journal, so the double tap's line is lost there; the tap and hold lines, which are what a trace needs to show, survive. 4 taken in part: the hold's `.began`-only toggle, its duration, the recognizers never being simultaneous with each other, and the wait-then-present over a dismissing child (with a mock presenter: a real presentation never landed in the test host) are tested.

---

LAND WITH CHANGES

1. **MATERIAL — Dismissal bypasses the presentation retry.** [DevMenuIOS.swift:103](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:103) skips a dismissing child; `show()` then checks the parent’s flags at line 116. During an interactive modal dismissal, that parent still has a presented controller but neither flag is set: the menu attempts presentation instead of waiting and can still disappear silently. Save Trace, Share and Open Project also bypass the retry entirely. **Fix:** centralize presentation, wait for the active presentation/dismissal transition, then resolve the presenter again. Cover all four flows.

2. **MINOR — A pending open survives reload.** The callback at [DevMenuIOS.swift:118](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:118) checks only whether a sheet exists. Trigger the menu during a presentation, then reload before the retry: `reload()` dismisses the current sheet but the queued callback subsequently opens another. Repeated triggers also queue independent retries. **Fix:** keep one cancellable pending request and invalidate it on reload or toggle-off.

3. **MINOR — The double-tap breadcrumb is lost on a baked-plan reload.** [DevMenuIOS.swift:25](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:25) logs before reloading. The fresh boot replaces the host at [abi.rs:506](host/apple/src/abi.rs:506), including its journal. Saving a trace afterward therefore cannot show that trigger. **Fix:** record the reload reason in the replacement runner after successful restart; test that sequence.

4. **MINOR — Neither test proves presentation succeeds.** [DevMenuIOSTests.swift:25](host/apple/tests/ExactKitTests/DevMenuIOSTests.swift:25) silently tolerates timeout; the second test only traverses mocked pointers. Both pass if `show()` does nothing. **Fix:** assert the awaited condition and actually present over a modal. Add transition/retry, retained-dismissed-sheet and action-chain cases; assert the hold duration, `.began`-only trigger, reload exclusivity and empty-event rejection.

The gesture change permits app recognizer callbacks to run alongside the shortcut; cancelling view touches does not undo those callbacks. The taps retain cancellation, the hold disables it, and `.began` prevents repeated hold callbacks. The single-tap/reload failure dependency and nil-event guard remain intact ([DevMenuIOS.swift:19](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:19), lines 34–38, 74–88). Single-finger injected touches cannot meet the four-finger requirement. iPad anchors use the selected presenter’s view coordinates; tvOS availability guards remain intact.

Other possible phone causes include finger timing/movement, touches landing in another window, disabled installation, and accessibility interception—[VoiceOver assigns four-finger taps](https://support.apple.com/guide/iphone/use-voiceover-gestures-iph3e2e2281/ios). `isMultipleTouchEnabled = false` does **not** suppress recognizer multitouch ([Apple documentation](https://developer.apple.com/documentation/uikit/uiview/ismultipletouchenabled)). The reported device cause remains unproven.

No batch-skip code changed; the earlier idle-tick fixes remain present. No prior dev-menu review files were found. Both files meet the size limit; `git diff --check` passed. UIKit tests were not run in the read-only environment.