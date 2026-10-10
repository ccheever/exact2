# macOS: an inline text run's `contextmenu` never runs; only a node's own handler does

**Status:** Open
**Systems:** host/apple macOS, pointer events, agent
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x77-macos-inline-run-contextmenu.md; docs/contract-grammar.md ("A `contextmenu` (a right-click, a long press) …"); issues/20261009-apple-inline-run-frame-layout.md

## Summary

Contract accepts `contextmenu=` on an inline run (a `text` inside a `text`), and the web runs it. In Chrome the
run is its own element, so a right-click on it runs the handler. The grammar says "The nearest `contextmenu`
handler runs" (`docs/contract-grammar.md`, Events). On macOS it never runs.

A run is not a view. The host finds the run under the pointer with `inlineTarget(at:handler:)`
(`host/apple/Sources/ExactKit/InlineText.swift:174`). It asks only for `press` (`Mac/NodeViewMac.swift:1358`,
`:1430`) and `hover` (`:334`). `rightMouseDown` (`NodeViewMac.swift:1443-1453`) calls `dispatchContextMenu`
(`Mac/MouseEventsMac.swift:85-95`) on the paragraph's view. That runs the view's own `contextmenu` or
`contextPopover` and never asks the run. The paragraph has neither, so nothing runs and the click goes on to
super. The ContextMenu key's walk (`MouseEventsMac.swift:63-75`) also visits views only. A `press` on the same
run works.

The agent does not show it. On macOS, `tap <run> contextmenu` sends no secondary click. `AgentMac.swift:496-499`
gives an inline run with no `at`, `mouse`, `clicks` or `auxclick` to `activateInline`, which presses the run or
follows its `href` (`session.follow`). It does not check the `contextmenu` word. So the drive answers `delivery:
"host-activation"` and runs the run's press or link instead. On the web the same op right-clicks the run.

## Why this arose

T3 Code's Markdown web links have the app's own menu (`externalLinkContextMenu.ts`): Link or Unlink from thread
for a pull request, Open in integrated browser, Open in system browser, Copy Link. The clone's `ChatRuns`
(`examples/t3-code/markdown.contract`, on the T3 branch) writes `contextmenu=linkMenu(run.href)` on each web link
run. A paragraph, list item, quote or heading that holds a link is laid out word by word (`FlowRuns`), each word
its own node, and there the link's menu opens. A Markdown table cell draws its runs inline in one text node
(`TableCell` → `ChatRuns`), so a long cell ends in one ellipsis at the column's edge. On macOS a right-click on a
cell's link runs nothing. The clone's shell menu (`T3TextContextMenu.swift`, task `shell-context-menu`, T3 PR
#407, merged `6bac646cc`) then answers the unanswered click with Copy Link, where the reference opens the link
menu.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `42fd5d99e4c884a366483aecd226f08442dee793`. Its
`app.ts` exports an empty `sources`. `contract build` compiles it with no warning. This is the part of the app
that X77 uses; the record has the full contract (it also carries X76's rows).

```text
component X77app
  state runCtx = 0
  state nodeCtx = 0
  state linkCtx = 0
  state runPresses = 0
  action runMenu
    runCtx = runCtx + 1
  action runPress
    runPresses = runPresses + 1
  action nodeMenu
    nodeCtx = nodeCtx + 1
  action linkMenu
    linkCtx = linkCtx + 1
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text testId="p" font-size=24
        text "Visit the "
        text "site" href="https://example.com" contextmenu=runMenu testId="run"
        text " today"
      text `run contextmenu=${runCtx}` testId="runctx"
      text testId="p2" font-size=24
        text "Visit the "
        text "site" press=runPress testId="run2"
        text " today"
      text `run press=${runPresses}` testId="runpress"
      text "node" href="https://example.com" contextmenu=nodeMenu testId="node" font-size=24
      text `node contextmenu=${nodeCtx}` testId="nodectx"
      link href="https://example.com" contextmenu=linkMenu testId="l"
        text "Example link" font-size=24
      text `link contextmenu=${linkCtx}` testId="linkctx"
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent <web|macos> --size 520x420 "tap p2 at 106 14" "tap p
contextmenu at 106 14" "tap node contextmenu" "tap l contextmenu" tree logs`. The point (106, 14) is inside
"site" in both paragraphs. The agent sends a real secondary click, through the application on macOS and over CDP
in Chrome.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| right-click on the run "site" (`tap p contextmenu at 106 14`) | macOS 26.6.2 (25G83), Apple Silicon | main `42fd5d99e` | `run contextmenu=0`. The log has no `contextmenu` line for it | `run contextmenu=1`, as on the web | image (right), record |
| Control: a click at the same point on a run with `press` (`tap p2 at 106 14`) | macOS | main `42fd5d99e` | `run press=1` (the point lands on the run) | — | image (right), record |
| right-click on a text node and on a `link` node, each with its own `contextmenu` | macOS | main `42fd5d99e` | `node contextmenu=1`, `link contextmenu=1` | — | image (right), record |
| The same drive, web | Exact web (JS target) in Chrome 155.0.8059.39 | main `42fd5d99e` | `run contextmenu=1`, `run press=1`, `node contextmenu=1`, `link contextmenu=1` | (the reference) | image (left), record |
| `tap run contextmenu` (aimed at the run itself) | macOS / web | main `42fd5d99e` | macOS: `delivery: "host-activation", native: "inline-text"`, `run contextmenu=0`. Web: a right-click at the run, `run contextmenu=1` | macOS delivers a secondary click at the run | record |

![X77: a contextmenu on an inline run, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x77-inline-run-contextmenu-web-macos.png)

Record (contract, commands, both drives, the agent's run target):
https://raw.githubusercontent.com/ccheever/exact2/3d74174e0644477a05bcb95c1802f7af3c3555f0/fw-issues-20261010g/x77-record.txt

Not run: #327's head, `contextPopover` on a run, a run that wraps across lines, the ContextMenu key on a
focused link run, iOS (a long press), Linux, and real hardware input (the agent's secondary click was used).

## Constraints

- The web is the parity oracle (`CLAUDE.md`), the author wrote the handler, and the compiler accepts it. LLP
  1115: author > platform > CSS default.
- The order matches a node's: the secondary button's `pointerdown`, then `contextmenu` at its point on the
  button's down (`docs/contract-grammar.md`). The nearest handler runs, so a run's handler runs before its
  paragraph's, and a paragraph's handler still runs for a click on a run that has none (as today).
- A run whose `contextmenu` ran ends the click, as `dispatchContextMenu` returning true does for a node. No host
  text menu opens and nothing goes up the responder chain.
- A run's `press`, `hover` and link activation work as today.
- The agent's `tap <run> contextmenu` delivers a secondary click at the run on macOS, as it does on the web, and
  does not activate it.
- Main PR #327 (head `14493d253`) changes how hover reaches a run (`MouseChainMac.swift`, `TextInteraction.swift`
  `hoverInline`, `AgentMac.swift`'s hover path) and routes `mouseEntered`/`mouseMoved`/`mouseExited` through
  `presenter.trackPointer`. It does not change `rightMouseDown`, `dispatchContextMenu`, or the agent's inline tap
  (read from its diff, not run).

## Acceptance criteria

- In the repro on macOS, `tap p contextmenu at 106 14` shows `run contextmenu=1`, and the log has a
  `contextmenu view <run> (runMenu)` line. `tap run contextmenu` does the same.
- `tap p2 at 106 14` still shows `run press=1`, and the node and link `contextmenu` counts are unchanged.
- A macOS XCTest covers a secondary click on an inline run that declares `contextmenu`.
