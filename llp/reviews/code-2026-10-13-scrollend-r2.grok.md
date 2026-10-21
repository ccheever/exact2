# Code review: DOM's `scrollend` round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `1f837fefe8f99c20fbad3a66e512eb47319994d0e127c692cdd819038f932573`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 3's code.

---

Verdict: **NOT READY**. One bug (MAJOR), one nit (NIT). No blockers.

`scrollend` holds on web, on an iOS drag, deceleration, or animated scroll, and on a Mac live scroll. Hiding a scrolled container with `display: none` still breaks it on iOS and Mac: the layout collapse clamps the offset, the `scroll` event is suppressed, and the quiet timer still emits `scrollend`.

Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-fd7016b3.md`The reviewer flagged one major issue. I'll check that path in the code before treating it as a ship blocker.The round-1 fixes and the reviewer’s findings still need a last pass against the tree before I write the verdict. I’ll check the Apple hide path and the web boot path, then report.The review file has one bug and one nit. I’ll confirm those citations and the web boot path before writing the summary.**NOT READY.** The round-1 fixes hold. Two contract breaks remain: a hide on iOS and Mac, and the boot scroll on the JavaScript target. No blockers.

## MAJOR

**Hiding a scroller still ends the scroll.** `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:703`

`scrollViewDidScroll` calls `scrollEndMoved` and then `sendScrollEvent`. `sendScrollEvent` returns when `hasScrollLayoutBox` is false (`NodeViewIOS.swift:714`). That guard is how a `display: none` collapse stays out of `scroll` (`NodeViewIOS.swift:925`). `scrollEndMoved` and `endScroll` have no such check (`host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:10`, `ScrollEndIOS.swift:28`).

A hidden slot is published as a 0×0 frame and a 0 content size (`kernel/src/layout/publication.rs:117`). The host applies that frame and content, and both call `fitScroll` (`PresenterIOS.swift:1130`, `PresenterIOS.swift:1135`, `NodeViewIOS.swift:1269`). UIKit then clamps a non-zero `contentOffset` to the origin and calls `scrollViewDidScroll`. The collapse is not tracking or decelerating, so the 100 ms quiet timer is armed. The queued `scroll` dies on the layout-box guard. When the timer fires, `endScroll` still calls `presenter.scrollend`.

The Mac path is the same split. `clipScrolled` calls `scrollEndMoved` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:601`). `fitScroll` shrinks the document view (`NodeViewMac.swift:1217`), the clip origin clamps, and `endScroll` sends `scrollend` (`ScrollEndMac.swift:45`) while `sendScrollEvent` returns (`NodeViewMac.swift:617`).

A feed that is scrolled off zero and then becomes `display: none`, or sits under an ancestor that does, gets one `scrollend` after a `scroll` the host deliberately dropped. An app that clears `flinging` or snaps a header on `scrollend` does that work for a hide.

Do not arm, and do not update `seen`, while `hasScrollLayoutBox` is false. In `endScroll`, if the box is gone, discard the owe without sending. The restore in `applyPendingScroll` still goes through `scrollViewDidScroll` / `clipScrolled` once the box is back, so a restore that actually changes the offset still ends once.

**The boot offset ends on the JavaScript target.** `host/web-js/rt.js:922`

`drain` marks a boot assignment with `$bootScroll` and then writes `scrollTop` (`rt.js:240`). The browser fires `scroll` on that write. `onScroll` swallows that one event and clears the flag. `Booting` is cleared before `mount` returns (`rt.js:1407`). `scrollend` is allowlisted (`host/web-js/src/emit.rs:1318`) and has no binder (`host/web-js/src/events.rs:46`), so it is a plain listener (`rt.js:896`) with no `$bootScroll` check. The browser fires `scrollend` after that programmatic scroll, once the flag is already gone. A boot commit that writes a non-zero offset therefore delivers one `scrollend` and no `scroll`.

A later programmatic write delivers both, which is the DOM rule and what the grammar describes. The wasm glue registers `scrollend` on the same `on` path as `close` (`host/web/glue.js:580`), and that path drops events while `inputReady` is false (`glue.js:520`). The baked first pixel runs before input opens (`glue.js:1444`, `glue.js:1466`, `setInputReady(true)` in `activateData` at `glue.js:1273`), so a boot `scrollend` that arrives before activation is dropped with the boot `scroll`. One that arrives after activation would be delivered.

