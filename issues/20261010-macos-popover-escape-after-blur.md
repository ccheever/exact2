# macOS: Escape closes no `popover="auto"` after an action's `blur()`, because the window itself is then the first responder

**Status:** Open
**Systems:** host/apple macOS, popover, focus
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x73-macos-popover-escape-after-blur.md; LLP 1021 (`llp/1021-menus.rfc.md`)

## Summary

On the web, Escape closes the topmost auto popover whatever has the focus, `<body>` included: the popover's
light dismiss hears the document's close request, not the focused element. On macOS the host's popover Escape
(`MenuHost.key`, `host/apple/Sources/ExactKit/Mac/MenusMac.swift:324-338`) answers only when the focus owner is a
view inside the session's viewport (`:330-331`). An action's `blur()` or `blur(id)` (`Presenter.blurElement`,
`Mac/PresenterMac.swift:698-711`) calls `window.makeFirstResponder(nil)`, which makes the window itself the first
responder. `focusOwner` (`MenusMac.swift:87-90`) then returns nil, because the window is not an `NSView`, so `key`
returns false and the popover stays open. A second Escape does the same. A click outside still closes it.

LLP 1021 §4 says "Light dismiss, the one-auto-popover rule, and Escape are the host's, per the spec"
(`llp/1021-menus.rfc.md:121-122`). Two neighbours of `MenuHost.key` already treat "no view holds the focus" as the
session's:

- `DialogHost.ownsFocus` returns true when there is no focus owner (`Mac/DialogsMac.swift:205-207`), so a modal
  dialog's Escape closes it in the same state.
- `ExactView.ownsShortcutFocus` (`Mac/ExactViewMac.swift:89-104`) gives the page's shortcuts the keys when the window
  is the first responder, "as a page's shortcuts hear keys with no element focused (jukebox F11)". With several
  sessions in one window, the first in view order hears them.

## Why this arose

