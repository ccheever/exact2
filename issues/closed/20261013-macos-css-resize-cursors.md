# macOS: CSS resize cursors use the legacy shapes and diagonals show a crosshair

**Status:** Closed
**Resolution:** fixed by #527
**Systems:** host/apple macOS, cursor
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-13
**Related:** T3 STATUS DP-4 (DP-4b) and G4-16; https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/STATUS.md

## Summary

`CSSCursor.value` (`host/apple/Sources/ExactKit/Mac/CSSCursor.swift`) maps `n-resize`, `e-resize`, `s-resize` and
`w-resize` to AppKit's legacy `resizeUp`/`resizeRight`/`resizeDown`/`resizeLeft` (24×24), `ns-resize` and
`row-resize` to `resizeUpDown` (24×28), `ew-resize` and `col-resize` to `resizeLeftRight` (30×24), and all six
diagonals (`ne-`, `nw-`, `se-`, `sw-`, `nesw-`, `nwse-resize`) to the crosshair (24×24), because AppKit 14 had no
diagonal. macOS 15 added the frame-resize cursors (`NSCursor.frameResize(position:directions:)`, `columnResize`,
`rowResize`) and marks the legacy six "to be deprecated" in their favour. Chrome on macOS 15+ uses them
(`ui/base/cocoa/cursor_utils.mm`, "Match Safari's cursor choices"): an edge (`n-` … `w-resize`) and a corner
(`ne-` … `sw-resize`) is the frame cursor at that position, `.outward`; `ns-`, `ew-`, `nesw-` and `nwse-resize` are
the frame cursor at the top, left, top-right and top-left, `.all`; `col-resize` and `row-resize` are `columnResize`
and `rowResize`. Their images are 18×28 (vertical), 24×18 (horizontal) and 22×22 (diagonal).

**The outer half of a handle that straddles its parent's edge**, which T3 saw show the arrow, does not reproduce on
main. AppKit's cursor-rect lookup follows the view hit test, not the superview's bounds (a plain `NSView` parent
loses the outer half, a parent whose `hitTest` reaches its overflow keeps it), and `NodeView.hitTest` reaches CSS
visible overflow (`overflowHit`). A fixture of T3's player (a `z-index` 40 box, an `overflow: hidden` column, a
`pointer-events: none` border, eight straddling handles with `pan`, text under it) shows each handle's cursor 6 pt
outside the edge as inside it, before and after a move or a resize of the player.

## Why this arose

The T3 Code clone's real-input session `realinput-1011c` (STATUS DP-4, B5) found that no zone of the floating
player's resize frame shows the reference's cursor: on its eight zones, probed 2–4 points outside the player's
edge, N, S, E and SE showed the arrow, W and NW `resizeLeftRight` (30×24) and NE and SW the crosshair (24×24).
G4-16 (B2) found the browser panel's resize cursors' images differ in size from the reference's.

## Reproduction

A box whose eight handles straddle its edges, each with its `cursor` (T3's player has this shape):

```text
box position="relative" width=200 height=120 margin=40
  box position="absolute" top=-6 left=20 right=20 height=12 cursor="n-resize"
  box position="absolute" top=-6 left=-6 width=12 height=12 cursor="nw-resize"
  ...
```

On macOS 15+, hover each handle: it shows the legacy shape, or the crosshair on a corner. Chrome shows the
frame-resize cursor.

| Scenario | Platform | Actual | Expected (Chrome 155, macOS) |
|---|---|---|---|
| `n-resize` … `w-resize` | macOS 26.6.2 (25G83) | `resizeUp` … `resizeLeft`, 24×24 | `frameResize(position: .top …, directions: .outward)`, 18×28 / 24×18 |
| `ne-resize` … `sw-resize`, `nesw-resize`, `nwse-resize` | same | the crosshair, 24×24 | `frameResize(position: .topRight …)`, 22×22 |
| `ns-resize`, `ew-resize` | same | `resizeUpDown` 24×28, `resizeLeftRight` 30×24 | `frameResize(position: .top / .left, directions: .all)` |
| `col-resize`, `row-resize` | same | `resizeLeftRight`, `resizeUpDown` (on macOS 26 the very objects `columnResize` and `rowResize` return) | `columnResize`, `rowResize` |

## Current scope

The macOS 15+ mapping, keeping the macOS 14 fallbacks. Not in scope: `move`, `all-scroll`, `cell`, `help`, `wait`
and `progress`, which Chrome draws from private Core Cursor images AppKit has no public shape for. 2026-10-14: the
outer-half arrow did not reproduce on main (Summary); T3's re-run of DP-4's eight zones decides whether the clone has
a cause of its own, filed then with the clone's tree at the point.
