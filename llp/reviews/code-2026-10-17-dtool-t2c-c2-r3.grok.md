# Code review: LLP 1122 T2c steps 2-3 (inspect on iOS and the Mac), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `fb3ee917c`.
- **Method:** one brief (sha256 `09850943b70a7e368d3652a4148674418f659b8fea4d0dd52eb199665fcf88f4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited. The process was stopped after it printed its verdict.
- **Verdict:** request changes (1 high, 4 medium, 2 low).
- **Disposition:** taken (the last round).
  - The Mac drain: as in the Astra record, item 3. A modifier still held keeps the drain too.
  - Mac toolbar segments: an automatic width is each label's width plus the space left shared alike (AppKit's `fill`). Limit: the collapsed group (a menu when the bar is tight) is not looked into.
  - iOS header tablist and search field: selected and outlined.
  - The Mac title outline looks only outside the content view.
  - Right to left: button boxes follow item order.
  - A finger down at entry is owned from the start.
  - `forget` gives `acceptsMouseMovedEvents` back.
  - On the local exit: as noted, a relaunch is a new build id, which the server tells inspect off. The exit is back in process memory, which settles review2's nit (c): no seq a restarted server reuses can collide.

---

I'll review this commit read-only: the round 1 and 2 records first, then the inspect-mode code against the selection rule, input gates, and the production compile gate.Round 2 named sixteen fixes and two limits. I'll read the inspect sources against those dispositions and the selection rule.The round-2 ownership and hit-walk changes are in the tree. Next I’ll check whether they hold in the window, toolbar, and channel code around them.Auto-compacting conversation (83% full)...
Conversation compacted.
The review is still open. I'll confirm the round-2 fixes and the three suspected defects against the current tree before writing the report.Round 2's other fixes hold. While inspect is on, iOS keeps every touch and press, presses the banner close inside `take`, owns Escape through its release, resets enabled recognizers (including ones still waiting), cancels in-flight control tracking and node presses, and ends editing again on `keyboardWillShow`. A status-bar tap scrolls nothing. On the Mac the monitor takes mouse, wheel, gesture, key, and modifier events in every app window, and menus close as they open. A click is reported on the session that owns the hit view. Swipe actions and grouped switches select their own nodes. A plain title is the lowest matching label, then the one nearest the bar's middle. Outlines follow bar buttons, titles, tab buttons, toolbar items, and the Mac search field, and a frame for another plan leaves other windows alone. Refresh matches one canonical instance path. The report is capped at 60 KiB before the plan and nonce. `EXACT_INSPECT` is defined everywhere except `EXACT_UPDATE_TRUST=production`, and the gate test's scan matches the sources. The three named limits match the code: an AppKit tracking loop already running at entry is not cancelled, a grouped row's cell is not boxed, and a Mac toolbar segment is not boxed.

## High

**Leaving inspect on the Mac delivers the rest of the input** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:102`

Hold Escape to leave. The first key-down is swallowed and `show` keeps the monitor because `currentEvent` is `.keyDown` (`InspectMac.swift:38`). The key repeat is another `.keyDown`. The drain allowlist is only mouse-up, mouse-dragged, key-up, and `flagsChanged` (`InspectMac.swift:99`), so the default branch removes the monitor and returns that repeat (`InspectMac.swift:102-104`). The repeated Escape reaches the app (a dialog's cancel, a key equivalent), and the eventual key-up is no longer monitored. The same abort happens for a scroll, a magnify, or a second key while a button-down exit is draining. A second hole is on the way in: closing the banner while Shift is held does not drain, because the mouse-up has no buttons down and is not a key-down (`InspectMac.swift:38`). Releasing Shift then hits `ExactViewMac.swift:272`, which synthesizes the app's key-down and key-up. Fix: while draining, swallow every event until no mouse button is down, the key that was down has come up, and `modifierFlags` is empty. Leave the monitor in place whenever a button, a key, or a modifier is still down, and never return the event from the drain.

## Medium

**Mac toolbar segments are always split into equal columns** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:191`

`width(forSegment:)` is the width last passed to `setWidth`, and 0 means automatic (`NSSegmentedCell.h`: "Width of 0 means autosize to fit"). `HeaderSegments` never calls `setWidth` (`HeaderMac.swift:206`). The default distribution is `.fill`: segments are sized to their contents, then leftover space is shared (`NSSegmentedControl.h`). `widths.allSatisfy { $0 > 0 }` is therefore false, and the index is `x / bounds.width * count` (`InspectMac.swift:192-193`). On a header tablist "All | Reading list", a click on the right side of All selects Reading list. The round-2 fix described this as AppKit's segment widths. The collapsed group (a menu when the bar is tight, identifier `exact.segments.<id>.<generation>`) misses the prefix list at `InspectMac.swift:198` and is reported as title-bar chrome. Fix: hit-test the segment's laid-out bounds, and map the collapsed item to the tab it shows.

**An iOS header tablist and the header search field are reported as UIKit chrome** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:236`

A header text tablist is a `LabelledSegmentedControl` installed as the navigation item's title view (`NavigationBarIOS.swift:789-824`). It is not an `ExactSegmentedControl`, so `tab(at:)` returns nil (`SegmentsIOS.swift:149`). The walk then stops at the navigation bar (`InspectIOS.swift:236-238`). `barNode` only knows bar-button items and the route-title label (`InspectIOS.swift:264-273`), so the tap is announced as part of UIKit's navigation bar. The header search field is the same: `UISearchController.searchBar.searchTextField` lives in that bar (`NavigationBarIOS.swift:854-860`), and `barNode` never returns `c.search?.field`. The Mac path selects both. Outlines miss them too: `place` boxes a `HeaderTitleView`, a bar button, or a tab-bar button (`InspectIOS.swift:116-130`), and the named limit covers only the grouped cell and the Mac segment. Fix: before the chrome answer, resolve the title view through `TitleSegments.press.tabs` and `segmentViews`, and resolve a hit inside the route's search bar to that field's node. Box those same frames.

**A Mac title outline can box a content label** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:247`

Outlining a hidden heading walks every subview of the frame view and returns the first visible `NSTextField` whose string equals `window.title`. The content view is in that walk. A label in the page that repeats the window title is boxed, and the title-bar field is not. Selection is stricter: it only accepts that field when the hit is outside the content view (`InspectMac.swift:163-165`). Fix: search only views that are not descendants of the content view.

**A right-to-left tab bar selects the mirrored item** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:150`

`buttonBoxes` sorts by `minX`, and both tab lookups use that index as the logical tab index (`SegmentsIOS.swift:152-155`, `InspectIOS.swift:296-299`). UIKit lays a tab bar out leading to trailing, so in Arabic the first tab is on the right. Tapping it returns the leftmost button's index and selects the last tab. The equal-width segment fallback already flips (`SegmentsIOS.swift:164`); the button-box path does not. `segmentViews` is already in segment order, so content segments are fine. Fix: order the boxes leading to trailing, using the bar's layout direction.

## Low

**A finger already down at entry is not owned until a later touch event** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:198`

Entry cancels recognizers, tracking, and the node press, but it does not put the current touches into `owned`. Ownership is added only for a touch event that arrives while inspect is still on. Turn inspect on with a finger down, then leave with Escape before that finger moves: the lift finds `Inspect.on` false and the touch absent from `owned`, so `ExactWindow.sendEvent` forwards it (`TouchIOS.swift:18-23`). The round-2 note says that finger is owned. Fix: at entry, record the window's current touches and own them through their end.

**A window dropped during inspect keeps mouse-moved delivery** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:86`

`drop` writes back `acceptsMouseMovedEvents` (`InspectMac.swift:65`). `forget`, which runs when the overlay removes itself because no session remains in the window (`InspectMac.swift:337`), clears the saved value and leaves the flag set. Fix: restore the flag in `forget` as well.

The host tests call `target`, `pick`, and `select` directly. They do not send a `UIEvent` or an `NSEvent`, which round 2 declined. The gate proves the `Package.swift` define and that the inspect identifiers sit inside `#if EXACT_INSPECT`; it does not compile a production target unless a production Theme fixture binary is already on disk. A relaunch stays out of inspect because the new process mints a new build id and the server answers an unknown id with inspect off. The `UserDefaults` value is cleared by that replay, so it does not itself carry the exit across relaunch. Same-process reconnect still does.

Request changes: the Mac drain hands input to the app after leaving, and header tabs, the iOS search field, and uneven Mac toolbar segments still miss the plan node under the pointer.

