---
name: 20261008-x65-scroll-empty-area-press
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/317
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X65: on macOS a press on a `scroll`'s empty area reaches no node, so no ancestor hears its `pointerdown`

Moved to main `issues/20261009-macos-empty-scroll-pointer.md` (2026-10-09); tracked there.

## Summary

On macOS, when a `scroll` node's content is shorter or narrower than its port, a primary press in the uncovered part
(the port's ground, below or beside the content) reaches no node: no `pointerdown` (and so no `pointerup`) runs on the
`scroll` node or on any ancestor that declares one. On the web the press targets the scroll element itself, and
`pointerdown` bubbles to its ancestors.

## Why it arose

Task `20261008-popover-escape-parity` (#290) closes a pinned usage-segment popover on a press outside it, as Base UI's
`useDismiss` does. The Usage page's root column hears `pointerdown`/`pointerup` and hit-tests the popover. In the
clone's first agent drive (2026-10-08, session 1), a press on the page below the cards did nothing:
- The op was `tap usage-scroll clicks 1 at 600 700`, below the 604-pt content of the 788-pt `usage-scroll` port.
- The popover stayed pinned.
- The journal held no `pointerdown` action.

A press on a card's body in the same session dismissed it. The agent's click at a point is a real `NSEvent` sent
through `NSApp.sendEvent` (`"delivery": "platform"`), the path a hand's click takes.

## Clone workaround

Not blocking. [popover-escape-parity](../../tasks/closed/20261008-popover-escape-parity.md) puts a full-height column
(`usage-ground`, `min-height="100%"`) under the Usage page's scroll content, so every press in the port lands on a
node. Remove it when the host routes the ground's presses.

## Evidence and history

- Seen in the clone's agent drive (above); main `2531fb826` read unchanged on that path. Checked on 2026-10-08 against
  #266–#302 and an upstream search: not #281 (X51, too many nodes hearing a press) or #277 (scroll restore).
- Filed as [#317](https://github.com/ccheever/exact2/issues/317) on 2026-10-08, reproduced on main `b896050d7` in agent
  mode before filing (evidence under `file-x48-x68/` on `t3-code-evidence`).
