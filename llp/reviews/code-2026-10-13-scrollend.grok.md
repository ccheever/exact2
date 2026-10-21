# Code review: DOM's `scrollend`, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `d182f0bd4e40bb5b314000b9d1f094b6a03f52246d395d32be5d49fb0f679ed3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's code.

---

`c84bf130` is **NOT READY**. A normal fling, wheel, or programmatic jump does deliver one `scrollend` after its last `scroll`. Three paths deliver an end that is not that scroll.

## MAJOR

**A Mac clip-view resize ends a scroll that did not move.** `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:601`

`clipScrolled` handles `boundsDidChangeNotification` and always calls `scrollEndMoved()`. That notification covers bounds size as well as origin. `scrollEndMoved` (`ScrollEndMac.swift:23`) sets `owed` and, outside a live scroll, arms the 100 ms timer without comparing the origin. `sendScrollEvent` (`NodeViewMac.swift:616`) then no-ops when the origin is unchanged, and `endScroll` still sends `scrollend`. A window resize or a layout pass that only changes the clip view's size snaps a header that keyed off `scrollend`.

Arm the end only when the clip origin changed.

**An iOS follow-to-end animation emits `scrollend` before that animation's scrolls.** `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:656`

When `followsEndAfterInteraction` is set, `scrollViewDidEndDragging` (`willDecelerate == false`) and `scrollViewDidEndDecelerating` (`:658`) call `followEndIfOwed()` and then `endScroll()`. Within 80 pt of the maximum, `followEndIfOwed` (`:672`) starts `setContentOffset(animated: true)` and returns. `endScroll` takes the owed end in that same callback. UIKit runs the animation afterward. Those frames are not tracking or decelerating, so they re-arm `owed` (`ScrollEndIOS.swift:13`), and `scrollViewDidEndScrollingAnimation` (`NodeViewIOS.swift:666`) sends a second `scrollend`. The runner sees `scrollend`, then the animation's `scroll` events, then `scrollend` again. `Presenter.send`'s waiting queue cannot repair it: those later scrolls do not exist yet when the first end is queued. A fling that does not follow the end stays ordered.

If the follow starts an animation whose target differs from the current offset, leave the end to `scrollViewDidEndScrollingAnimation`. If the target is already the offset, keep the immediate `endScroll`.

**A grouped list's quiet timer can fire on the node that reused the view.** `host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:13`

The work item does not capture `incarnation`. `queueScrollEvent` does (`NodeViewIOS.swift:731`). `endScroll` (`ScrollEndIOS.swift:21`) calls `take()` before the handler and `presenter?.views[id] === self` checks, so a stale item clears `owed` even when it sends nothing. `recycle` / `rebind` leave `scrollEnd` in place (`NodePoolIOS.swift:485`, `NodePoolIOS.swift:537`). A non-grouped scroller with `scrollend` sets `scrollNeeded` and fails `recyclable` (`NodePoolIOS.swift:327`). A grouped list does not: `groupedOwner` makes `needScroll` a no-op, and the collection is the scroll view, so the node is pooled. `pool.take` rebinds that view to the new id during the batch (`PresenterIOS.swift:856`), and `GroupedLists.sync` drops the old collection afterward (`GroupedListIOS.swift:205`). The 100 ms item then sees `views[newId] === self`, a `scrollend` handler, and a live scroll view, and delivers the previous node's end on the new list.

Capture `incarnation` and return before `take()` on a mismatch. Clear `scrollEnd` in `recycle` and `rebind`.

## MINOR

**Mac live-scroll observers outlive the scroll view, and `live` can stay true.** `host/apple/Sources/ExactKit/Mac/ScrollEndMac.swift:10`

`observeLiveScroll` never removes `willStartLiveScrollNotification` or `didEndLiveScrollNotification`. Overflow teardown sets `scroll = nil` (`NodeViewMac.swift:1080`) without clearing `scrollEnd.live`. `live = false` is written only in `liveScrollEnded` (`ScrollEndMac.swift:20`). One live view is not double-registered. After the scroll view is replaced, if `didEnd` never arrived, later wheel notches and programmatic jumps stay bracketed (`ScrollEndMac.swift:27`) and never arm the quiet timer.

Remove both observers and clear `live` in that teardown.

