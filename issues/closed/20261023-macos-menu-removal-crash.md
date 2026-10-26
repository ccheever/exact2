# A plan batch while a menu with removed items is open crashes the Mac app

**Status:** Closed
**Resolution:** fixed: a removal writes an item's hidden state only when it changes, a batch's change while its menu is open is written on the run loop's next turn, and the host's sync sets a removed item's state through the removal
**Systems:** host/apple
**Author:** Daehyeon Mun
**Date:** 2026-10-23

## Problem

With `-exact-menu-remove` naming an item in Help (or any menu whose layout reaches a batch), moving
from another menu to Help ends the app: AppKit throws from
`-[NSTableRowData insertRowsAtIndexes:withRowAnimation:]` ("Assertion failure … NSTableRowData.m")
and `+[NSApplication _crashOnException:]` traps. Seen in the T3 clone and in Caltrain with
`-exact-menu-remove="app-help"` on macOS 26.6.

## Cause

AppKit lays an open menu out as a table. While it adds Help's search field to that table, the field
becomes key, `PageFacts` posts the window facts, and the batch reaches `ShortcutHost.sync` inside
the layout. `ShortcutMenu.applyRemovals` (`host/apple/Sources/ExactKit/Mac/ShortcutsMac.swift`)
showed every removed item and hid it again on each call; showing one inserts a table row, which
`NSTableView` refuses during its own layout.

## Fix

`applyRemovals` computes each item's final state and writes `isHidden` and
`allowsKeyEquivalentWhenHidden` only where they change. A batch's change while the menu (or the
bar holding it) is tracking is written on the run loop's next turn, outside the layout; the
removal takes the item's chord out at once. The host's sync shows and hides its own items (Go ▸
Back, the Edit items a command stands in for) through the removal, recording what a removed
item goes back to, so it never writes a state the removal reverses.

## Acceptance

`MenuBarMacTests` opens a menu whose item view runs a sync inside the layout: an unchanged removal
writes nothing, and a withdrawn one shows its item on the next turn while the menu is open.
`BackMacTests` removes and restores Back while the bar tracks: Back is written once each way and
⌘[ leaves with the removal. Caltrain with Help's item removed survives moving from Window to Help
with real input, and a plan that changes the removal while Help is open shows and hides the item.
