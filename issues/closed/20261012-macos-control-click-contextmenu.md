# macOS: a Control-click runs a node's `press`, not its `contextmenu`

**Status:** Closed
**Resolution:** fixed by #487
**Systems:** host/apple macOS, pointer events
**Severity:** P2
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** T3 PR #474 (reply-links-reference-behavior); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md (X85); `issues/closed/20261010-macos-right-click-selects-no-word.md` (#454, the same `menu(for:)`); `issues/closed/20261010-macos-inline-run-contextmenu.md` (X77); `issues/20261012-macos-inline-link-run-tab-stop.md` (#484, an inline link run takes no keyboard focus); docs/contract-grammar.md ("A `contextmenu` (a right-click, a long press) …")

## Summary

On a Mac a Control-click is the secondary click. Chrome and Safari fire `contextmenu` for it and no `click`, and
AppKit shows a view's context menu for it. On the macOS host, a Control-click on a node with a `contextmenu` handler
runs the node's `press` instead, with `ctrlKey` held, and the `contextmenu` handler never runs. A right-click on the
same node runs `contextmenu`.

AppKit handles a Control-click (a `leftMouseDown` with the control flag) by asking the hit view for `menu(for:)`. If
that returns a menu, AppKit pops it and calls no `mouseDown`. If it returns nil, AppKit calls `mouseDown(with:)` with
the control flag set. The swiftc oracle below shows both paths. `NodeView.menu(for:)`
(`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift:507-522`) has handled a Control-click since #454
(`a961b8268`), but only for a paragraph with no authored handler: `guard isParagraph, …, !contextMenuAuthored,
inlineTarget(at:handler: "contextmenu") == nil else { return super.menu(for: event) }`. There it selects the word and
returns the text menu. On a node that wrote `contextmenu` (or `contextPopover`), or under one, it returns
`super.menu(for:)`, which is nil. AppKit then calls `NodeView.mouseDown` (`Mac/NodeViewMac.swift:1342`), which takes
the click as a press. `dispatchContextMenu` (`Mac/MouseEventsMac.swift:85-95`) runs only from `rightMouseDown`
(`NodeViewMac.swift:1429`) and from the ContextMenu key (`MouseEventsMac.swift:24`). #454's comment says that an
authored handler "hears the click instead". That is true for a right-click. For a Control-click nothing hears it as
a secondary click.

So on macOS, a Control-click on an app's row, link or chip runs its primary action. A Mac user who Control-clicks to
get the menu opens the row, follows the link, or presses the button. The web target on the same Mac does what Chrome
does, so the two hosts disagree.

## Why this arose

