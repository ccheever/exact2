---
name: 20261010-x79-macos-tab-from-a-focused-box
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X79: On macOS Tab from a focused box that is no Tab stop goes nowhere, so the import wizard's popup takes Tab itself

Moved to main `issues/20261010-macos-tab-from-a-focused-box.md` (2026-10-10), where it is tracked. Main PR
[#411](https://github.com/ccheever/exact2/pull/411) filed it (merged `294a17827`).

## Summary

On macOS, Tab and Shift+Tab from a focused box that is no Tab stop (`tabindex=-1`, focused by `autofocus`, `focus()`
or a click) leave the focus on it. The host builds its own key-view loop (`PresenterMac.swift` `syncKeyViewLoop`).
It links each Tab stop to the next, and a paragraph that is no stop on to the stop after it. Any other node is left
unlinked, so the box's `nextValidKeyView` is nil and `window.selectNextKeyView` does nothing. On the web, Tab goes on
to the next stop after the box in tree order (for a box that holds buttons, its first button), and Shift+Tab to the
one before it. A hand-built AppKit window with its own key-view loop does the same.

## Why it arose

Task `20261010-import-wizard-initial-focus` (T3 PR #409, merged `d608796fa`; first declared there as X76, then X78,
renumbered X79 at merge). The reference's browser import wizard is a Base UI dialog. After a step change, Base UI's
`restoreFocus: "popup"` leaves the focus on the popup itself (`tabindex=-1`). From there Tab reaches the popup's first
control and Shift+Tab (the focus trap's guard) its last. The clone's popup (`browser-profiles.contract`
`BrowserImportWizard`, `column id="browser-import-popup" tabindex=-1`) takes the focus the same way. In the task's
live drive 1 on macOS, an agent `type <popup> key Tab` left the focus on the popup. The cause was read in the host's
source, and the task's PR asked the coordinator whether to reproduce and file it ("Decision needed").

## Clone workaround (import-wizard-initial-focus)

The popup's own `key` handler (`popupKeys`, `browser-profiles.contract`). While the popup itself holds the focus (its
own `focus` and `blur` set `held`), Tab goes to `wizard.tabFirst` and Shift+Tab to `wizard.tabLast`, after
`preventDefault()`. The stops come from `browser-profiles-settings.ts` `wizardTabStops`. There is no visible
difference: live drive 2 matched the reference row for row. When main fixes the host, re-drive the popup's Tab: if it
reaches `tabFirst` with no handler, the Tab half can go. The Shift+Tab half stays until the focus trap (X53, #282)
lands. The reference's guard wraps Shift+Tab to the last stop, while the web, and a fixed host, go to the stop before
the popup.

## Evidence and history

- 2026-10-10, main PR #411 reproduced it on main `474999b9f` in a one-file app, on the first attempt. An autofocused
  `column tabindex=-1` (`popup`) holds First and Last, between Before and After. A second `tabindex=-1` box (`box2`) is
  focused by `focus()` and followed by Next A and Next B. The agent sent Tab and Shift+Tab to the focused box. The Exact
  web in Chrome 155 moved the focus to First and Before from `popup`, and to Next A and Focus box2 from `box2`. On macOS
  26.6.2 the focus stayed on the box all four times, while Tab and Shift+Tab from the stops moved (the control). A
  `swiftc` AppKit oracle confirmed it. With AppKit's own loop, a focused view that is no key view goes on: Tab to
  First, Shift+Tab to Before. An explicit loop that leaves its `nextKeyView` nil strands Tab. Linking it on to First
  moves Tab there. Main PR #327 changes neither `syncKeyViewLoop`'s links nor the Tab path in `NodeView.keyDown` (read
  from its diff). Image:
  [x79-tab-from-unstopped-focus-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/fd65b9a4a734e9631d92189b430ad94a659a117d/fw-issues-20261010i/x79-tab-from-unstopped-focus-web-macos.png);
  record: [x79-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fd65b9a4a734e9631d92189b430ad94a659a117d/fw-issues-20261010i/x79-record.txt).
