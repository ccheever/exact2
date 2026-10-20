# Code review: LLP 1122 T2c steps 2-3 (inspect on iOS and the Mac), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `8f3b89c30`.
- **Method:** one brief (sha256 `f1d3e8cf37d9e801647f02c7d2b6bd6de7decb75a7838c357a4fdb9602ba335a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (3 high, 4 medium, 1 low).
- **Disposition:** taken. The same fixes as the Astra record's round-2 disposition:
  - A press already down: cancelled at entry and owned to its end, and no touch goes on to UIKit.
  - A Mac sheet: every app window's events are taken. The banner's close is clicked inside the monitor, so no tracking loop waits for a mouse-up that gets dropped.
  - A swipe action or a grouped switch selects its own node.
  - The toolbar's tabs and search field are selected.
  - The Escape that leaves is owned.
  - Segment boxes come from the controls themselves.
  - A Back label with the title's words is not the title.
  - An outline goes only to the windows of the matching plan, and a bar item is boxed.
  - Low: the local exit is kept across a relaunch (`UserDefaults`).

---

I'll review this commit read-only: the inspect-mode changes, the round-1 findings and how they were fixed, and whether the hit-walk and input gates match the stated rule.Round 1's fixes are in the tree. Next I'll check the hit walk, input gates, and whether those fixes actually hold in the surrounding host code.Round 1's refresh identity, tvOS refusal, bar-item ids, heading role, 60 KiB budget, status-before-refresh, in-process exit replay, and `EXACT_INSPECT` compile-out all hold. These do not.

## High

**A press already down still fires in the app** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:50`

Hold a finger on a pressable card, then turn inspect on. Entry resets gesture recognizers and calls `cancelTracking` on `UIControl`s only. An Exact press is a `NodeView` contact (`PointerIOS.swift`), so it stays armed. `owned` records a touch only on `.began` (`InspectIOS.swift:117`), and the banner test uses the touch's current point (`InspectIOS.swift:114`). Lift after leaving inspect, or lift over the banner while it is still on: `take` returns false, `touchesEnded` runs, and the card's press fires. A second finger on the banner also fails to own a finger that began on content. On entry, cancel press contacts in the window, own every touch that is already down or that begins while inspecting, and keep swallowing that sequence until it ends. Forward a sequence only when every touch in it began on the banner.

**A Mac sheet still receives input, and the banner's close can hang the process** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:44`

Open a route sheet and turn inspect on. `show` overlays only `session.view?.window` (`InspectMac.swift:23`). The sheet is its own `NSWindow` (`SheetsMac.swift` `beginSheet`). `take` returns the event whenever that window has no overlay, so clicks, typing, Return, and Escape all reach the sheet. The sheet's confirm is Return and its cancel is Escape. Separately, press the banner's close and release just below the 32 pt banner. `mouseDown` is forwarded (`InspectMac.swift:50`) and `NSButton` waits in `trackMouse`/`nextEvent`; the monitor then drops `mouseUp`, and the main thread stays in that loop. The same freeze hits a drag that was already tracking when inspect turned on. While inspect is on, swallow every event in every app window, including sheets and panels. Finish the close click inside the monitor, without handing it to `NSButton`, and do not leave a tracking loop waiting for a mouse-up you will drop.

**A swipe action or a grouped switch selects the row** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:161`

Reveal Delete and tap it. UIKit's button is not a `NodeView`, `NativeButtonIOS`, or `AgentOwned`; the action node is hidden, so the walk stops at the cell or the row and reports the row. Tap a grouped row's switch or detail button. `GroupedListsLink.part` is called with the cell, so the loop never sees the `UISwitch` and the returned id is the row. The tool joins the row. Resolve a swipe button through `SwipeActionsHost` to the action's id. Call `part` with the hit view, and when the part is the switch or the detail button, select that accessory's node.

## Medium

**A Mac toolbar tab or search field is reported as AppKit chrome** — `host/apple/Sources/ExactKit/Mac/InspectMac.swift:109`

Click a header tablist in the toolbar, or the search field. `toolbarNode` accepts only `exact.command.` and `exact.menu.`. The group is `exact.segments.<id>.<generation>` (`HeaderMac.swift:206`) and the field is `exact.search.<id>` (`HeaderMac.swift:268`). Both are rejected, and the banner says the click is the title bar. If the search text equals the window title, the field is reported as the heading. Map `exact.search.` to that input, and the segment under the point through `HeaderSegments.tabs`.

**The Escape that leaves inspect is delivered to the app** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:102`

Press hardware Escape. `leaveInspect` clears `Inspect.on`, then `return Inspect.on` is false, so `ExactWindow.sendEvent` calls `super`. The key still reaches a key command or a presented controller. Return true for a press that arrived while inspect was on.

**An uneven segmented control selects the wrong tab** — `host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:151`

A tablist narrower than its natural width sets `apportionsSegmentWidthsByContent` (`SegmentsIOS.swift:493`). `tab(at:)` still uses `point.x / width * count`. On a squeezed "Temperature | Length" control, a tap on Temperature lands in the equal-width second column and selects Length. Hit-test the real `segmentViews`. For an `ExactTabBar`, use the button frames and let the gaps select nothing.

**Back's label is the title when the titles match** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:233`

Push a screen whose title equals the previous one, both "Library". `titleFrame` returns the first `UILabel` with that text, which can be the back button. Tapping Back selects the route heading. Skip labels inside the back button and bar-button items, and use the title label that contains the point.

**An outline for one plan clears another window, and a bar item is never boxed** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:74`

Two windows running different plans: a frame whose plan matches one session still calls `mark` on every overlay, and the other window gets an empty list, so its boxes disappear. A bar button, tab, or grouped row is the hidden source in `presenter.views`, and `shown` drops it, so the visible control is never outlined. Update only the windows of the matching plan. Resolve a hidden source to the bar item, cell, or toolbar view that shows it before marking. The Mac path does the same (`InspectMac.swift:135`).

## Low

**A local exit does not survive relaunch** — `host/apple/Sources/ExactKit/Inspect.swift:152`

`Inspect.left` is process memory. Leave inspect, lose the exit POST, and relaunch: the server replays `inspect {on, seq}` and inspect turns back on. The wasm page stores that seq and answers the replay with the exit again (`host/web/dev.js`). Persist the exited command the same way. The new tests never send a `UIEvent` or `NSEvent`, and never hit a swipe, a switch, a toolbar tab, a sheet, or a refresh, so none of the failures above fail the suite.

**Verdict: Request changes — a press already down, a Mac sheet, and the banner's close still reach or freeze the app, and a swipe or grouped switch selects the row.**

