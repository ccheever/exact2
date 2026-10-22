# A plan batch while a menu with removed items is open crashes the Mac app

**Status:** Closed
**Resolution:** fixed: a removal writes an item's hidden state only when it changes, and while its menu is open it writes on the run loop's next turn
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
`allowsKeyEquivalentWhenHidden` only where they change. A change while the menu (or the bar
holding it) is tracking is written on the run loop's next turn, outside the layout, followed by
the host's sync.

## Acceptance

`MenuBarMacTests` opens a menu whose item view runs a sync inside the layout: an unchanged removal
writes nothing, and a withdrawn one shows its item on the next turn while the menu is open.
Caltrain with Help's item removed survives moving from Window to Help with real input.
