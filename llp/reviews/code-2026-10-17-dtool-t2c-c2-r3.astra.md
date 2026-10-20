# Code review: LLP 1122 T2c steps 2-3 (inspect on iOS and the Mac), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `fb3ee917c`.
- **Method:** one brief (sha256 `09850943b70a7e368d3652a4148674418f659b8fea4d0dd52eb199665fcf88f4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (6 high, 8 medium, 1 low).
- **Disposition:** taken (the last round), except where noted.
  - 1: tvOS compiles again. The segments lookup that names `InspectHost` is iOS only; checked with `--tvos --no-install`.
  - 2: the Mac monitor also takes `mouseEntered`, `mouseExited` and `cursorUpdate`.
  - 3: after leaving, the Mac drain takes every event until no button, no key inspect mode took and no modifier is down. Keys are owned one by one, so Escape held through a mouse move is still owned. A key whose release never comes frees the app after 5 s. Tested with constructed NSEvents.
  - 4: a keyboard about to show ends editing in every window of the app's sessions, not only those with an overlay.
  - 5: entry also turns off the status-bar tap of every other scroll view in the window (a web frame's, a hatch's) and gives it back on leaving.
  - 6: limit kept. An AppKit control's tracking loop already running at entry is not cancelled. It is for §10 in T2c's as-built.
  - 7: after inspect mode ends, a press begun beside an owned one is owned too. A touch already down at entry is owned from the start: the window records the touches down (`InspectHost.see`).
  - 8: an iOS header tablist shown as the title's segments selects the tab under the point (`titleSegments`, `segmentViews`). The header's search bar selects its field. Both are outlined.
  - 9: a plain Mac toolbar item is outlined by the viewer AppKit makes for it.
  - 10: outlines are grouped by the window each node is in (a sheet's, a popover's), on iOS and the Mac.
  - 11: not taken. Two sessions in one build running different plans give one status digest (the first session's). It is for §10.
  - 12: a disabled Mac toolbar tab is selected by its retained id.
  - 13: button boxes follow item order, right to left in a right-to-left layout.
  - 14: not taken. `actionView` already refuses an ambiguous match (more than one button for an id), so the reverse lookup finds nothing rather than another action.
  - 15: an overlay that removes itself gives its window's `acceptsMouseMovedEvents` back.
  - Tests: InspectMacTests now sends constructed NSEvents. Every key and modifier is taken, a click in a window with no overlay is taken, and Escape leaves while its release is still taken, then the drain ends. UIKit gives no public way to make a UIEvent, so iOS interception is checked end to end on the simulator (repeated after this round). The gate test checks a production binary only when one is on disk (theme-fixture's, built for it).

---

1. **High — Development tvOS compilation is broken again.** [SegmentsIOS.swift:153](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:153) references `InspectHost` inside `#if EXACT_INSPECT`, but that file also compiles on tvOS and neither `InspectHost` implementation does. Guard this helper with `os(iOS)` too. The textual gate accepts this invalid platform combination.

2. **High — Mac hover events still reach the app.** [InspectMac.swift:46](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:46) omits `.mouseEntered` and `.mouseExited`. Those reach `NodeView.mouseEntered/mouseExited`, then `Presenter.trackPointer`, which dispatches application hover and pointer-move handlers. Moving across tracking boundaries can therefore change application state while inspecting. Intercept those events and guard application pointer dispatch.

3. **High — Mac draining stops before owned inputs finish.** [InspectMac.swift:97](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:97) removes the monitor on a mouse move, wheel event, or repeated keydown. It also removes it after one keyup without checking other held keys. Hold Escape to leave, move the mouse, then release Escape: its release reaches the app. Track owned keys/buttons individually and retain interception until each finishes.

4. **High — A later iOS window can retain an active keyboard.** [InspectIOS.swift:34](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:34) ends editing only in windows already represented by overlays. A new scene/window with an autofocus field can show its keyboard before its first intercepted touch creates an overlay. Software-keyboard editing then bypasses `ExactWindow.sendEvent`. Gate application focus independently of overlays and cover newly attached windows immediately.

5. **High — Status-bar interception misses native scrollers.** [ScrollsToTopIOS.swift:23](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/ScrollsToTopIOS.swift:23) disables only the viewport and registered node scrollers. An iframe’s `WKWebView.scrollView`, or a hatch-provided scroll view, retains its own `scrollsToTop`. With the other scrollers disabled, a status-bar tap can scroll that content while inspecting. Cover actual window scrollers, including later additions, and restore their previous settings on exit.

6. **High — The acknowledged Mac tracking limitation still violates input isolation.** [InspectMac.swift:43](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:43) cancels menu tracking but leaves existing control tracking active. Start dragging an `NSSlider`, then enable inspect: its nested tracking loop can continue invoking `rangeChanged` and application input/change handlers. The round-two limitation is honestly recorded, but remains a correctness blocker. Cancel tracking or suppress its application actions while allowing the loop to unwind.

7. **Medium — iOS press ownership is incomplete after exit.** [InspectIOS.swift:183](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:183) adds presses to `owned` only while `Inspect.on`. After exit, an event containing owned press A and newly begun press B is swallowed wholesale, but B is not recorded; B’s later release can reach the app without its beginning. Apply the touch branch’s ownership rule to presses too. Entry also needs an active-input ledger to capture sequences that produce no event before inspect ends.

8. **Medium — iOS header search and title segments remain unselectable.** [InspectIOS.swift:245](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:245) resolves segments through `SegmentHost`, whose lookup requires `ExactSegmentedControl`; navigation titles use `LabelledSegmentedControl` with identities in `RouteController.titleSegments.press.tabs`. `barNode` likewise ignores `RouteController.search.field`. These authored controls fall through to “UIKit’s navigation bar.” Resolve both retained associations before the boundary.

9. **Medium — Ordinary Mac toolbar items still have no outline.** [InspectMac.swift:241](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:241) requires `NSToolbarItem.view`. AppKit explicitly returns nil for automatically generated item views, which `ToolbarMac` uses for normal buttons. Selecting Add can succeed while its outline and hover highlight use no visible geometry. Resolve the generated item viewer, as the hit walker already does. This omission exceeds the recorded grouped-row/Mac-segment limits.

10. **Medium — Mac sheet outlines are drawn in the parent window.** [InspectMac.swift:219](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:219) groups every session node under `session.view.window`, even when a node lives in a sheet or popover window. Its geometry closure returns coordinates in that other window, then the parent overlay interprets them locally. Group placements by their actual presentation window and create/update that window’s overlay.

11. **Medium — A second plan’s selections are permanently rejected as stale.** [BuildChannel.swift:65](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/BuildChannel.swift:65) publishes only the first booted session’s plan digest, while selection accepts any session. With windows running plans A and B, selecting B produces B’s digest against A’s status; `tamber-inspect.current` rejects it, and refresh repeats the mismatch. Give each inspected plan/session matching channel identity and status, or explicitly refuse inspection outside the registered session.

12. **Medium — Disabled Mac toolbar tabs are mistaken for system chrome.** [InspectMac.swift:194](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:194) calls `HeaderSegments.tab(at:)`, which rejects disabled/inert tabs and disabled groups. A visible disabled authored tab still carries a plan identity under the selection rule. Read its retained tab id independently of action availability.

13. **Medium — iOS tab identities reverse in right-to-left layouts.** [InspectIOS.swift:299](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:299) indexes logical tabs using button boxes sorted left-to-right. UIKit reverses their presentation in RTL; the standalone `ExactTabBar` lookup makes the same assumption. Selecting one tab reports the opposite tab, and outlines use the wrong box. Order boxes according to the bar’s effective layout direction.

14. **Medium — Swipe-action identity still depends on nonunique labels/images.** [SwipeActionsIOS.swift:219](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/IOS/SwipeActionsIOS.swift:219) reverses `actionView`, which matches rendered buttons by label or image. Give a row different leading and trailing actions with the same label: the visible trailing button can resolve to the leading action first. Retain an explicit rendered-button/action association, or refuse ambiguous matches instead of reporting another action.

15. **Low — Automatic Mac overlay cleanup loses the saved window setting.** [InspectMac.swift:338](/tmp/rv-dt-ios-r3/host/apple/Sources/ExactKit/Mac/InspectMac.swift:338) calls `removeFromSuperview` directly when the last session leaves. `forget` discards the saved `acceptsMouseMovedEvents` value without restoring it; only `drop` restores it. Detaching a session therefore leaves the window changed permanently. Route every teardown through the restoration path.

The canonical instance comparison, generation-aware refresh, report budget, status-before-refresh ordering, internal banner-close handling, grouped accessory identity, and weak display-link target look correct by inspection. I found no unguarded production reference to the inspect types or new threading issue.

The tests do **not** prove input isolation. The round-two claim that NSEvents cannot be constructed publicly is incorrect; existing [TableFeedKeyMacTests.swift:40](/tmp/rv-dt-ios-r3/host/apple/tests/ExactKitTests/TableFeedKeyMacTests.swift:40) already constructs and dispatches them. Add interception and ownership tests using that pattern. The [compile gate:49](/tmp/rv-dt-ios-r3/host/apple/inspect-gate.test.mjs:49) checks an optional pre-existing binary, not a fresh compilation of HEAD. Builds and tests were not run under the read-only restriction.

**Verdict: Request changes — input isolation remains incomplete, native selection/outlines have unresolved gaps, and development tvOS does not compile.**