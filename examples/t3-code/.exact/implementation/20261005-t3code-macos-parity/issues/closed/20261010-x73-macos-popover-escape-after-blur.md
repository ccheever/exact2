---
name: 20261010-x73-macos-popover-escape-after-blur
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X73: On macOS Escape closes no auto popover after an action's `blur()`, so the Pull Requests page's Filters stayed open

Moved to main `issues/20261010-macos-popover-escape-after-blur.md` (2026-10-10), where it is tracked. Main PR
[#401](https://github.com/ccheever/exact2/pull/401) filed it (merged `d413487a8`).

## Summary

On macOS the host's popover Escape (`MenuHost.key`, `host/apple/Sources/ExactKit/Mac/MenusMac.swift`) closes the top
auto popover only when the focus owner is a view inside the session's viewport. An action's `blur()`
(`Presenter.blurElement`: `makeFirstResponder(nil)`) makes the window itself the first responder, so `key` returns
false and Escape closes nothing. On the web, Escape closes the top auto popover whatever has the focus, BODY included.

## Why it arose

FW-3 (T3 PR #378, task `20261010-audit-wave-followups-3`) drops the focus with `blur()` when a Filters submenu closes by
Escape with the pointer outside its row, because Base UI leaves the focus on BODY there. In the attended real-input
session realinput-1010c (RC-3) the user went Pull Requests › Filters › Author, clicked in "Search authors" and pressed
Escape, which closed Author. A second Escape left Filters open; only a click outside closed it (2 of 2). The reference
closes Filters there. #378's agent drive had passed, because its `type <row> key Escape` focused the row first. Task
`20261010-realinput-1010c-fixes` found the cause by reading the host code on the feature branch's framework. The draft
stayed local as `EXACT2-GAPS.md` X73 until the coordinator filed it.

## Clone workaround (realinput-1010c-fixes)

`PrFiltersMenu` (`examples/t3-code/pages-prs.contract`): `closeSub` focuses `pr-filters-rest` instead of calling
`blur()`. That is a 1-pt rest box inside the Filters popover (`tabindex=-1`, `aria-hidden`, `pointer-events="none"`,
outside both KeyMenus and the submenu's row). No row is lit, ↓ moves nothing, it draws no ring, and the next Escape
reaches the host's popover Escape, which closes Filters and gives the focus back to the Filters button, as the
reference does. `menu-keys.test.ts` FW-3 checks that `closeSub` focuses the rest box. The clone's other `blur()`
callers were checked by reading (2026-10-10). None leaves an auto popover open that only the host's Escape closes:
`PrPageBack` and `UsageKeys` leave their page with nothing shown, the menu and dialog openers blur for a popup that
takes the focus by `autofocus` and closes on its own Escape shortcut, and the Browser URL field's `go` opens nothing.
When main fixes the host, re-check these and remove the rest box (`EXACT2-GAPS.md` X73).

## Evidence and history

- 2026-10-10, realinput-1010c RC-3 (real keys and pointer): Filters stayed open after the second Escape. Image:
  [G-fw3-fw4-filters.png](https://raw.githubusercontent.com/ccheever/exact2/e8fabfeed724dce27c432773cd615219967a8fea/realinput-1010c/G-fw3-fw4-filters.png).
- 2026-10-10, T3 PR [#399](https://github.com/ccheever/exact2/pull/399) (merged `df2cc67e1`): RC-3 passes with the rest
  box. Before, the focus was on no node after the first Escape, and Filters was still open after the second. After,
  the focus is on the rest box, ↓ moves nothing, and the second Escape closes Filters and focuses the Filters button.
  Image: [rc3-filters-second-escape.png](https://raw.githubusercontent.com/ccheever/exact2/c000b19f72e89e5bf01e8d0e3b1bb058ce992f6b/realinput-1010c-fixes/rc3-filters-second-escape.png).
- 2026-10-10, main PR #401 reproduced the gap on main `a10050516` in a one-file app: an auto popover with a button
  whose action calls `blur()`. The agent sent Escape to the root, which takes no focus, so the focus stayed where
  `blur()` left it. On macOS 26.6.2 the popover stayed open after one Escape and after two
  (`state.navigation.popover` phase `open`). The Exact web build in Chrome 155 closed it. Two macOS controls closed
  it: Escape with the focus on the invoker, and Escape sent to the popover's button, which the agent focuses first.
  Main PR #327 changes neither `MenuHost.key`'s focus gate nor `blurElement` (read from its diff). Image:
  [x73-escape-after-blur-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/0eb074309d3ec59a43b7edf57adce9c52bfdb8de/fw-issues-20261010/x73-escape-after-blur-web-macos.png);
  record: [x73-record.txt](https://raw.githubusercontent.com/ccheever/exact2/0eb074309d3ec59a43b7edf57adce9c52bfdb8de/fw-issues-20261010/x73-record.txt).
