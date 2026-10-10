---
name: 20261010-x70-aria-disabled-focusable
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X70: A control cannot be disabled and still take the focus (`aria-disabled`)

Moved to main `issues/20261010-aria-disabled-focusable-control.md` (2026-10-10); tracked there. Filed on main by PR #386 (merged `e281d83e9`).

## Summary

T3 Code's sidebar "Check for updates" control (`SidebarUpdatePill`) and its Settings nav twin are `aria-disabled` when
no update feed exists: Tab still reaches them (a focus ring, no action), and assistive technology reads them as
dimmed. Contract has no `aria-disabled` (`contract vocab`), and a `disabled` button refuses the focus
(`NavigationRules.focusRefusal`), as a disabled `<button>` does on the web. So the clone's control (merged in #368,
`sidebar-icons.contract` `SidebarUpdatePill`) is skipped by Tab. Its label, dimmed look, tooltip and no-op press match.

## Why it matters

A small parity gap in keyboard order. The web names the behavior (`aria-disabled="true"` on a focusable control); an app
can only approximate it with an enabled button that ignores its press, which loses the accessibility state.

## Where the clone carries it

`EXACT2-GAPS.md` "Desktop update controls (no feed): declared differences".
