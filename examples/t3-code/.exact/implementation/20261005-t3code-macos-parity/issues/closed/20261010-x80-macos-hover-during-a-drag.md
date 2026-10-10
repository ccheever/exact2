---
name: 20261010-x80-macos-hover-during-a-drag
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X80: On macOS hover does not follow the pointer during a mouse drag, so the diff gutter's "+" stays on the pressed line

Moved to main `issues/20261010-macos-hover-during-a-drag.md` (2026-10-10), where it is tracked. Main PR
[#414](https://github.com/ccheever/exact2/pull/414) filed it (merged `34655f4de`).

## Summary

On macOS, during a mouse drag with the button held, the hover stays on the node the button went down on. It stays
there for the whole drag and after the release, until the pointer moves again. The pointer reaches `hover` only through
each node's `NSTrackingArea` (`NodeViewMac.swift` `syncHoverTracking`), whose options have no
`.enabledDuringMouseDrag`. `mouseDragged` feeds the held pointer, the press and the selection, but never the hover.
The resting-pointer hit test (`hoverUnderPointer`) stops while a button is down (`restingPointer`). On the web,
Contract `hover` (`pointerenter`/`pointerleave`) and CSS `:hover` follow the pointer through the drag
(`buttons` 1), and the web host captures no pointer.

## Why it arose

Task `20261010-realinput-1010f-followups` (RF-3, T3 PR #413, open at filing). The reference's diff gutter
(@pierre/diffs) starts a line selection on a press on a line number or on the "+". The "+" follows the pointer over
the lines (`placeUtility`) while the lines from the press to the pointer paint. The clone's gutter (`DiffCell`,
`diff-line-drag.ts`) holds the pointer on the pressed node and names the line under it with `elementFromPoint` on
each `pointermove`, so the selection paints (lines 3–5 in the session). The "+" is placed by the lines' `hover`,
and in the real-input session it stayed on the pressed line 3. The task declared it in its EXACT2-GAPS section from
the host's source and did not file it.

## Clone workaround

None. During a drag, the gutter's "+" stays on the pressed line, where the reference's follows the pointer. The
selection, which is what the drag is for, paints correctly. When main fixes the host, re-drive the gutter's drag under
a real pointer: the "+" should follow with no clone change.

## Evidence and history

- 2026-10-10, main PR #414 reproduced it on main `efeb92cd2` in a one-file app. Column A has five rows that hear
  `hover`. Column B has five rows that also hear `pointerdown`/`pointerup` (the pressed row holds the pointer, as the
  gutter does). The macOS drives took three attempts.
  - The agent's drag, run twice, showed only the symptom: the agent's input does not go through tracking areas.
  - Then the system cursor, moved by `cliclick` (a press on A1 and 31 drag steps to A4). The log stayed `0A1+` while
    the button was held over A4 and after the release. The first free move gave `1A1- 2A4+`. B1 → B4 was the same.
  - The Exact web in Chrome 155 (the agent's CDP mouse, button held) logged `… 4A3+ 5A3- 6A4+` during the drag.
  - A Chrome probe showed CSS `:hover` on the row under the pointer while the button was held.
  - A `swiftc` AppKit oracle got no `mouseEntered`/`mouseExited` during a synthesized drag, with or without
    `.enabledDuringMouseDrag`. So the flag's effect is read from `NSTrackingArea.h`, not observed, and main's issue
    leaves the fix open: the flag, or a hit test on each drag.

  It is not X62 (#322, which nodes a free pointer hovers). Open main PR #327 does not change it: its `trackPointer`
  returns early while a pointer is held, and its tracking areas have the same options (read from its diff).
  Image:
  [x80-hover-during-drag-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/931a320fd932afec2e89a9e4bd83b6369a0c6a22/fw-issues-20261010j/x80-hover-during-drag-web-macos.png);
  record: [x80-record.txt](https://raw.githubusercontent.com/ccheever/exact2/931a320fd932afec2e89a9e4bd83b6369a0c6a22/fw-issues-20261010j/x80-record.txt).
