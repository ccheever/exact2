# macOS: hover stops following the pointer after a context menu closes

**Status:** Closed
**Resolution:** fixed: MenuHost.track ends the pointer hold of the press whose release NSMenu took, so hover follows the pointer once a menu closes (ContextMenuMacTests)
**Systems:** Apple host (MenuHost, NSMenu, pointer hold, hover)
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** LLP 1005 §3 (the pointer hold), LLP 1021 §5.1 (context menus), #139 (hover follows layout), realinput-1017l failure 3

## Problem

Rows that bind `hover=` and `contextPopover=` (an `NSMenu`) under an
ancestor that hears `pointerdown` (the T3 Code clone's window root): right-click
a row, close the menu with Escape or a pick, then move the pointer over the
rows. Their hover no longer follows the pointer: a row is entered and shows
nothing, while leaves still arrive. One left click anywhere restores it. In a
hand-built AppKit window, and on the web, hover follows the pointer as soon as
the menu has closed.

## Cause

`rightMouseDown` holds the pointer (`pointerPressed`: `pointerHeld`,
`pointerSource`, `pointerPath`) for the nodes from the row up that hear
`pointerdown`/`pointerup`, until the button comes up. The context menu pops
up on the next turn, while the button is still down, and `NSMenu`'s tracking
takes the `rightMouseUp`: the view never hears it, so `pointerReleased` never
ran and the hold stayed. While a hold stands, `trackPointer` and
`restingPointer` deliver no hover (a held contact owns move delivery); only
tracking-area exits still reached `hoverAt`. A left click's `mouseUp` ended
the stale hold, which is why one click restored hover.

## Fix

`MenuHost.track` ends the hold once `popUp` returns with no button down:
`pointerReleased`, which sends `pointerup` to the nodes that heard the down
(the button did come up, in the menu; LLP 1005's hold lasts "until the
button comes up, wherever that is") and hit-tests the resting pointer, as a
native button already does after AppKit's own tracking loop takes its up
(`NativeButtonsMac.swift`). Test: `ContextMenuMacTests
testTheHoldOfTheRightPressThatOpenedAMenuEndsAsItCloses`.

Checked with real input in the T3 Code clone (bundle 9ee4e4e88, flat sidebar):
without the fix, after a right-click and Escape the row showed no Snooze or
Settle on its next hovers until a left click; with it, the row's actions came
back on every hover after Escape and after a pick ('Mark unread').
