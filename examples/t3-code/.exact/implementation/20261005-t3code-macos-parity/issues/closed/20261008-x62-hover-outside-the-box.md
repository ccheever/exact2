---
name: 20261008-x62-hover-outside-the-box
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/322
reproduced_on: feature branch c0475fbaa's framework with a one-file app and a real pointer; the hover code (host/apple/Sources/ExactKit/Mac MouseChainMac.swift, PresenterMac.swift `hover`, NodeViewMac.swift `syncHoverTracking`/`mouseMoved`/`mouseExited`) is unchanged on main 9314e7a81
---

# X62: macOS hover ignores a node's overflowing descendants: a hover card beside its trigger hears no pointer, and the trigger hears a leave as the pointer moves into it

Moved to main `issues/20261009-macos-subtree-hover.md` (2026-10-09); tracked there.

## Summary

On the web, `hover` is `pointerenter`/`pointerleave`, which count every descendant, an overflowing one
included. On macOS a `hover` handler is a tracking area on the node's own bounds, clipped by every
ancestor, and the presenter keeps one hovered node. So (1) a node outside its parent's box hears no
pointer, (2) moving into an overflowing descendant leaves the parent, and (3) overlapping hover nodes hand
the hover back and forth on every move. A hover card is the usual case: drawn next to its trigger, outside
the trigger's box, it cannot keep itself open on macOS, and the trigger's leave closes it as the pointer
moves in.

## Why it arose

### The T3 Code behavior
Base UI's hover popovers (`PopoverTrigger openOnHover`) close through `safePolygon()` and the popup's own hover
(`useHoverFloatingInteraction`), so the pointer can move from the trigger into the popup and stay there:
`PullRequestBaseFreshnessWarning` (closeDelay 120), `AccessScopeSummary` (closeDelay 100), the Usage page's
`PoolSegment` popover (closeDelay 0). Every popup is portaled to the body.

### Where the clone hits it
- `pages-pr-actions.contract` PaFreshnessMark: the base branch's hover card closed as the pointer moved into it
  (#262, real-input batch row 6b).
- `usage-pooled.contract` SegmentPopup: the Usage segment's popover closed as the pointer moved into it, so the
  account email could not be revealed by pointer (#263, batch bug 10).
- `connections-network.contract` NetScopes: the same for "N scopes" (closeDelay 100), behind its clipping (#248).

## Clone workaround (fix-hover-cards)

Built by [fix-hover-cards](../../tasks/closed/20261008-fix-hover-cards.md) (#307):
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

Once main's subtree hover is adopted, only the X62 part of the hover layer's reason changes; the layer stays for
the clipping (#251, #248) and as the reference's portal.

## Evidence and history

- Reproduced on the feature branch's framework (`c0475fbaa`) with a one-file probe app launched normally and a
  real pointer (cliclick, 2-8 pt steps, full-screen captures; 2026-10-08 06:12-06:32 UTC, under the real-input
  lock): pointer onto box A then into its card logged `0A+ … 3A-` and the card went; into B's card and its
  button logged `0B+ 1B- 2Bcard+` and nothing more; five moves inside an overlapping sibling logged twenty
  events (`5U- 6O+ 7O- 8U+ …`). Agent drives do not show (1) or (2): `tap X hover` hit-tests up to the nearest
  node with a `hover` handler, so a card that is a child of its trigger is "inside" it.
- The hover code is unchanged on main `9314e7a81`.
- Seen again in fix-hover-cards' real-pointer session (2026-10-08, 10:41-11:22Z): the email tooltip and the
  sidebar press above (`t3-code-evidence:fix-hover-cards/real-session-record.txt`).
- Filed as [#322](https://github.com/ccheever/exact2/issues/322) by the coordinator on 2026-10-08.
