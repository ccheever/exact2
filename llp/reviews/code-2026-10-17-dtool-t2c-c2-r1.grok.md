# Code review: LLP 1122 T2c step 2 (inspect on iOS), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `81e50b725`.
- **Method:** one brief (sha256 `36ed7a5afe3e61570da27ed9af3a7db8faed3cec81c0836c586784d9bc7039fe`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** none. The run ended after its opening lines, with no findings.
- **Disposition:** nothing to take. Round 2 runs both reviewers on the fixed commit.

---

I'll review this commit as a read-only correctness pass, starting with the review criteria and the diff itself.The commit is inspect mode on the iOS host. I'll read the new Swift, the wasm mirror, and the design rules next.Inspect mode selects an ordinary content view and the drawn header title, and the production gate is real. Several of the paths the ruling names still select the wrong node or let the app take the touch.

## High

**A finger on the banner delivers every touch in that event to the app** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:68`

`take` returns false as soon as any touch in `event.allTouches` is geometrically over the banner, and `ExactWindow.sendEvent` then calls `super`. One finger on the close control and another on a button: the content finger's began, moved, and ended all go to the app, so the button fires while the banner is up. A drag that starts on content and crosses the banner does the same for the rest of that gesture. Forward only the touch that began on the close control; keep swallowing the others.

**A touch that is already down when inspect turns on is never cancelled** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:70`

`pressed` is set only on `.began` while an overlay exists. A pan or a held `UIControl` that began earlier has already received `touchesBegan`. Later phases are swallowed, including `.ended` and `.cancelled`, and `show` does not reset recognizers. The pan stays in `.changed` and the control stays highlighted; after you leave inspect that recognizer can ignore new drags. On entering inspect, cancel in-flight touches and reset the window's gesture recognizers before swallowing.

**The keyboard still types into the app** — `host/apple/Sources/ExactKit/IOS/TouchIOS.swift:18`

Interception is only `ExactWindow.sendEvent`, and only for `.touches`. The software keyboard is another `UIWindow`, so taps on keys insert text, and the pad bar's Done and ± run. A hardware keyboard's `.presses` and a trackpad's `.scroll` also fall through to `super`. A field that was focused when inspect turned on keeps taking input. On entering inspect, resign first responder, and consume every event type except the close control's own touches.

**Refresh after a new generation reports whichever node reused the view id** — `host/apple/Sources/ExactKit/Inspect.swift:156`

A generation does `presenter.reset()` and the new runner assigns ids again. `refreshSelection` re-finds by site and instance only when `views[id] == nil`. After a plan edit the old id is usually live as a different node, so the host answers the server's `{refresh}` with that node and never reads the stored `plan`. The tool joins the wrong source instead of the same site and instance path, or "gone". Remember the session generation; when it differs, accept a candidate only when its plan digest, site, and instance path all match, and only when exactly one node matches.

**The plain title and Back's label are "the widest UILabel in the bar"** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:157`

`titleFrame` walks every label, including the system back button and bar-button titles. On Chat pushed over a longer previous title, tapping the word on Back selects the route heading. Tapping the real inline title selects nothing whenever some other label is wider, because the point is not inside that widest rect. The test only hits the chevron at `bar.minX + 22` and the drawn `HeaderTitleView`. Use the bar's title and large-title labels, and treat the system back button as UIKit chrome.

**A revealed swipe action selects the row** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:104`

The action's node is hidden. UIKit's button is not a `NodeView`, `NativeButtonIOS`, or `AgentOwned`, so the walk climbs through the row's table to the row and reports that. Tapping Delete joins the row, not the action. Before accepting an ancestor node, resolve a swipe button to the action it shows.

## Medium

**Outlines and the press highlight follow the hidden source, not the view UIKit drew** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:46`

`outline` keeps a site's view only when `window != nil && !isHidden`. A header button is `isHidden`, so its site gets no box. A grouped row's section is hidden while the row itself is not, so the box is drawn on the authored frame and `reveal` scrolls that, not the cell. The bar-item and tab press highlight uses `presenter.views[id]`, the same hidden node, so a finger down draws a zero-height box. Map those ids to the bar item, tab control, or cell, and outline that.

**A grouped-list switch or detail button selects the row** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:108`

`GroupedListsLink.part` already distinguishes `"switch"` and `"detail"`, and the row's `target` is that accessory's node. The walk returns `row` and drops the part, so the accessory's own site is never the selection. When the part is the switch or the detail button, select `target`.

**The banner's close is undone when the same command is replayed** — `host/apple/Sources/ExactKit/Inspect.swift:143`

`leaveInspect` reports the exit and forgets it. If that POST is lost, the next hello is answered with `{on: true, seq}` of the command just left, and `inspectCommand` turns inspect back on. Remember that seq, and answer a replay of it with the exit again.

**A window that appears after inspect turns on is not covered** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:23`

Overlays are created only inside `show`, for windows that already host a session view. `take` then requires an overlay. A later scene window, or a session view that gains a window after the command, has `Inspect.on` but no banner: its touches reach the app and it gets no outlines. Install the overlay from `sendEvent` when inspect is on and the window has none.

**The 64 KiB trim does not include `plan`, which is added after the count** — `host/apple/Sources/ExactKit/Inspect.swift:48`

`bytes()` measures the envelope without `plan`. `reportSelected` then sets `plan` (up to 1024 characters). A node trimmed to just under 64 KiB is posted over the server's limit and dropped. Put `plan` in the envelope before counting.

**A tap on the status bar selects the content under it** — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:92`

The status bar is not another window, and the walk has no status-bar stop. On a screen with no navigation bar, a tap on the clock hit-tests the page and selects that node. If the point lies in the scene's status-bar frame, select nothing and say so.

## Low

**The tests do not exercise the paths above** — `host/apple/tests/ExactKitTests/InspectIOSTests.swift:61`

They call `target` for one content point, the drawn title, and the back chevron, plus `Inspect.select` for the report and the click token. They never send a touch through `ExactWindow`, never open a plain title, a bar item, a tab, a grouped row, or a swipe action, and never refresh, outline, reconnect, or compile with `EXACT_UPDATE_TRUST=production`. `Inspect`, `InspectHost`, and `inspectCommand` are all inside `#if EXACT_INSPECT`; a production build does compile without the define.

The compile-time gate holds, but a second finger, the keyboard, Back's title, a swipe action, and a refresh after a plan edit all still do the wrong thing, and the tests do not catch any of them.

