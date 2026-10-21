# Code review: the dev menu opens on a phone and over a presented controller, round 2 (042c8b48c), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `042c8b48c`.
- **Method:** one brief (sha256 `dc2171a2c3fe4dfc37d5214072b775fca7523de2b6b8b8fe7720d61916a1dbf4`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: the presentation waiting is remembered (`DevMenu.waiting`), and a toggle while the menu waits cancels it. 2 taken in part: the test now drives `reload()` and `toggle()` through the mock presenter (a reload drops a waiting one; toggle, toggle cancels; a third toggle opens). Real UIKit presentation, the popover anchor and the action chain stay unasserted.

---

LAND WITH CHANGES

1. **MINOR — A second toggle cannot cancel a queued opening.** At [DevMenuIOS.swift:143](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:143), a waiting sheet has no `presentingViewController`, so another toggle calls `show()` again. Press ⌘D twice during an interactive dismissal: the second request replaces the first, and the menu subsequently opens. Round 1’s toggle-off disposition is incomplete. **Fix:** distinguish a queued opening from a retained, dismissed sheet; toggling the queued opening should invalidate its ticket and clear it. Test that sequence.

2. **MINOR — Tests still bypass important production behavior.** [DevMenuIOSTests.swift:58](host/apple/tests/ExactKitTests/DevMenuIOSTests.swift:58) checks journal lines without asserting presentation; replacing `show()` with a no-op still passes. The cancellation case at line 106 increments `pending` itself, so deleting cancellation from `reload()` also passes. **Fix:** exercise `toggle()` and `reload()` through the mock presenter and assert their effects. Add action-chain, retained-dismissed-sheet, empty-event and iPad-anchor assertions; real UIKit presentation remains unverified.

The other round-1 correctness fixes hold: hold/tap failure dependencies and hold cancellation are present at [DevMenuIOS.swift:75](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:75); all four flows use the bounded presentation retry, including dismissing children. Popover coordinates use the selected presenter’s view. The baked-reload breadcrumb loss remains as acknowledged in the disposition.

Simultaneous recognition still permits app gesture callbacks alongside the shortcut; cancelling view touches cannot undo those callbacks. The hold now cancels touches, contrary to the original brief. The nil/empty-touch-event guard remains shared by all three recognizers. The touch runner injects single-finger input, which cannot satisfy their four-touch requirement. tvOS retains its availability guards and excludes remote presses.

Other possible phone causes remain timing/movement outside tap tolerances, fingers landing in different windows, system gesture interception, or disabled installation. [VoiceOver uses four-finger taps](https://support.apple.com/en-gb/guide/iphone/iph3e2e2281/ios); [`isMultipleTouchEnabled=false` does not restrict recognizers](https://developer.apple.com/documentation/uikit/uiview/ismultipletouchenabled).

There is **no batch-skip widening in this diff**. The idle-tick repairs remain, including [sequence accounting](host/apple/Sources/ExactKit/Session.swift:964) and [projection refresh ordering](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:24). No October 5 dev-menu review exists. Source files are 257 and 112 lines; no additional rules or dead-code issue found. `git diff --check` passed. UIKit/device tests were not run in this read-only review.