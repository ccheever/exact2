# Code review: DOM's `scrollend` round 6, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `afefd66bf9e9d7d4c8555dc96ca67bc240d715024af54e88867d9b3fea76075d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** The one MINOR (both reviewers: a negative no-op boot restore on the JS target) taken in the landing commit, as the reviewers suggested, without a seventh round.

---

## MINOR

**`host/web-js/rt.js:240`** — A boot write with a negative target still arms `$bootScroll` and `$bootScrollEnd` when the scroller will not move. `at < 0` sets both flags before the clamp test runs. The browser then clamps the write onto the offset it already has (a `scrollTop` of `-1` while resting at `0`, or an RTL `scrollLeft` past the minimum while already there) and fires no `scroll`, so the flags stay set. The next gesture's first `scroll` is dropped, and that drop returns before clearing `$bootScrollEnd`, so a gesture that is one `scroll` plus one `scrollend` (one wheel notch, one `scrollTo`) drops the end too. A fling still ends: the second `scroll` clears the flag and `scrollend` runs. The positive overshoot from round 5 (target past the max, already sitting on that max) is handled. Set the flags only when the value the browser will clamp to differs from the current offset: `0..max` for `scrollTop` and LTR `scrollLeft`, `-max..0` for RTL `scrollLeft`.

## Checked

Round 5's iOS catch is in place. `scrollViewDidEndDecelerating` and `scrollViewDidEndScrollingAnimation` go through `scrollMayHaveEnded`, which settles while `isTracking` or `isDragging` and ends when the touch lifts (`scrollViewDidEndDragging` with `willDecelerate: false`) or once the quiet check sees the scroll still. A follow of the end that starts an animation ends with that animation.

`scrollViewDidEndScrollingAnimation` still returns early while an `OffsetDriver` or an owed start correction is running. Those frames are not dragging or decelerating, so they take the 100 ms quiet path, which keeps rescheduling while the driver exists and fires once it is gone. A UIKit animated scroll that is not a driver ends in `scrollMayHaveEnded`.

Ordering holds. `endScroll` flushes `sendScrollEvent` before `presenter.scrollend`. On a user drag or coast the last `scroll` was already sent synchronously from `scrollViewDidScroll`. A programmatic move queues its `scroll` before the end is scheduled, so that delivery runs first and the flush is a no-op when the offset was already sent. `Presenter.send` queues `scrollend` behind a batch that is still applying.

The quiet item is cancelled by the next `settle` or by `hold()` when a drag or a live scroll starts. iOS captures `incarnation` and both platforms' `endScroll` require `presenter.views[id] === self`. Parking or rebinding a pooled iOS view calls `ScrollEnd.reset()`, which cancels the item. On the Mac, `observeLiveScroll` removes the start and end observers for the scroll view it replaces and resets what that view owed.

A `scrollend` handler alone calls `needScroll`, so the iOS scroll view exists. `scrollWaits` stays true for a swipe row that has neither `scroll` nor `scrollend`, and `GroupedListIOS` forwards the end callbacks to the owner.

`scrollend` is appended after `toggle` in `EventKind`, so earlier ordinals stay put and the plan digest changes. The three embedded plans (`NativeFieldsIOSTests`, `InlineFrameMacTests`, `WebDocumentsTests`) carry the same new digest. Kind 51 has no payload, and a node without the handler commits nothing.

AppKit's `didEndLiveScrollNotification` is still taken on faith for momentum. If that notification arrives after the coast, the code ends the scroll once, on the next turn, after a knob drag puts a refused origin back. A wheel's notches and a programmatic jump end once the origin has been still for 100 ms.

Reviewed `04f5c0b7e` against `origin/main`: 37 files, +458/−48. Notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-2cca7f0d.md` and `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-2cca7f0d.md`.
