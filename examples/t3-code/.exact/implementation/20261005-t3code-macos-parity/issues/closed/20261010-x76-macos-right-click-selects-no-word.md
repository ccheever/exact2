---
name: 20261010-x76-macos-right-click-selects-no-word
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X76: On macOS a right-click on unselected text selects no word, so the shell menu's Copy stays disabled there

Moved to main `issues/20261010-macos-right-click-selects-no-word.md` (2026-10-10), where it is tracked. Main PR
[#410](https://github.com/ccheever/exact2/pull/410) filed it (merged `474999b9f`).

## Summary

On a Mac a secondary click on unselected text selects the word under the pointer, then opens the menu for it.
Chrome, AppKit's read-only `NSTextView` and WebKit all do this. The macOS host selects nothing:
`NodeView.menu(for:)` (`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift`) opens the read-only text menu
(LLP 1115 D8) only over an existing selection, and only a double or triple click selects a word. So no word is
selected and no `selectionchange` fires.

## Why it arose

`EXACT2-GAPS.md`'s "Text context menu" entry S1, from tasks `20261010-realinput-1010d-followups` (RD-4) and
`20261010-shell-context-menu` (T3 PR #407, merged `6bac646cc`). The T3 desktop shell (`DesktopWindow.ts`
`installContextMenu`) builds its menu from Chromium's edit flags after Chromium has selected the word under the
pointer, so the reference's Copy is enabled on any word. The clone's module (`T3TextContextMenu.swift`) builds the
same menu, and its Copy follows the page's selection.

## Clone workaround (shell-context-menu)

None. The difference stays: over unselected page text the shell menu's Copy is disabled, where the reference
enables it. Over selected text it matches. When main fixes the host, the module's page menu should enable Copy
after a right-click on a word with no change of its own, since ExactKit's selection then exists. Re-check that,
and check that the monitor that takes the host's read-only text menu (`T3TextContextMenu.swift`) still answers
the click.

## Evidence and history

- 2026-10-10, main PR #410 reproduced it on main `42fd5d99e` in a one-file app: `text "Known words here"
  selectionchange=…`, then the agent's real secondary click on "Known". The Exact web in Chrome 155 showed
  `selected=[Known]`. macOS 26.6.2 showed `selected=[]`, and a double click there selected "Known" (the control).
  A `swiftc` oracle sent the same right-click to a read-only, selectable `NSTextView` and to a `WKWebView`. Both
  selected "Known" and opened Look Up "Known", Copy and the rest. Main PR #327 does not touch
  `TextSelectionMac.swift` (read from its diff). Image:
  [x76-right-click-word-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x76-right-click-word-web-macos.png);
  record: [x76-record.txt](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x76-record.txt).