Share the boot suppression: swallow that first `scrollend` the way `onScroll` swallows the boot `scroll`.

## NIT

**The vocab card’s `list` line still omits `scrollend`.** `contract/cli/src/vocab/card.md:57`

`scroll` on the next line gained it, and the grammar says it comes on a `scroll` or a `list` (`docs/contract-grammar.md:927`). `docs/card.md:68` repeats the list line. The 12 KiB cap is real (`contract/cli/src/vocab/card.rs:253`, `contract/cli/tests/it/vocab.rs:551`). This is a card omission, and the compiler accepts `list scrollend=`. When the card is next edited, add it if the byte budget allows.

## Round-1 fixes

All six hold in the tree.

1. `ScrollEnd.moved(to:)` arms only when the reported position changes (`ScrollEnd.swift:23`). Mac passes the clip origin (`ScrollEndMac.swift:40`). iOS adds the top inset (`ScrollEndIOS.swift:14`).
2. `followEndIfOwed` returns whether it started an animation (`NodeViewIOS.swift:670`). Drag and deceleration call `endScroll` only when it returns false (`NodeViewIOS.swift:657`). The animation’s own end is `scrollViewDidEndScrollingAnimation` (`NodeViewIOS.swift:667`), which returns early only for an `OffsetDriver` or an owed start correction (`NodeViewIOS.swift:665`).
3. Park sets `incarnation = 0` and then `recycle`, which resets (`NodePoolIOS.swift:289`, `NodePoolIOS.swift:488`). `rebind` issues a new incarnation and resets (`NodePoolIOS.swift:542`). The quiet check captures that incarnation (`ScrollEndIOS.swift:21`).
4. A quiet check that still sees a touch schedules another quiet (`ScrollEndIOS.swift:23`).
5. Live-scroll observers are removed with their scroll view, state is reset, and both handlers require `note.object` to be that scroll view (`ScrollEndMac.swift:11`, `ScrollEndMac.swift:29`, `NodeViewMac.swift:1086`).
6. Order and a single deceleration end are covered by `NativeListScrollIOSTests.testAScrollEndsOnceAfterItsLastScroll`. A resize that leaves the origin owes nothing, and a live scroll holds past the quiet, in `ScrollEndMacTests`.

## The rest of the check

Ordering holds. `endScroll` calls `sendScrollEvent` and then `presenter.scrollend`, and both `scroll` and `scrollend` go through `Presenter.send`’s waiting queue (`PresenterIOS.swift:695`, `PresenterMac.swift:929`). A queued scroll whose point already matches `lastScrollEvent` does nothing.

`needScroll` runs for this node’s `scroll` or `scrollend` handler (`NodeViewIOS.swift:82`). `scrollWaits` stays true for a swipe row that has neither (`NodeViewIOS.swift:1254`), so swipe-only rows stay plain views.

`scrollend` is appended after `close` in `plan/tables/format.json:759`, so the generated plan digest changes for that reason. Kind 50 decodes to `Event::Scrollend` and ignores payload bytes (`runner/src/runner/host_kinds.rs:49`). A node without the handler commits nothing. Linux and Android are documented gaps (`docs/contract-grammar.md:931`, LLP 1005).

The Bluesky clone is served for a real fling on web, iOS, and Mac. It still writes `headerY` on every `scroll`, and `scrollTick` only restarts `calm` and `snap` (`app.contract:1074`). `scrollend` replaces that restart. `snapHeader` assigns `headerY` (`app.contract:2826`), a translate, so it does not owe another end. A snap that wrote `scrollTop` would, which is the same rule as DOM. The two majors above would run that snap on a hide, and on the JavaScript target once at boot if the feed opens off zero. Linux and Android still need the timer.

Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-fd7016b3.md` and `.../grok-review-summary-fd7016b3.md`. 31 files, +422/−40. The review file has the Apple bug and the card nit. The boot finding is from checking the web path against `rt.js`.

NOT READY