**A bottom-inset change on a grouped list owes `scrollend` with no scroll.** `host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:638`

`mount` calls `owner.scrollViewDidScroll` whenever `roomChanged` is true. The offset write there runs only for a top-inset change (`:603`). A bottom padding or footer change can leave `contentOffset` and `scrollTopInset` equal to `lastScrollEvent`, so `sendScrollEvent` returns (`NodeViewIOS.swift:712`). `scrollViewDidScroll` has already called `scrollEndMoved` (`NodeViewIOS.swift:699`). One quiet interval later the list emits `scrollend` for a layout inset change.

Call `scrollEndMoved` only when the offset or the top inset differs from `lastScrollEvent`.

## NIT

**The new tests do not lock the ordering they describe.** `host/apple/tests/ExactKitTests/NativeListScrollIOSTests.swift:145`

`testAScrollEndsOnceAfterItsLastScroll` passes when the only record is `"end"`. The deceleration case is a manual `scrollViewDidEndDecelerating` after an unanimated `setContentOffset`. `ScrollEndMacTests` does not post a bounds-size change or replace the scroll view. The contract and JS emit tests lock kind 50 and the string `"scrollend"`.

**The three `ScrollEnd` headers restate LLP 1005.** `host/apple/Sources/ExactKit/ScrollEnd.swift:1`

The policy is already in the LLP and the grammar. The two `endScroll` bodies are the same four lines, which matches the existing iOS/Mac split.

**The card's `list` line omits `scrollend`.** `docs/card.md:67`

`scroll` gained it (`docs/card.md:68`, `contract/cli/src/vocab/card.md:58`). The grammar, the lowerer, and `scrollend.rs` allow `list scrollend=`.

## What holds

Web, both targets, listens for the browser's own `scrollend` on the scrolling element, the same node as `scroll` (`host/web/glue.js:580`, plain listener in `rt.js:896`). It does not bubble, and the listener is on the target. Programmatic `scrollTo` and `scrollTop` writes are that DOM event. Wasm drops it when the element is no longer that view.

On a path that does not follow the end, `endScroll` flushes `sendScrollEvent` and then `scrollend`. `Presenter.send` queues FIFO while a batch is applying (`PresenterIOS.swift:663`, `PresenterMac.swift:929`), so a queued scroll does not overtake that end. A later coalesced flush no-ops when the origin is unchanged.

The quiet timer cancels on the next move (`ScrollEnd.swift:20`). A drag that starts during quiet is bracketed, so the item is cancelled; if it fires while tracking, dragging, or decelerating it returns without `take()` and the drag or deceleration end still calls `endScroll`. Deallocation is safe (`[weak self]`).

`scrollViewDidEndScrollingAnimation` returns before `endScroll` only while an `OffsetDriver` or a start correction is active (`NodeViewIOS.swift:664`). A normal animated `setContentOffset` still reaches `endScroll`. Driver frames are unbracketed `contentOffset` writes, so the quiet timer still fires after the last frame.

`scrollWaits` / `needScroll` look at this node's handlers (`NodeViewIOS.swift:82`, `:1250`). A grouped list does not grow a second scroll view. `GroupedListIOS` forwards the scroll, drag-end, deceleration, and animation callbacks (`:1021`). A swipe row without its own `scrollend` still waits.

`EventKind` appends `"scrollend"` after `"close"` (`plan/tables/format.json:758`), so the digest changes and existing ordinals stay put. Kind 50 has no payload (`host_kinds.rs:49`). Both `exact_dispatch` arms accept it. A node without the handler commits nothing (`event.rs:1088`). Linux and Android are documented as not yet firing it.

## bsky5

Served for the feed itself on web, iOS, and Mac. Set `flinging` on `scroll` and clear it and snap the header on `scrollend`. A fling, including momentum, a drag that throws nothing, a wheel's notches, and a programmatic jump each end once, after the last move. A `scroll` handler that sets `flinging` only when it was false does not write every frame. Linux and Android still have no event, so a clone there keeps the per-frame timer.

The Mac resize end and the iOS follow-end double end both snap that header for a scroll that has not finished. The recycle end does too, on a grouped list that was pooled.

Full notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-1c40e3f8.md` and `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-1c40e3f8.md`.

NOT READY
