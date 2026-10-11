# macOS: Tab from a focused box that is no Tab stop (tabindex=-1) goes nowhere

**Status:** Open
**Systems:** host/apple macOS, focus, keyboard
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x79-macos-tab-from-a-focused-box.md; docs/contract-grammar.md ("Focus order: `tabindex`"); LLP 1088 D7.3; issues/20261009-gui-dialog-action-commands.md (the focus trap, a different gap)

## Summary

`tabindex=-1` makes a box focusable but no Tab stop (`docs/contract-grammar.md`, "Focus order"; LLP 1088 D7.3).
`autofocus`, `focus(id)` and a click put the focus on it. On the web, Tab from that box goes on to the next Tab
stop after it in tree order, and Shift+Tab to the one before it. HTML's sequential focus navigation starts from the
focused element. For a box that holds buttons, Tab reaches its first button.

On macOS the focus stays on the box, for Tab and for Shift+Tab. A focused node's Tab calls
`window.selectNextKeyView(self)` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:232-235`), and AppKit moves to
the box's `nextValidKeyView`. The host builds the key-view loop itself: `autorecalculatesKeyViewLoop` stays false,
and `syncKeyViewLoop` (`Mac/PresenterMac.swift:1254-1285`) links each Tab stop to the next. A paragraph that is no
stop gets a `nextKeyView` to the stop after it, since a clicked paragraph is where Tab starts, as on the web. Any
other node is left unlinked. `Presenter.tabbable` (`:1289-1295`) is false for an explicit negative `tabindex`, while
`acceptsFirstResponder` (`Mac/FocusMac.swift:15-19`) is true for any explicit `tabindex`. So the box takes the
focus, its `nextValidKeyView` is nil, and Tab does nothing. A keyboard user cannot leave the box without the pointer.

A hand-built AppKit window does what the web does. With AppKit's own loop (`autorecalculatesKeyViewLoop`), a
focused view goes on to the next key view after it and back to the one before it. It does so also when the view is
no key view itself (`canBecomeKeyView` false, AppKit's nearest thing to `tabindex=-1`). Only an explicit loop that
leaves the focused view's `nextKeyView` nil, the host's shape, strands Tab there. Linking that view on to the next
stop, as the host does for a paragraph, moves Tab on. The `swiftc` oracle below shows all three.

## Why this arose

T3 Code's browser import wizard is a Base UI dialog. After a step change, Base UI's `restoreFocus: "popup"` leaves
the focus on the popup itself, which is `tabindex=-1`. From there Tab reaches the popup's first control, and
Shift+Tab (the focus trap's guard) its last. The clone's popup is `column tabindex=-1` and takes the focus the same
way (`examples/t3-code/browser-profiles.contract` `BrowserImportWizard`, on the T3 branch; task
`import-wizard-initial-focus`, T3 PR #409, merged `d608796fa`). In the clone's live drive on macOS, an agent `type
<popup> key Tab` left the focus on the popup.

The clone works around it with the popup's own `key` handler (`popupKeys`). While the popup itself holds the focus
(its own `focus` and `blur`), Tab goes to the first stop and Shift+Tab to the last. The stops come from
`browser-profiles-settings.ts` `wizardTabStops`. Every box that takes the focus this way needs the same handler.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `474999b9fe95610fdde7b99314cec8b82723de1e`. Its
`app.ts` exports an empty `sources`. `contract build` compiles it with no warning.

```text
component X79app
  state at = "-"
  action seen(name: string)
    at = name
  action focusBox2
    focus("box2")
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=10
      text `focus=${at}` testId="log" font-size=22
      button "Before" testId="before" focus=seen("before")
      column id="popup" testId="popup" tabindex=-1 autofocus focus=seen("popup") padding=10 gap=8 border-width=1 border-style="solid" border-color="#888"
        text "popup, tabindex=-1, autofocus"
        button "First" testId="first" focus=seen("first")
        button "Last" testId="last" focus=seen("last")
      button "After" testId="after" focus=seen("after")
      button "Focus box2" testId="go2" press=focusBox2 focus=seen("go2")
      column id="box2" testId="box2" tabindex=-1 focus=seen("box2") padding=10 border-width=1 border-style="solid" border-color="#888"
        text "box2, tabindex=-1, focus()"
      button "Next A" testId="nexta" focus=seen("nexta")
      button "Next B" testId="nextb" focus=seen("nextb")
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent <web|macos> --size 520x520 "type popup key Tab" tree logs`
(and the other drives in the table, each from launch). At launch `popup` holds the focus on both hosts
(`focus=popup`). The agent's `type <target> key` focuses its target only when the target does not hold the focus
already, so the key goes to the focused box.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| Tab from the autofocused `popup` (`type popup key Tab`) | macOS 26.6.2 (25G83), Apple Silicon | main `474999b9f` | `focus=popup`: the focus stays on the box, and no `focus` event runs | `focus=first`, as on the web | image (right), record |
| Shift+Tab from `popup` | macOS | main `474999b9f` | `focus=popup` | `focus=before` | record |
| Tab and Shift+Tab from `box2`, focused by `focus("box2")` (`tap go2`, then `type box2 key Tab`) | macOS | main `474999b9f` | `focus=box2` for both | `focus=nexta`, then `focus=go2` | record |
| The same four drives, web | Exact web (JS target) in Chrome 155.0.8059.39 | main `474999b9f` | `focus=first`, `focus=before`, `focus=nexta`, `focus=go2` | (the reference) | image (left), record |
| Control: Tab and Shift+Tab from Tab stops | macOS; the last drive also on the web | main `474999b9f` | `before` Tab → `first`, `first` Tab → `last`, `last` Tab → `after`, `after` Shift+Tab → `last`, `first` Shift+Tab → `before` (the box is skipped) | — | record |
| Hand-built AppKit: Before, Box (a custom `NSView` that accepts first responder, holding First and Last), After. Box is made first responder, then a Tab key event goes through `NSWindow.sendEvent` | macOS 26.6.2, `swiftc` | — | AppKit's own loop, Box a key view or not: Tab → First, Shift+Tab → Before. An explicit loop that leaves Box's `nextKeyView` nil: Tab stays on Box, with or without an `initialFirstResponder`. The same loop with Box's `nextKeyView` set to First: Tab → First | — | record (oracle source beside it) |

![X79: Tab from a focused tabindex=-1 box, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/fd65b9a4a734e9631d92189b430ad94a659a117d/fw-issues-20261010i/x79-tab-from-unstopped-focus-web-macos.png)

Record (contract, commands, both drives, the controls, the AppKit oracle):
https://raw.githubusercontent.com/ccheever/exact2/fd65b9a4a734e9631d92189b430ad94a659a117d/fw-issues-20261010i/x79-record.txt
(oracle source: `fw-issues-20261010i/x79-appkit-oracle.swift.txt` at the same commit).

Not run: #327's head, a focused `dialog` that is no stop (`acceptsFirstResponder` admits `semanticTag == "dialog"`),
a `tabindex=-1` box inside an open modal `dialog` or a popover, positive `tabindex` values around the box, iPadOS
with a hardware keyboard, Linux, and real hardware input (the agent's key events were used).

## Constraints

- The web is the parity oracle (`CLAUDE.md`), and AppKit's own key-view loop does the same. LLP 1115: author >
  platform > CSS default. The author wrote `tabindex=-1` (focusable, no stop) and said nothing about Tab.
- Tab from a focused node that is no stop goes to the first stop after it in the order the loop already uses (tree
  order, positive `tabindex` first) and wraps as Tab does. Shift+Tab goes to the last stop before it. For a box that
  holds stops, Tab reaches its first one.
- The box stays out of the loop. No stop's `nextKeyView` points to it, as for a paragraph today ("it points on to
  the next stop after it, and no stop to it"), so Tab and Shift+Tab from the stops still skip it.
- Inside an open modal `dialog`, the walk is the dialog's, as `syncKeyViewLoop` walks `dialogs.active`. A focused
  non-stop inside a dialog goes on to the dialog's stops.
- A `key` handler that prevents Tab still wins (author > platform), so the clone's workaround keeps working until it
  is removed.
- Main PR #327 (head `14493d253`) does not change it. Its `PresenterMac.swift` hunks add date and select controls to
  `tabbable` and `keyView(of:)` and leave inert and hidden nodes out. `syncKeyViewLoop`'s links, its paragraph
  `starts`, and the Tab path in `NodeView.keyDown` are unchanged, and an explicit negative `tabindex` is still never
  linked (read from its diff, not run). Its `showModal(id)` work belongs to the focus trap's issue
  (`issues/20261009-gui-dialog-action-commands.md`, #282), a different gap.

## Acceptance criteria

- In the repro on macOS, `type popup key Tab` shows `focus=first` and `type popup key Shift+Tab` shows
  `focus=before`. After `tap go2`, `type box2 key Tab` shows `focus=nexta` and `type box2 key Shift+Tab` shows
  `focus=go2`, as on the web.
- Tab from `before` still goes to `first`, and Shift+Tab from `first` still goes to `before`: the box is not a stop.
- A macOS XCTest covers Tab and Shift+Tab from a focused `tabindex=-1` box.
