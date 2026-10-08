---
name: 20261008-x62-hover-outside-the-box
plan: 20261005-t3code-macos-parity
status: filed
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/322
reproduced_on: feature branch c0475fbaa's framework with a one-file app and a real pointer; the hover code (host/apple/Sources/ExactKit/Mac MouseChainMac.swift, PresenterMac.swift `hover`, NodeViewMac.swift `syncHoverTracking`/`mouseMoved`/`mouseExited`) is unchanged on main 9314e7a81
---

# X62: macOS hover ignores a node's overflowing descendants: a hover card beside its trigger hears no pointer, and the trigger hears a leave as the pointer moves into it

## Summary

On the macOS host a `hover` handler is an `NSTrackingArea` with `.inVisibleRect`, on the node's own bounds,
and the presenter keeps one hovered node (LLP 1008 §5, "the previously hovered node's leave sent before the
new one's enter"). Three results disagree with the web, where `hover` is `pointerenter`/`pointerleave`
(host/web/glue.js, host/web-js/rt.js) and both count every descendant, an overflowing one included:

1. **A node outside its parent's box hears no pointer.** Its tracking area is clipped by every ancestor's
   bounds (`visibleRect`), even when no ancestor clips its drawing (`overflow: visible`). Only the frame's
   resting-pointer hit-test (#139) reaches it, once, after a batch that makes or moves boxes. CSS: the
   overflowing child is hit-tested and hovered like any other.
2. **Moving into an overflowing descendant leaves the parent.** The parent's tracking area ends at its bounds,
   so the pointer crossing into its absolutely positioned child sends the parent `hover(false)`. CSS:
   `pointerleave` fires only when the pointer leaves the element and all its descendants.
3. **Overlapping hover nodes hand the hover back and forth on every move.** Both tracking areas hear each
   move and the presenter keeps one hovered node, so each move sends a leave and an enter to each of them
   (the last one, the topmost, wins at rest). CSS: the topmost hit node and its ancestors are hovered; an
   occluded sibling is not.

A hover card is the usual case: a `PopoverTrigger openOnHover` or a hoverable tooltip draws its popup next to
its trigger, outside the trigger's box. On macOS the popup cannot keep itself open, and the trigger's leave
closes it as the pointer moves in.

## Why this issue arose

### The T3 Code behavior
Base UI's hover popovers (`PopoverTrigger openOnHover`) close through `safePolygon()` and the popup's own hover
(`useHoverFloatingInteraction`), so the pointer can move from the trigger into the popup and stay there:
`PullRequestBaseFreshnessWarning` (closeDelay 120), `AccessScopeSummary` (closeDelay 100), the Usage page's
`PoolSegment` popover (closeDelay 0). Every popup is portaled to the body.

### What exact2 does today
Probe (one-file app from `exact new`, `bun exact.mjs mac`, launched normally, real pointer moved with cliclick
in 2-8 pt steps, read back from full-screen captures; 2026-10-08 06:12-06:32 UTC, under the real-input lock):

```contract
component Probe
  state log = ""
  state n = 0
  state aOver = false
  action ev(name: string, over: bool)
    n = n + 1
    log = `${n}${name}${over ? "+" : "-"} ${log}`
  action hA(over: bool)
    aOver = over
    ev("A", over)
  view
    main width="100%" height="100%" padding=24 position="relative"
      text `log: ${log}` font-size=13
      // 1, 2: a box and its card below it, outside the box
      box position="absolute" left=24 top=140 width=160 height=20 background-color="#3b82f6" hover=hA
        when aOver
          box position="absolute" left=0 top=20 width=160 height=80 background-color="#93c5fd"
      box position="absolute" left=300 top=140 width=160 height=20 background-color="#10b981" hover=ev("B")
        box position="absolute" left=0 top=20 width=160 height=80 background-color="#6ee7b7" hover=ev("Bcard")
          box position="absolute" left=30 top=30 width=100 height=24 background-color="#047857" hover=ev("Bbtn")
      // 3: a node under a later, overlapping sibling
      box position="absolute" left=560 top=140 width=160 height=80 background-color="#f59e0b" hover=ev("U")
      box position="absolute" left=590 top=160 width=100 height=40 background-color="#7c2d12" hover=ev("O")
```

- Pointer onto A, then down into its card: `0A+ … 3A-`, and the card goes. Web: A stays hovered.
- Pointer onto B, into its card, onto the button, around it, back to the card, out: `0B+ 1B- 2Bcard+` and
  nothing more. The card's enter came from the resting-pointer hit-test after B's leave; the button never
  heard an enter; leaving the card sent no leave (the card stays "hovered" until another hover event).
- Five moves inside O: `5U- 6O+ 7O- 8U+ 9U- 10O+ …` (twenty events), ending `O+` at rest.

Agent drives do not show 1 or 2: `tap X hover` hit-tests the view under the point up to the nearest node with
a `hover` handler, so a card that is a child of its trigger is "inside" it.

### Where the clone hits it
- `pages-pr-actions.contract` PaFreshnessMark: the base branch's hover card closed as the pointer moved into it
  (#262, real-input batch row 6b).
- `usage-pooled.contract` SegmentPopup: the Usage segment's popover closed as the pointer moved into it, so the
  account email could not be revealed by pointer (#263, batch bug 10).
- `connections-network.contract` NetScopes: the same for "N scopes" (closeDelay 100), behind its clipping (#248).

## The clone's fix (fix-hover-cards)
- Hover cards and the tooltips in clipping scroll areas are drawn by a window-level hover layer
  (`hover-layer.contract`, T3Window), whose boxes span the window, so a card there hears the pointer; the Usage
  page draws its popover in its scroll content, inside every box around it.
- A close delay on the root's hover clock (Contract has timers only in the root: `task`) keeps a card open
  while the pointer crosses into it: Base UI's closeDelay, or 50 ms where it has none (safePolygon).
- A hover node inside the card (its buttons, the email) reports to the card, so (3) cannot close it. The real-pointer
  session showed (3) still takes the email's own tooltip away: the email's hover box and the card's hover trade the
  hover on each move, and the tooltip unmounts.
- A press outside the Usage page drops a hover-opened popover, as Base UI's outside press does. Without it, a
  pinned popover stayed open after a press in the sidebar: while the pointer crossed the sidebar's resize handle,
  each move gave the handle an enter and then a usage segment (far from the pointer) an enter again, and the last
  enter before the press was the segment's.

## Why it must be resolved
Every app with hover cards (Base UI, Radix, Floating UI popovers) needs the pointer to reach the popup. On
macOS each one must today draw its popups in a window-level layer with its own state, frames and timers, and
the web and macOS disagree for the same Contract (the parity rule of CLAUDE.md).

## Ask (proposal)
- macOS: a hover handler's area is the node's subtree, as `pointerenter`/`pointerleave` are: the tracking area
  (or a hit-test on each move) covers overflowing descendants, and moving into a descendant sends no leave.
- macOS: hover follows the hit-test (the topmost node and its ancestors), not every tracking area under the
  pointer.
- Contract: a hover-opened top-layer popover (HTML's `interestfor`, LLP 1021 §5.1 refuses it today), so a hover
  card escapes clipping without a hand-made layer.

## Status
Filed as [#322](https://github.com/ccheever/exact2/issues/322) by the coordinator (2026-10-08), fixed by
[#327](https://github.com/ccheever/exact2/pull/327) (open): hit-tested hover takes in the ancestor path and
overflowing descendants. Once #327 is adopted, only the X62 part of the hover layer's reason changes; the layer stays
for the clipping (#251, #248) and as the reference's portal.

Reproduced with the probe above on the feature branch's framework; the hover code is unchanged on main `9314e7a81`
(PresenterMac.swift and NodeViewMac.swift differ there only in key handling). Seen again in fix-hover-cards' real-pointer
session (2026-10-08, 10:41-11:22Z): the email tooltip and the sidebar press above
(`t3-code-evidence:fix-hover-cards/real-session-record.txt`).