The T3 Code desktop app (Electron) shows its shell's context menu for a Control-click on a reply's link, as for a
right-click: Chromium on a Mac turns a Control-click into the secondary click. This was measured over CDP with the
main process's menu read back. A click with ⌘ or ⇧ asks for a window, which the shell denies, and ⌥ starts a
download. In the clone (T3 PR #474, task reply-links-reference-behavior), a Control-click on the same link presses
the link with Control held. The clone declares this as X85 in its `EXACT2-GAPS.md`.

## Reproduction

Main `85a116a9c80f0b9775f4ea593a80d815bc8d822b`, macOS 26.6.2 (25G83), Apple Silicon; the web target in Google Chrome
155.0.8059.39 on the same Mac. A one-file app made with `bun scripts/exact.mjs new <dir>` (`app.ts` exports an empty
`sources`), built with `bun exact.mjs mac`:

```text
component App
  state log = "-"
  state presses = 0
  state menus = 0
  action pressed(name: string)
    presses = presses + 1
    log = `press ${name}`
  action menu(name: string)
    menus = menus + 1
    log = `contextmenu ${name}`
  action seen(name: string)
    log = `focus ${name}`
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text `${log} · presses=${presses} · menus=${menus}` testId="log" font-size=20
      link href="#team" press=pressed("link") contextmenu=menu("link") focus=seen("link") testId="the-team"
        text "the team" font-size=20
      button press=pressed("button") contextmenu=menu("button") focus=seen("button") testId="options"
        text "Options" font-size=20
      text testId="para" font-size=20
        text "Read "
        text "the notes" href="#notes" press=pressed("run") contextmenu=menu("run") testId="run"
        text " here."
      button press=pressed("after") focus=seen("after") testId="after"
        text "After" font-size=20
```

Run `bun exact.mjs agent <macos|web> --size 520x300 "tap the-team modifiers Control" tree "tap options modifiers
Control" tree "tap the-team contextmenu" tree "tap options contextmenu" tree`. On both hosts the agent's click goes
through the platform: on macOS `NSApp.sendEvent` with the control flag held, on the web CDP's
`Input.dispatchMouseEvent` with the Control modifier. No real input was used.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| A Control-click on the `link`, then on the `button` | macOS | main `85a116a9c` | `press link · presses=1 · menus=0`, then `press button · presses=2 · menus=0`: the press runs, `contextmenu` never does | `contextmenu link`, then `contextmenu button`; no press | image (right), record |
| The same, web | Chrome 155 on macOS | main `85a116a9c` | `contextmenu link · presses=0 · menus=1`, then `contextmenu button · presses=0 · menus=2` | (the reference) | image (left), record |
| Control: a right-click (`tap … contextmenu`) on each | macOS and web | main `85a116a9c` | `contextmenu` runs on both hosts (macOS `menus=1` → `2`, presses unchanged) | — | record |
| AppKit oracle: a view with `acceptsFirstMouse` true (as `NodeView`) overriding `mouseDown`, `rightMouseDown` and `menu(for:)`; a Control-click through `NSApp.sendEvent` | macOS 26.6.2, `swiftc`, in-process | — | `menu(for:)` nil: the view gets `menu(for:)`, then `mouseDown` with the control flag, and no `rightMouseDown`. `menu(for:)` returning a menu: AppKit pops that menu and calls no `mouseDown` | (the platform's routing) | record |

![X85: a Control-click on nodes with press and contextmenu, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/1acf9de468f9f429508694d28104647d397cc07b/fw-issues-20261012-x85/x85-ctrl-click-web-macos.png)

Record (the contract, the commands and both hosts' output, the oracle's source and output, the code read):
https://raw.githubusercontent.com/ccheever/exact2/60ae554222a9ad462600ffe84be0285849bcbcd6/fw-issues-20261012-x85/x85-inline-link-focus-record.txt

Not run: real hardware input (the agent's events were used); Safari; a Control-click on an inline run (its
`contextmenu` is X77's, and the agent's tap on a run activates it); a Control-click on a `contextPopover` node (the
same `menu(for:)` path, read from code); a Control-click inside a text field or text area (AppKit's own editing menu;
not this issue); iOS, Linux and Windows.

## Constraints

- The web is the parity oracle (`CLAUDE.md`), and the platform agrees: Chrome, Safari and AppKit all treat a
  Control-click as the secondary click. LLP 1115: author > platform > CSS default. The author wrote `contextmenu`
  and did not say a Control-click should press.
- A Control-click should take a right-click's path, and in the same order: the secondary button's `pointerdown`,
  then `contextmenu` at its point (`docs/contract-grammar.md`), with no `click` or `press`. A handler that prevents
  the default ends it. One that does not leaves it to the host's own menu as a right-click does (#454's text menu
  for a paragraph, nothing elsewhere).
- Where no node wrote `contextmenu`, a Control-click on selectable text keeps #454's word selection and text menu.
  What it should do on a plain pressable with no `contextmenu` (a press with `ctrlKey` held today) is outside this
  issue and was not driven here.
- A text field or text area keeps AppKit's own Control-click menu.
- No open or closed issue on main covers it, and neither do the GitHub issues (searched `control-click`,
  `ctrl-click`, `⌃-click`). #454 is the nearest: it made `menu(for:)` answer a Control-click for a paragraph's text
  menu and stepped aside where an authored handler is, which is the case here.

## Acceptance criteria

- In the repro on macOS, `tap the-team modifiers Control` shows `contextmenu link · presses=0 · menus=1`, and
  `tap options modifiers Control` shows `contextmenu button · presses=0 · menus=2`, as on the web.
- A right-click, a plain click and the ContextMenu key behave as before. A Control-click on unselected paragraph text
  with no authored handler still selects the word (#454).
- A host test (`host/apple` XCTest) sends a Control-click to a `NodeView` with `press` and `contextmenu` and sees
  `contextmenu` run and `press` not.
