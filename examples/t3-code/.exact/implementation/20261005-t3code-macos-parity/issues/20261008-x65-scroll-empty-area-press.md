---
name: 20261008-x65-scroll-empty-area-press
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/317
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X65: on macOS a press on a `scroll`'s empty area reaches no node, so no ancestor hears its `pointerdown`

**Status (reclassified 2026-10-08):** Bucket 7, attempt withdrawn: #327 withdrew its #317 attempt. #317 is open and no fix is in progress.

## Summary

On macOS, when a `scroll` node's content is shorter or narrower than its port, a primary press in the
uncovered part (the port's ground, below or beside the content) reaches no node: no `pointerdown`
(and so no `pointerup`) runs on the `scroll` node or on any ancestor that declares one. On the web the
press targets the scroll element itself, and `pointerdown` bubbles to its ancestors. Where the content
does cover the point, the press reaches the node under it and bubbles as expected.

## Why this issue arose

Task `20261008-popover-escape-parity` (draft PR #290) closes a pinned usage-segment popover on a press
outside it, as Base UI's `useDismiss` does. The Usage page's root column hears `pointerdown`/`pointerup`
and hit-tests the popover. In the clone's first agent drive (2026-10-08, session 1), a press on the page
below the cards did nothing:
- The op was `tap usage-scroll clicks 1 at 600 700`, below the 604-pt content of the 788-pt `usage-scroll` port.
- The popover stayed pinned.
- The journal held no `pointerdown` action.

A press on a card's body in the same session dismissed it. The agent's click at a point is a real
`NSEvent` sent through `NSApp.sendEvent` (`AgentMac.swift`, `"delivery": "platform"`), the path a hand's
click takes.

## Where it lives (main 2531fb826, read)

- `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift`:
  - A node whose overflow scrolls moves its child node views into a `ChainingScrollView` whose `documentView` is a `FlippedView` (`CollectionMac.swift`). That is a plain `NSView`, not a `NodeView`.
  - The port's ground is that document view or the scroll view's clip view.
- `MouseChainMac.swift`:
  - `pointerPressed(_:)` walks up from the pressed `NodeView` to the innermost node that wants the pointer.
  - It is called only from `NodeView`'s own `mouseDown`/`rightMouseDown`/`otherMouseDown` (and the native buttons').
  - Nothing on the scroll view's own path (`FlippedView`, the clip view, `ChainingScrollView`) calls it.
- Unchanged since `e200397ec` on this path. `ChainingScrollView`'s later commits on main are the linked-capability split (`244ac708d`, `ca2ae9252`) and native buttons (`84e140ee5`).

## How to reproduce

The clone (above) reproduces it. A one-file app should show the same; this sketch was **not run**
(the task's live-session budget was spent):

```
component Repro
  state downs = 0
  action down(e: PointerEvent)
    downs = downs + 1
  view
    column width=400 height=300 pointerdown=down testId="outer"
      text `downs ${downs}` testId="count"
      scroll flex=1 min-height=0 width="100%" testId="port"
        box height=60 width="100%" background-color="#ccc" testId="content"
```

| Step | Actual (macOS, as seen in the clone) | Expected (Chrome) |
| --- | --- | --- |
| `tap content` (a press on the content) | `downs 1` | `downs 1` |
| `tap port clicks 1 at 200 200` (below the content, inside the port) | `downs` unchanged | `downs 2`: the press targets the scroll element and bubbles to `outer` |

## Not the same as #281 or the rest of #138

Checked on 2026-10-08 against #266–#302 and a search of the upstream issues for `pointerdown`, scroll
presses and hit testing:
- **#281** (X51) is about a click inside an open popover also pressing the page control under it. That is too many nodes hearing a press, not none.
- **#277** (the rest of #138) is about scroll restore and `scroll-padding`, not hit testing.
- No issue covers a press on a scroller's own ground.

## Requested support

On macOS, a press on a `scroll` node's ground (the part of its port its content does not cover) is that
node's press: `pointerdown`/`pointerup` run on the innermost node from the `scroll` node up that hears
them, as they do in Chrome. (A right-click there was not checked.)

## Clone-side

Not blocking. [popover-escape-parity](../tasks/closed/20261008-popover-escape-parity.md) puts a full-height
column (`usage-ground`, `min-height="100%"`) under the Usage page's scroll content, so every press in the
port lands on a node. Remove it when the host routes the ground's presses.

## Filed upstream (2026-10-08)

Filed as [#317](https://github.com/ccheever/exact2/issues/317) ([Bug] macOS: a press on a `scroll`'s empty area reaches no node, so neither it nor an ancestor hears `pointerdown`), reproduced on main `b896050d7` in agent mode before filing (evidence under
`file-x48-x68/` on `t3-code-evidence`). Under the framework vs T3 split (user, 2026-10-08) the fix is framework work;
the clone's side waits for main fix of #317, then an adoption round.

- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): attempt withdrawn; #317 stays open. `usage-ground` stays.
