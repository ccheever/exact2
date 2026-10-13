# Code review: DOM's `scrollend`, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `d182f0bd4e40bb5b314000b9d1f094b6a03f52246d395d32be5d49fb0f679ed3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 2's code.

---

Reviewed `c84bf1300`. Five findings:

1. **MAJOR — macOS emits `scrollend` without scrolling.** [NodeViewMac.swift:601](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:601) calls `scrollEndMoved()` for every bounds notification, including size changes. I reproduced a notification while resizing a clip view whose offset remained `(0,0)`. Mounting or resizing can therefore invoke the handler after 100 ms without any `scroll`. Track actual offset changes before marking an end owed, as [CSSOM View requires](https://drafts.csswg.org/cssom-view/#scrolling).

2. **MAJOR — a recycled iOS list can deliver its previous occupant’s end.** [ScrollEndIOS.swift:13](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:13) captures neither `incarnation` nor the originating scroll view. `recycleScroll()` preserves `scrollEnd`, and `rebind()` changes the node’s ID. Reusing a nested list during the quiet interval lets the old callback invoke the new list’s handler; the presenter identity check passes. Cancel/reset this state during recycling and validate the captured incarnation.

3. **MAJOR — iOS reports completion before its follow-to-end animation finishes.** [NodeViewIOS.swift:656](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:656) calls `endScroll()` immediately after `followEndIfOwed()`, which can start an animated offset correction. This clears `flinging` and snaps the header while scrolling continues, then permits another end after that animation. The `OffsetDriver`/`startOwed` check protects only `scrollViewDidEndScrollingAnimation`; other completion paths need equivalent coordination.

4. **MINOR — a stationary touch can permanently suppress an owed end.** [ScrollEndIOS.swift:14](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/ScrollEndIOS.swift:14) abandons the quiet callback when `isTracking` is true. A programmatic jump followed by a touch spanning that deadline, without dragging, receives no subsequent drag-end callback. The end stays owed indefinitely. [Tracking can precede dragging](https://developer.apple.com/documentation/uikit/uiscrollview/istracking); completion must be reconsidered when tracking stops.

5. **MINOR — macOS observers survive scroll-view replacement.** [ScrollEndMac.swift:10](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/Mac/ScrollEndMac.swift:10) registers observers without removing them when overflow changes replace the scroll view. The callbacks also ignore the notification’s source. An old view retained during a gesture can end the replacement’s scroll; conversely, its retained `live` flag can suppress the replacement’s quiet completion. Detach observers and reset state when replacing the backend.

The compiler, payload handling, ABI routing, ordinary presenter queue ordering, and grouped-list forwarding are consistent. The plan digest changes correctly.

For bsky5, this supplies the missing timer-replacement API on web. Apple needs the fixes above before reliably replacing `calm`/`snap` with clearing `flinging` and snapping on `scrollend`. Per-frame header updates still remain.

Read-only validation: five relevant prebuilt tests passed, plus the AppKit resize reproduction. No fresh build or full iOS rerun; the reported unrelated iOS failures remain unbaselined.

**NOT READY**
