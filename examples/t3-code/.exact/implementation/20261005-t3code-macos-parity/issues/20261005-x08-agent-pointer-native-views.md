---
name: 20261005-x08-agent-pointer-native-views
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-browser-surface, 20261005-diff-review-engine, 20261005-floating-device-player, 20261005-right-panel-tab-menu, 20261005-settings-scoped-controls-and-theme-editor, 20261005-sign-in-terminals, 20261005-terminal-drawer, 20261005-terminal-integrations, 20261005-terminal-layout, 20261005-terminal-surface]
upstream_url: https://github.com/ccheever/exact2/issues/107
reproduced_on: null
---

# X8: Pointer input (down, move, up, wheel) for native views in the agent driver

## Summary

T3 Code has several surfaces that people use with the pointer inside one canvas or view: the terminal, the device stream, the diff text, the floating player. The exact2 agent driver can tap,
type and press keys, but it cannot send pointer phases into a native view. The clone therefore proves these behaviors only in `(attended session)` rows, with a person at a real trackpad.
The fix needed is pointer phases (down, move, up, wheel) in the agent's native input, as forms of `tap`.

## Why this issue arose

### The T3 Code behavior
The reference runs in a browser engine, so every surface receives pointer events, and a browser driver can send them (Playwright and CDP both can; what the plan's oracle tools expose is to confirm at `issue-open`). The behaviors in question:
- **Terminal.** The Ghostty canvas listens for pointer down/move/up, wheel, mouse down/up and context menu; the scrollbar has its own pointer handlers
  (`apps/web/src/terminal/ghostty/surface.ts:1686-1697`). A click count of 2 selects a word and 3 selects a line (`:1305-1343`); a click on a link is handled before selection (`:1307`). A person drags to
  select text, scrolls the scrollback with the wheel or the thumb, right-clicks for the selection menu, and drags a pane separator (`20261005-terminal-surface`, `-layout`, `-integrations`).
- **Device stream.** Pointer down, move and up on the stream become touches on the device with pointer capture (`apps/web/src/components/device/DeviceStreamView.tsx:413-430`); drag, flick and capture outside
  the window are part of the feel (`20261005-floating-device-player`).
- **Floating player and panels.** Dragging the mini player, its eight resize zones and cursors, the settings panel's corner grip (`20261005-settings-scoped-controls-and-theme-editor`).
- **Diff and text.** Drag over line numbers to comment a range, select text to Cite, wheel through pinned headers (`20261005-diff-review-engine`).
- **Menus.** Right-click and middle-click on tabs (`20261005-right-panel-tab-menu`) and the links in sign-in terminals (`20261005-sign-in-terminals`).
Each behavior has states that a driver should reach: idle, pressed, dragging, released, cancelled (pointer leaves the window and returns), and wheel with momentum. Errors: none beyond the target
being missing.

### What exact2 does today
- `EXACT2-GAPS.md` X8 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "`ExactNativeInput` has only `.text` and `.key`
  (`host/apple/modules/ExactNativeModule.swift:527-530`)." "The agent cannot click, drag or scroll inside native views (terminal, browser, device screen, PDF/video). These checks need a real-input session."
  Support needed there: "Pointer phases (down/move/up, wheel) in `ExactNativeInput`, as forms of `tap` (the DEFERRED agent-API rule allows a new input as a form of `tap`)."
- Library (`20261005-platforms-v3`, testing-and-debugging.md): the agent runs `tap`, `clock`, `layout`, `screenshot` and the other listed operations against Contract nodes by `testId`; "web input checks whether another
  element covers the chosen point"; `layout <target> at x y` is "a world/canvas pick, not a general button hit-test". Pointer phases inside a native view are not covered: unknown in this library.
- Observed in the clone (clone code, mc-orch tree, 2026-10-05): a native view receives agent input through `agentInput(_ input: ExactNativeInput)`, whose `switch` has the two cases `.text` and `.key`
  (`modules/apple/T3KeyRecorder.swift:97-106`, which refuses text and accepts key chords).

### Where the clone hits it
- The device stream view turns mouse down/drag/up into touches (`modules/apple/R6DeviceStream.swift:366-381`, `touch("begin"|"move"|"end", at:)`); the agent cannot reach this path.
- The existing web views (`R6MediaPreview.swift`, `T3TimelineMermaid.swift`), the AVPlayerView and PDFView surfaces (X29) and the planned terminal web view have the same limit.
- Workaround: `(attended session)` rows. I counted 11 such pointer rows in 8 tickets on 2026-10-05 (terminal-surface 1, terminal-integrations 2, sign-in-terminals 1, floating-device-player 1,
  right-panel-tab-menu 1, diff-review-engine 3, settings-scoped-controls-and-theme-editor 1); two more rows are key-only (terminal-layout) and the terminal-drawer row mixes keys and window resize. Each row needs a person, a lane build with
  `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, and notes; none can run in an agent loop or in CI.
- Difference: the logic below the pointer (selection model, hit tests, drag math) is covered by ported unit tests and Contract `tap` rows, but the join between a real pointer and the native view is checked by hand, once, per release of the clone.

## Why it must be resolved

The parity goal needs these surfaces to keep matching the reference after the first verification. An attended row proves a behavior once; it does not catch a regression a week later. Terminal selection, device touch and
drag-to-resize are the places where such regressions are likely and where a user would notice at once. Waiting tickets are the nine in `blocks`; their rows are marked nonblocking with the attended workaround.
Cost of the workaround: a person for every release, slow feedback, and rows that agents cannot repeat. The risk is silent drift in the most pointer-heavy parts of the app.

## Requested support

The web way: Pointer Events (`pointerdown`, `pointermove`, `pointerup`, `wheel` with coordinates, button, modifiers) and a driver that can dispatch them (Playwright's `mouse.down/move/up/wheel`, CDP `Input.dispatchMouseEvent`).
For exact2: new cases in `ExactNativeInput` for pointer down, move, up and wheel, reachable from the agent as forms of `tap` (or a sibling operation if the DEFERRED rule needs it), macOS first, other hosts noted.

| Piece | Request |
| --- | --- |
| Native input cases | `.pointer(phase, x, y, button, modifiers)` with `phase` in down, move, up; `.wheel(dx, dy, x, y, modifiers)`; coordinates in the target view's space, with a stated origin |
| Agent syntax | A form of `tap`, for example `tap <testId> at x y`, plus `drag <testId> from x y to x y [steps]` and `wheel <testId> dx dy` (names to settle upstream) |
| Safety | Same refusals as `tap`: missing target, hidden target, coverage; a clear refusal when the view does not implement pointer input |
| Delivery | The event reaches the native view's `mouseDown/Dragged/Up`, `scrollWheel` path as a real `NSEvent` would, so existing handlers run unchanged |
| Clock | Moves can be spread over the agent clock (`clock +N`) so a drag has a duration |

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a native view module that records every pointer event it receives in a Contract-visible state slot.
2. Agent: `tap <view> at 10 20`, then try any pointer-phase operation. Expected (target): down, up at (10, 20), and a drag with moves. Actual: only text and key reach the view.
3. Clone scenario: lane build, open the device surface with the fixture hub; expected: no agent operation can start a touch through `R6DeviceStream.swift:372-381`; compare with a real trackpad drag, which can.
4. Reference check: the oracle (Electron) receives pointer events from its driver.

## Acceptance for the fix

- An AppKit test: a view receives down, move, up and wheel with the given coordinates and modifiers through the agent path, and its `NSEvent`-based handlers run.
- Agent run on the minimal app: a drag produces the expected phases in order with coordinates inside the view; wheel produces the expected deltas; an unsupported view returns a clear refusal in `logs`.
- The documented syntax is added to the agent's operation list.

## App adoption after resolution

Convert the pointer `(attended session)` rows in the nine tickets to agent rows: terminal selection and links, scrollbar drag, device drag, mini player drag and resize, settings panel resize, diff drag-to-comment and
Cite selection, tab middle-click. Keep an attended row only for what a driver cannot judge (trackpad feel, cursors). Implement `.pointer` and `.wheel` in the clone's native views (`R6DeviceStream`, the terminal web view
once built, the tab strip if it has a native part). `issue-close` checks that at least the terminal selection row and the device drag row run by agent.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Fix built (2026-10-06)

Correction: the draft's premise was wrong. On exact2 main the agent already sends real window mouse events (down, move, up, drag, wheel, pinch, hover, context menu, double click) to a native module's NSView. What was missing, and is now built on branch `daehyeon/fw-x8-agent-pointer` (worktree `~/orca/workspaces/exact2/t3-fw-x8`), commits `e3c67cff2` and `a160ae671` (review fixes), not pushed:
- Agent mouse-button events go through `NSApplication.sendEvent`, so an app's NSEvent local monitors (the clone's tab-strip monitor) see them as they see a hand's.
- New `tap` forms: `auxclick [at x y]` (middle button), `clicks <1-3> [at x y]`, `at x y` for `contextmenu` and `wheel`, `modifiers` held through `down`/`drag`; wheel and pinch carry the window and point. The parser refuses words it would drop. iOS, Linux and Windows answer `delivery: "unsupported"`.
- Known: a mouse-up an AppKit tracking loop takes does not reach the monitors, as with a real mouse, so an open popover may stay open (QUEUE.md names MenusMac and CollectionMac).
- Evidence: the five checks; macOS XCTests (689); `smoke.mjs macos --app native-fixture` 103/103; `smoke.mjs web --app native-fixture` 57/57 with the router; two independent reviews, their findings fixed. No DEFERRED change needed (forms of `tap`).

## Merged upstream; partly adopted (2026-10-07)

The fix landed as main PR #186 (#107 closed) and reached the feature branch with main `cff90b364` in task [20261007-adopt-main-fixes-r3](../tasks/20261007-adopt-main-fixes-r3.md). Adopted there:

- The right-panel tab's middle click (`RightPanelTabsInput.swift`, a local monitor) is an agent row: `tap panel-tab-<id> auxclick`. Driving it showed a clone bug: the monitor tested `view.visibleRect`, which an unclipped NSView reports beyond its bounds, so every tab matched and the first one in the dictionary closed (a middle click on Files closed Diff). The hit test is now `bounds ∩ visibleRect`; `macos/tests/contextmenu/tab-input.swift` has a two-tab case that fails on the old test. Live: BEFORE (base driver, no `auxclick`) the tap is a plain left press and both tabs stay; AFTER the Files tab closes and Diff stays.
- Comments in `T3Sidebar.swift`, `T3Timeline.swift` and `T3PanelsNative.swift` say which agent forms their monitors now see (`tap … mouse modifiers`, `tap … wheel`, `tap … mouse at x y`).

Still to convert (attended until their tasks re-drive them, now possible with `tap … mouse`, `clicks 1-3`, `wheel … at`, `drag … modifiers`): terminal selection, links and scrollbar (terminal-surface, terminal-integrations), device drag (floating-device-player), panel resize (settings-scoped-controls-and-theme-editor), diff drag-to-comment and Cite selection (diff-review-engine), sign-in terminal links. A mouse-up an AppKit tracking loop takes still does not reach monitors (as with a real mouse).

## Attended rows converted (2026-10-07, adopt-main-fixes-r3 follow-up)

Each pointer row is now an agent row, proved first in AppKit with the agent's event shapes (window never key):
- Terminal (`macos/tests/terminal/pointer.swift`): drag select, double and triple click, right-click menu, a file link and a ⌘-click URL (link messages), scrollbar thumb drag and wheel. Two clone bugs found and fixed: the terminal's web view swallowed the first click in a window that is not key (`acceptsFirstMouse`), and its selection popup / right-click menu ran a modal `NSMenu.popUp` that blocks an agent (now reported as `selectionMenu` under the agent, as `T3ContextMenu` does).
- Device stream (`macos/tests/r7-device`): an agent mouse drag sends begin/move/end touches.
- Theme editor panel drag and grip, diff Shift-range and drag over line numbers, pinned headers under the wheel, Cite: agent recipes written into their task rows.
- Live drive: not yet run successfully for these rows (see the task record); the tab middle click was driven live.

Still not agent rows, with reasons: Korean 2-Set and IME (input method, not pointer), ⌘C/⌘V (shared real pasteboard), following a link (opens the user's browser or editor), a drag outside the window and back (`agent-drag.mjs` refuses points outside the viewport), cursor shapes (no cursor readback), sign-in terminal links (user hold on sign-in tasks).

## Real input (2026-10-07, real-input-checks)

The rows above marked "not agent rows" that a real hand can cover ran with real input (Orca computer use and
HID events, lane bundle copies, base `4f523ef5c` against the branch) in
[20261007-real-input-checks](../tasks/20261007-real-input-checks.md):

| Row | Result |
| --- | --- |
| Right-panel tab middle click | base closes the wrong tab (Diff); branch closes Files — pass |
| Cursor shapes | terminal I-beam, drawer edge up-down resize, chat arrow, theme editor header open hand, grip crosshair, in both builds — pass |
| A drag out of the window and back | theme editor header released outside the window: the panel follows and stays inside it, both builds — pass |
| Terminal double-click, right-click | word selected with the Add to chat / Copy popup; Add to chat / Copy / Paste menu — pass |
| Terminal drag selection | **fails with a real pointer in both builds** (the agent's drag selects): open, cause not found (QUEUE.md) |
| Sidebar rail drag and double-click reset | pass (branch) |

Still not covered, with reasons: following a link (opens the user's browser or editor), sign-in terminal
links (user hold), ⌘C/⌘V into the shared pasteboard (the user's clipboard; only an empty-clipboard check
ran), IME and Korean 2-Set (input method, not pointer). X8 stays `closed-upstream` and open in the plan
until the terminal's real-pointer drag selects.