T3 Code's Pull Requests page has a Filters menu with submenus (Author, State). When a submenu closes by Escape with
the pointer outside its row, Base UI leaves the focus on BODY. The clone matched that with `blur()` (FW-3, T3 PR
#378). In a real-input session on macOS (realinput-1010c, RC-3) the user went Filters › Author, clicked in "Search
authors" and pressed Escape, which closed Author. A second Escape left Filters open; only a click outside closed it
(2 of 2). The reference closes Filters there. Image:
[G-fw3-fw4-filters.png](https://raw.githubusercontent.com/ccheever/exact2/e8fabfeed724dce27c432773cd615219967a8fea/realinput-1010c/G-fw3-fw4-filters.png).
#378's agent drive passed, because its `type <row> key Escape` focused the row first.

The clone works around it (T3 PR #399, task `20261010-realinput-1010c-fixes`). Instead of calling `blur()`, its
`closeSub` focuses a 1-pt rest box inside the Filters popover (`pr-filters-rest`: `tabindex=-1`, `aria-hidden`,
outside the menu's key handlers; `examples/t3-code/pages-prs.contract` `PrFiltersMenu` on the T3 branch). The focus
owner is then a view in the viewport, and the host's Escape closes Filters. The clone's other `blur()` callers were
checked by reading: none leaves an auto popover open that only the host's Escape closes. Any app that drops the focus
with `blur()` while an auto popover is shown, as a web page may, leaves a popover that Escape cannot close on macOS.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `a10050516b549be8273668b4c9fdd6caa3ee746e`. Its
`app.ts` exports an empty `sources`.

```text
component X73app
  state log = "-"
  action drop
    blur()
    log = "blurred"
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 display="flex" flex-direction="column" align-items="flex-start" gap=12 background-color="#ffffff" color="#111111"
      text `last: ${log}` testId="out"
      button popovertarget="pop" testId="open"
        text "Open"
      column id="pop" popover="auto" testId="pop" padding=12 gap=8 background-color="#f4f4f4"
        text "An auto popover"
        button press=drop testId="drop"
          text "Drop focus"
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent <macos|web> --json --size 420x260 "tap open" "tap drop" state
"type root key Escape" state`. `root` takes no focus, so the agent's `type root key Escape` leaves the focus where
`blur()` left it (`Mac/AgentMac.swift:817-819`). The agent sends the key down the keyboard's route
(`Presenter.routeKey`, `KeyEvents.swift:124-134`; the window's key monitor calls the same, `ExactViewMac.swift:196-216`).
A drive that names a focusable target, such as `type drop key Escape`, focuses it first, which hides the bug.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| Open, Drop focus (`blur()`), Escape | macOS 26.6.2 (25G83), Apple Silicon | main `a10050516` | `state.focus` is all null after `blur()`. After Escape, and after a second Escape, `state.navigation.popover` is still `{popover: 5, source: 3, phase: "open"}`, and the window shows the popover | closed, as on the web | image (middle), record A, A2 |
| Same, web | Chrome 155.0.8059.39 (the agent's) | main `a10050516` | focus null after `blur()`; Escape closes the popover (the browser's own Popover API); focus stays null | (the reference) | image (left), record W |
| Control: Open, Escape (focus on Open) | macOS | main `a10050516` | `navigation.popover` null: closed | — | image (right), record C1 |
| Control: Open, Drop focus, `type drop key Escape` (the agent focuses Drop focus first) | macOS | main `a10050516` | closed; the focus returns to Open (node 3) | — | record C2 |

![X73: Escape after blur() on the Exact web, on macOS, and the macOS control](https://raw.githubusercontent.com/ccheever/exact2/0eb074309d3ec59a43b7edf57adce9c52bfdb8de/fw-issues-20261010/x73-escape-after-blur-web-macos.png)

Record (contract, commands, each step's focus and popover state):
https://raw.githubusercontent.com/ccheever/exact2/0eb074309d3ec59a43b7edf57adce9c52bfdb8de/fw-issues-20261010/x73-record.txt

The repro sends the agent's key events, not a keyboard's. The clone's RC-3 above is the real-key case. Not run: #327's
head, iOS and Linux.

## Constraints

- The web is the parity oracle (`CLAUDE.md`). HTML closes the topmost auto popover on the document's close request
  (Escape), whatever is focused. LLP 1021 §4 gives Escape to the host "per the spec".
- `testEscapeDoesNotCrossSessionsAndHeldEscapeDoesNotCloseTheDialog`
  (`host/apple/tests/ExactKitTests/PopoverMacTests.swift:148`): with two sessions in one window, an Escape at one
  session's focus must not close the other session's popover. The focus gate exists for that. With no view focused,
  `ExactView.ownsShortcutFocus` already names the one session that owns the window's keys.
- A focus in a view outside the session (another session's, a native module's, an embedding app's) keeps its Escape,
  as today.
- An input method's composition keeps Escape (`MenusMac.swift:332`;
  `testCompositionOwnsEscapeAndNoAutofocusLeavesFocusAtTheInvoker`), and a held Escape closes one popover
  (`escapeHeld`).
- `blur()` keeps leaving no node focused, as `document.activeElement === document.body` on the web.
- Main PR #327 (head `14493d253`) edits `MenusMac.swift` and `PresenterMac.swift`, but changes neither
  `MenuHost.key`'s focus gate nor `blurElement` (read from its diff, not run).

## Acceptance criteria

- In the repro on macOS, after Drop focus, one Escape closes the popover (`state.navigation.popover` is null), as on
  the web. The focus stays on no node, as the web's stays on BODY.
- The two-session test still passes: no Escape crosses sessions. With no view focused, only the session that
  `ownsShortcutFocus` names closes its popover.
- With the focus in a view outside the session, Escape closes none of the session's popovers.
- A `PopoverMacTests` case covers it: open the popover, `blurElement([])`, then Escape closes it.
- C1 and C2 behave as today.
