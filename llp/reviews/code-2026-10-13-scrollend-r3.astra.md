# Code review: DOM's `scrollend` round 3, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `4d2c32505357252911c5487a4d80a36011ee6d679d18c888d51f953b39e0a83a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 4's code.

---

- **MAJOR — [ScrollEndIOS.swift:27](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:27): the quiet callback ignores pending animations.** An offset change arms the 100 ms timer. If a smooth correction starts just before expiry, `OffsetDriver.start()` schedules its first display frame without cancelling that timer. The quiet callback can therefore call `endScroll()` while the correction still has pending movement. Subsequent frames re-arm another end. The `offsetDrivers`/`startOwed` guard in `scrollViewDidEndScrollingAnimation` does not protect this path. Reschedule the quiet check while a correction or UIKit scroll animation is pending. This violates the requirement that completion have no pending position updates. [CSSOM View](https://www.w3.org/TR/cssom-view/#scrolling)

This finding is based on the source trace; I could not reproduce the UIKit interleaving in this read-only environment.

The other reviewed paths look consistent: compiler and ABI wiring, digest change, FIFO delivery, incarnation checks, Mac observer replacement, swipe-row allocation, and JS listener retirement. Existing binaries passed two Contract tests, one emitter test, and three Mac tests; the JS listener lifetime probe also passed. The full iOS failures remain unbaselined.

**bsky5’s ordinary fling use is served:** set `flinging` on `scroll`, then clear it and snap on `scrollend`, removing the per-frame timer-key writes. The animation gap above still permits an early clear/snap during programmatic scrolling.

**NOT READY**
