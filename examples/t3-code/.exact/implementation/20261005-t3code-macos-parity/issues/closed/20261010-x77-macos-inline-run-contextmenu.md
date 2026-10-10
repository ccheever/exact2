---
name: 20261010-x77-macos-inline-run-contextmenu
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X77: On macOS an inline run's `contextmenu` never runs, so a Markdown table cell's link opens the shell menu, not the link menu

Moved to main `issues/20261010-macos-inline-run-contextmenu.md` (2026-10-10), where it is tracked. Main PR
[#410](https://github.com/ccheever/exact2/pull/410) filed it (merged `474999b9f`).

## Summary

Contract accepts `contextmenu=` on an inline run (a `text` inside a `text`), and the web runs it. The macOS host
never does, because it dispatches `contextmenu` per view only (`MouseEventsMac.swift` `dispatchContextMenu`). A run
is asked only for `press` and `hover`. On macOS the agent's `tap <run> contextmenu` also activates the run (presses
it or follows its `href`) instead of right-clicking it.

## Why it arose

`EXACT2-GAPS.md`'s "Text context menu" entry S2, from task `20261010-shell-context-menu` (T3 PR #407, merged
`6bac646cc`). The reference's Markdown web links have the app's own menu (`externalLinkContextMenu.ts`). The
clone's `ChatRuns` (`markdown.contract`) writes `contextmenu=linkMenu(run.href)` on each web link run, and
`external-link-menu.ts` builds the menu. A paragraph, list item, quote or heading that holds a link is laid out
word by word (`FlowRuns`; `flow_tokens` in `macos/src/markdown.rs` flows a block on any `href`), each word its own
node, so the menu opens there. A Markdown table cell draws its runs inline in one text node (`TableCell` →
`ChatRuns`) so that a long cell ends in one ellipsis, and there the run's `contextmenu` never runs.

## Clone workaround (shell-context-menu)

Paragraph links need none: `FlowRuns` makes each link word a node. A table cell's web link is the difference that
stays. Its right-click goes unanswered, so the module's tail responder (`T3ShellMenuTail`, `T3TextContextMenu.swift`)
opens the shell's menu with Copy Link, taken from the run's accessibility URL. The reference opens the link menu
(Link or Unlink from thread, Open in integrated browser, Open in system browser, Copy Link). When main fixes the
host, re-drive a table cell's link: it should open the link menu with no clone change.

## Evidence and history

- 2026-10-10, main PR #410 reproduced it on main `42fd5d99e` in a one-file app. The paragraph `Visit the [site]
  today` has `contextmenu=runMenu` on the run, a twin paragraph has `press=runPress` on its run, and a text node
  and a `link` node each have their own `contextmenu`. The agent sent real secondary clicks. The Exact web in
  Chrome 155 ran all four (`run contextmenu=1`, `run press=1`, `node contextmenu=1`, `link contextmenu=1`). On
  macOS 26.6.2 the run's `contextmenu` stayed 0 while the press at the same point, the node and the link ran.
  Main PR #327 changes how hover reaches a run, but not `rightMouseDown`, `dispatchContextMenu` or the agent's
  inline tap (read from its diff). Image:
  [x77-inline-run-contextmenu-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x77-inline-run-contextmenu-web-macos.png);
  record: [x77-record.txt](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x77-record.txt).
