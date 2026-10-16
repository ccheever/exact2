# Code review: DOM's `scrollend` round 4, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `1a4edf9aaca8a8a0ac42fa1a7364acb8c7b96aacb51d4640b59a3d1983da18c2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 5's code (with the rebase onto main: kind 51).

---

A Mac collection's scrollbar mouse-up delivers `scroll` and `scrollend` at the offset the host is about to discard. That is the only new defect. Round 3's two fixes are in the tree and hold.

## MAJOR

`host/apple/Sources/ExactKit/Mac/ScrollEndMac.swift:28` — a collection's scrollbar release runs `scrollend` on the mouse-up offset that `KnobDrag` exists to throw away.

`clipScrolled` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:592`) returns before `scrollEndMoved` and `queueScrollEvent` when `KnobDrag.admits` rejects that bounds change (`host/apple/Sources/ExactKit/Mac/CollectionMac.swift:108`). The clip is still sitting on AppKit's derived origin when `didEndLiveScrollNotification` is delivered. The node's observer is added as the scroll view is installed (`NodeViewMac.swift:1071`); `KnobDrag`'s is added later (`CollectionMac.swift:62`, from `observeKnobDrags` at `CollectionMac.swift:223`), so the node runs first. `liveScrollEnded` calls `endScroll` (`ScrollEndMac.swift:46`), `sendScrollEvent` (`NodeViewMac.swift:615`) reads that origin and dispatches it, then `scrollend` runs. `KnobDrag.ended` (`CollectionMac.swift:113`) only then puts the origin back. A handler that snaps from the event has already committed against the discarded offset, and the restoration is a second scroll whose `scrollend` follows the 100 ms quiet timer. A trackpad gesture leaves `KnobDrag.tracking` false, so this path is the scrollbar.

## What held

Round 3's iOS check is at `host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:28`: the quiet callback asks again while `collections.offsetDrivers[id]` or `startOwed` holds the view. A driver frame assigns `contentOffset` (`host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:439`) before `done` clears the driver (`CollectionIOS.swift:236`), and that assignment calls `scrollViewDidScroll`, which settles again. The early return in `scrollViewDidEndScrollingAnimation` (`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:665`) therefore leaves a quiet check armed, and the end still arrives after the correction.

Round 3's JS drop is `host/web-js/rt.js:240` and `rt.js:926`. `$bootScrollEnd` is set beside `$bootScroll` only while `Booting`, which is the boot commit (`rt.js:1399`), and the listener is attached during that commit, before `drain` assigns the offset. One later `scrollend` is dropped and the flag clears.

`scroll` and `scrollend` both go through `Presenter.send`. `endScroll` flushes `sendScrollEvent` first, which records `lastScrollEvent` before the handler, so an asynchronously queued send of that same offset does not run again after `scrollend`. A nested apply queues the two in call order. On iOS a finger fling ends in `scrollViewDidEndDecelerating`, after the last synchronous `scroll`.

The quiet item is cancelled by `hold`, `settle`, `take`, and `reset` (`ScrollEnd.swift:32`). A drag that starts during the wait takes the gesture-end path (`ScrollEndIOS.swift:19`). The iOS callback captures `incarnation`; parking sets that to 0 and `recycle` / `rebind` call `reset`, and `recycleScroll` clears the delegate around the zero offset. Mac live-scroll observers are removed with the scroll view they were added for (`ScrollEndMac.swift:11`) and the note is filtered to that scroll view. There is no second Mac scroll view installed over a live one: overflow going idle passes the old view to `observeLiveScroll(nil, replacing:)`.

`needScroll` and `scrollWaits` (`NodeViewIOS.swift:82`, `NodeViewIOS.swift:1254`) treat a `scrollend` handler the way they treat `scroll`, so the row's scroll view and delegate exist. A swipe row that already created one for `scroll=` is unchanged. `GroupedListIOS` forwards the end callbacks to the owner, and the owner's `scrollView` is the collection view, so the identity guard passes.

`scrollend` is appended after `close` in `plan/tables/format.json:759`, so existing `EventKind` discriminants stay put and the generated format digest moves. Kind 50 carries no payload. Linux and Android are the QUEUE entry, as LLP 1005 and the grammar say.

## The clone

Setting `flinging` on `scroll` and clearing it to snap the header on `scrollend` works on iOS: a fling ends once, at `scrollViewDidEndDecelerating`, after that fling's last `scroll`. On the web it is the browser's own `scrollend`, including a programmatic scroll, once that scroll's offset has stopped; a host correction that then moves the port is a new scroll with its own end. On Mac a wheel notch and a jump end once after 100 ms of stillness, and a trackpad live scroll ends at `didEndLiveScrollNotification`. A scrollbar drag on the clone's Mac list hits the bug above, so the snap runs on the discarded offset.

Whether `didEndLiveScrollNotification` waits out trackpad momentum is still not proven in this repo.

NOT READY
