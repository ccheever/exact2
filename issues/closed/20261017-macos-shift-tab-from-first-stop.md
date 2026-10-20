# macOS: Shift-Tab from the first stop of a key view loop does nothing

**Status:** Closed
**Resolution:** fixed: the key view loop links its start before its ring, so the last stop is the first's previous
**Systems:** Apple host (key view loop)
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** realinput-1017l failure 2

## Problem

With Keyboard Navigation on, Shift-Tab from the first stop of the window's
key view loop kept the focus there (the T3 Code clone's 'Go to threads', 6 of
6), while Tab from the last stop wrapped to the first. AppKit wraps a key view
loop both ways. A dialog's sheet and a popover AppKit shows build their loops
the same way.

## Cause

`Presenter.keyViewLoop` linked the stops into a ring and then set the loop's
start (the viewport, a sheet's or popover's content) to point at the first
stop. Setting `nextKeyView` makes the receiver the next view's
`previousKeyView`, so the first stop's previous became the start, which is no
key view and has no previous: `previousValidKeyView` was nil, and
`selectPreviousKeyView` found nothing. A field's editor and AppKit's own
buttons take that path; Exact's buttons found no `keyViewBefore` entry either.

## Fix

The start's link is set before the ring's, so the last stop's link is the
first stop's previous. `KeyboardNavigationMacTests` covers Shift-Tab from the
first stop in both Keyboard Navigation modes (a field, a button, a native
button), and the loops of a dialog's sheet and a popover.
