---
name: 20261010-x71-pointermove-without-press-capture
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X71: A popup cannot hear pointer moves without holding the presses inside it

Moved to main `issues/20261010-pointer-events-reach-ancestors.md` (2026-10-10); tracked there. Filed on main by PR #386 (merged `e281d83e9`).

## Summary

Base UI's menus take the keyboard highlight back to the row under the pointer on any pointer move (`onMouseMove`), so
after ArrowDown moves the highlight away, a small move of the resting pointer brings it back. The clone's painted menus
(audit-wave-followups-4, #383) move the highlight only when the pointer enters a row (`pointerenter`/hover). Hearing every
move needs a `pointermove` handler on the popup or its rows; in Exact that handler also keeps the presses inside the menu
from reaching the window's light dismiss, and forwarding them grew the plan from 27.1 to 33.7 MB. So the clone keeps the
enter-only rule.

## Where the clone carries it

`EXACT2-GAPS.md` "Painted menus: one highlight" and the closed task `20261010-audit-wave-followups-4` (FX-2).
