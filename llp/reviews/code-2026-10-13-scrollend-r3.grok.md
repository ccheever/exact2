# Code review: DOM's `scrollend` round 3, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4d2c32505357252911c5487a4d80a36011ee6d679d18c888d51f953b39e0a83a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 4's code.

---

## MINOR

**The JS target delivers a boot `scrollend` for the scroll it already swallows** — `host/web-js/rt.js:889`

Boot sets `$bootScroll` and assigns the restored offset in `drain` (`rt.js:240`), after `flush` has already attached listeners (`rt.js:210` then `settled`). `onScroll` (`rt.js:923`) drops that one `scroll`. The new `scrollend` listener does not look at the flag, and the browser still fires `scrollend` for that write, so the handler runs once on load with no `scroll` ahead of it. A header that snaps on `scrollend` snaps on first paint. The wasm glue drops the event only while `inputReady` is false (`host/web/glue.js:520`); a `scrollend` that arrives after `activateData` has the same hole, and that timing is not proven from this tree.

## Checked

Round 2 is in the code as described. A touch that is only tracking schedules the 100 ms check, and that check asks again while `isTracking`, `isDragging`, or `isDecelerating` (`ScrollEndIOS.swift:19`, `:27`). `endScroll` takes the node id and incarnation before `sendScrollEvent` and sends `scrollend` only if both still match (`ScrollEndIOS.swift:36-39`). The JS listener bails when a due task has disconnected the scroller or ended its owner, and removes itself at scope end (`rt.js:888-891`). A `display: none` clamp does not arm an end, and an end that runs with no layout box is discarded (`ScrollEndIOS.swift:13`, `:33`; `ScrollEndMac.swift:38`, `:47`).

Ordering through `Presenter.send` stays scroll, then `scrollend`. A gesture delivers each `scroll` immediately and flushes once more inside `endScroll` before `scrollend`. While a batch is applying, both go on the same FIFO waiting queue, and the drain skips an id whose view is gone (`PresenterIOS.swift:663-665`, `:695-696`, `:789-791`). A scroll handler that moves the offset again does it after `take()`, so the gesture's `scrollend` is not the new move's end.

The quiet timer is cancelled by `hold`, by `reset` when a pooled iOS view is recycled or rebound, and by the incarnation captured in the work item. A drag that starts during the wait is held by the next `scrollViewDidScroll` with `isDragging`. Mac live-scroll observers are removed with the scroll view they were added for, and both callbacks ignore a notification from any other view (`ScrollEndMac.swift:11-31`, `NodeViewMac.swift:1071`, `:1086`). `scrollViewDidEndScrollingAnimation` returns early while an `OffsetDriver` or a owed start correction is running (`NodeViewIOS.swift:665`); those frames are not dragging or decelerating, so they keep re-arming the quiet timer and the end is delayed, not dropped. A follow-to-end animation still ends in that callback when no driver owns the view.

`scrollend` on the scroller calls `needScroll` and clears `scrollWaits`, same as `scroll` (`NodeViewIOS.swift:82`, `:1254`). A feed `list`'s handler is on the list. Rows do not inherit it, and `GroupedListIOS` already forwards the end callbacks to the owner (`GroupedListIOS.swift:1021-1027`). `scrollend` is the last `EventKind` (`plan/tables/format.json:758-759`), so earlier ordinals stay put and the digest moves because the format table did.

Mac momentum is the open residual. `didEndLiveScrollNotification` is documented as the end of live scroll tracking (`NSScrollView.h:174-177`), and this change treats that as including momentum (`ScrollEnd.swift:18`, LLP 1005). The test posts the notification (`ScrollEndMacTests.swift:52-60`); it does not observe AppKit. Nothing in the header or in this tree proves the notification waits out `momentumPhase`. If it fires at finger lift, a Mac fling would end once while still coasting and again 100 ms after the coast.

The clone's use is served on the web and on iOS: set `flinging` from `scroll` (a value that does not change every frame) and clear it and snap on `scrollend`. That is one end after the last `scroll` of a fling, and a programmatic snap is its own scroll with its own end. On the JS target, boot also snaps once, per the finding above. macOS matches that if the live-scroll notification really includes momentum. Linux and Android are documented as not yet.
