# macOS: a right-click on unselected page text selects no word, so no text menu opens

**Status:** Open
**Systems:** host/apple macOS, text selection
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x76-macos-right-click-selects-no-word.md; LLP 1115 (D8, Appendix A's macOS row "right-click on selected text shows nothing"); issues/20261009-text-selection-rectangles-commands.md

## Summary

On a Mac, a secondary click on text that is not selected selects the word under the pointer, then opens the
menu for it. Chrome does this: the Exact web build in Chrome selects "Known" and fires `selectionchange` with
`text: "Known"`. AppKit does it too. A hand-built read-only, selectable `NSTextView` selects "Known" and opens
Look Up "Known", Translate, Copy, Speech and the rest. A `WKWebView` (Safari's engine) selects "Known" and opens
Look Up "Known", Copy and the rest.

The macOS host selects nothing. `NodeView.menu(for:)`
(`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift:426-450`) returns the read-only text menu (LLP 1115 D8)
only when the paragraph already has a selection. Otherwise it returns super's menu, which is nil.
`rightMouseDown` (`Mac/NodeViewMac.swift:1443-1453`) dispatches `contextmenu` and otherwise calls super. Only
a double or triple click selects a word or a line (`TextSelectionMac.swift:179-190`). So a right-click on a word
selects nothing and `selectionchange` does not fire (both seen in the drive below). By the code above it also
opens no menu (the agent does not see menus). Any Copy in the app that follows the selection stays disabled.

LLP 1115's 2026-10-09 audit listed "right-click on selected text shows nothing → Look Up/Copy/Services" for the
macOS host, and that has landed. This issue is the unselected case.

## Why this arose

T3 Code's desktop shell answers every right-click that the page leaves alone (`DesktopWindow.ts`
`installContextMenu`). It builds the menu from Chromium's edit flags, and Chromium on a Mac has already selected
the word under the pointer, so the reference's Copy is enabled on any word. The clone builds the same menu from
a module (`T3TextContextMenu.swift`, task `shell-context-menu`, T3 PR #407, merged `6bac646cc`). Its Copy follows
the page's selection, so on unselected text the clone's Copy is disabled where the reference's is enabled. The
clone keeps that difference (its `EXACT2-GAPS.md`, "Text context menu", S1).

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `42fd5d99e4c884a366483aecd226f08442dee793`. Its
`app.ts` exports an empty `sources`. The full contract (it also carries X77's rows) is in the record.

```text
component X76app
  state sel = ""
  action selected(s: Selection)
    sel = s.text
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text "Known words here" testId="t" selectionchange=selected font-size=24
      text `selected=[${sel}]` testId="sel"
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent <web|macos> --size 520x420 "tap t contextmenu at 30 14"
tree`. The agent sends a real secondary click, through the application on macOS and over CDP in Chrome.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| right-click on "Known" (`tap t contextmenu at 30 14`) | macOS 26.6.2 (25G83), Apple Silicon | main `42fd5d99e` | `selected=[]`: no word selected, no `selectionchange` (no menu, by the code read) | "Known" selected and the read-only text menu for it, as on the web and in AppKit | image (right), record |
| Same, web | Exact web (JS target) in Chrome 155.0.8059.39 | main `42fd5d99e` | `selected=[Known]`, highlighted | (the reference) | image (left), record |
| Control: a double click on "Known" (`tap t clicks 2 at 30 14`) | macOS | main `42fd5d99e` | `selected=[Known]` | (selection works) | record |
| Hand-built AppKit: a read-only, selectable `NSTextView` and a `WKWebView` holding the same text, sent `rightMouseDown`/`rightMouseUp` on "Known" | macOS 26.6.2, `swiftc` | — | both select "Known". Menus: `NSTextView` Look Up "Known", Translate "Known", Search With Google, Cut, Copy, Paste, …; `WKWebView` Look Up "Known", Translate "Known", Search with Google, Copy, … | — | record (oracle source beside it) |

![X76: a right-click on an unselected word, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x76-right-click-word-web-macos.png)

Record (contract, commands, both drives, the control, the AppKit and WebKit oracle):
https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x76-record.txt
(oracle source: `fw-issues-20261010g/x76-appkit-webkit-oracle.swift.txt` at the same commit).

Not run: #327's head, a right-click inside an existing selection, text whose node or ancestor has a
`contextmenu` handler, `user-select="none"` text and labels inside controls, iOS, Linux, and real hardware input
(the agent's secondary click was used).

## Constraints

- The web is the parity oracle (`CLAUDE.md`), and AppKit's read-only text view does the same. LLP 1115: what
  the author leaves unsaid is the platform's.
- Only text the host already lets the reader select is affected (`NodeView.textSelectable`: not a label inside
  a control, nor `user-select="none"`, nor chrome such as a `header`). A right-click there selects nothing, as
  now.
- A right-click inside the current selection keeps it, as `NSTextView` and Chrome do.
- The word is the one a double click selects. `selectionchange` fires for it before the menu opens, as a
  double click's does.
- An authored `contextmenu` or `contextPopover` still runs and wins over the host's menu (author > platform).
  Whether the word is selected under one is Chrome's call, and it was not checked here.
- Main PR #327 (head `14493d253`) does not touch `TextSelectionMac.swift`, and its `NodeViewMac.swift` hunk
  changes only `mouseEntered`/`mouseMoved`/`mouseExited` (read from its diff, not run).

## Acceptance criteria

- In the repro on macOS, `tap t contextmenu at 30 14` then `tree` shows `selected=[Known]`, as on the web.
- On real input, that right-click opens the host's read-only text menu with Look Up "Known" and an enabled Copy.
- A right-click on unselectable text (a button's label, `user-select="none"`) selects nothing and opens nothing.
- A macOS XCTest covers a secondary click on an unselected word.
