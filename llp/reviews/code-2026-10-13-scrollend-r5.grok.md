# Code review: DOM's `scrollend` round 5, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `ba7babde117f6c9b577945d8975cfe5e8d11b1a71b9aea3c2a9d5325889faa4a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 6's code.

---

One major defect: on iOS, catching a fling sends `scrollend` while the finger is still down. That is the gesture the clone snaps on.

## MAJOR

`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:659` and `:667`. `scrollViewDidEndDecelerating` and `scrollViewDidEndScrollingAnimation` call `endScroll()` with no look at `isTracking`. UIKit ends deceleration by calling `scrollViewDidEndDecelerating` when a finger lands on a moving scroller, and at that moment `isTracking` is already true and `isDragging` is still false. `endScroll` then `take()`s and delivers `scrollend` before the finger lifts. The same call is what a touch that cancels a UIKit `setContentOffset(animated:)` hits if `scrollViewDidEndScrollingAnimation` runs while that touch is down.

`host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:29` already treats that touch as not over: the quiet check reschedules while `isTracking || isDragging || isDecelerating`. The delegate ends never ask it. `scrollViewDidEndDragging` with `willDecelerate: false` (`NodeViewIOS.swift:657`) is the lift with nothing thrown, and that one is the right moment.

DOM `scrollend` waits until the touch is released and nothing is still animating. Chrome's condition is both: translation finished, and the finger up. The clone sets `flinging` on `scroll` and, on `scrollend`, clears it and snaps the header. On this catch the snap runs under the finger. A hold that never becomes a drag gets no second end, because `take()` already consumed the owed one. `NativeListScrollIOSTests.testAScrollEndsOnceAfterItsLastScroll` calls `scrollViewDidEndDecelerating` itself after an unanimated `setContentOffset`, so `isTracking` is false and the test does not see this.

The fix is local: on those two delegate ends, if `isTracking` (and on the animation end, if `isDragging` as well), call `settleScrollEnd()` instead of `endScroll()`.

## Checked, no other findings

Kind 51 is the same value on every supported dispatch path: `runner/src/runner/host_kinds.rs:49`, `host/apple/src/abi.rs:1027`, `host/web/src/abi.rs:534`, `host/apple/Sources/ExactKit/Bridge.swift:329`, `host/web/glue.js:579`, LLP 1005, and `contract/cli/tests/it/scrollend.rs:82`. Linux and Android are the documented gap.

The Mac scrollbar fix holds. `KnobDrag.ended` (`CollectionMac.swift:113`) puts the origin back synchronously inside `didEndLiveScrollNotification`. `liveScrollEnded` (`ScrollEndMac.swift:33`) sends the end on the next main-queue turn, so the flushed `scroll` and the `scrollend` both see the put-back origin, whichever observer runs first. Replacing or removing the scroll view removes both observers and `reset()`s.

The JS boot fix holds. A suppressed boot scroll leaves `$bootScrollEnd` set; the next delivered scroll clears it (`host/web-js/rt.js:932`), so a smooth restore that keeps moving still ends.

Web delivery matches the browser event: after the last `scroll`, once per gesture, only if the offset moved, including programmatic scrolls. Element `scrollend` does not bubble, and both glues listen on the element. `scroll` then `scrollend` stay in that order through `Presenter.send`'s waiting queue on both Apple platforms: the end flushes `sendScrollEvent` before `scrollend`, and a coalesced duplicate no-ops on the last offset. The 100 ms work item cancels on the next move, on `hold()`, and on `take()`/`reset()`. iOS pool recycle and rebind `reset()` and the settle closure checks `incarnation`. The `scrollViewDidEndScrollingAnimation` return while an `OffsetDriver` or a `startOwed` correction is running leaves the quiet check in place. `scrollWaits` / `needScroll` treat `scrollend` like `scroll`, so a swipe row without the handler is unchanged. `EventKind` is appended after `toggle`, and the NativeFields embedded plan digest is the header patch that append requires.

## bsky5

Served for a fling that coasts out, a drag that lifts with nothing thrown, a browser scroll, and a Mac live scroll: the clone can stop writing state on every frame and snap on `scrollend`. Not served for the iOS catch above.

A snap that itself moves the offset or the top inset owes another `scroll` and another `scrollend`, on the web and on these hosts. An idempotent snap is one extra cycle.

Whether AppKit's `didEndLiveScrollNotification` waits out trackpad momentum is still unproven. The header describes the end of live scroll tracking and says nothing about momentum; the tests post the notification. Legacy mice are explicitly not bracketed, so wheel notches correctly take the 100 ms path.

**NOT READY**
