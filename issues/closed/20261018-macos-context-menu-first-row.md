# macOS: a right press held a moment chooses the context menu's first row

**Status:** Closed
**Resolution:** A pointer's context menu pops up with NSMenu.popUpContextMenu(_:with:for:) and the press that asked for it (MenuHost.track), as a hand-built view's menu(for:) does: its top left at the pointer, the pointer 1 pt outside it; a request from the keyboard keeps popUp at the node's middle. Real right presses held 300, 600 and 1000 ms on a node and on a list row choose nothing, as in AppKit.
**Systems:** Apple host (MenuHost, NSMenu), context menus
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-18
**Related:** LLP 1021 §5.1, LLP 1115

## Problem

A right press on a node whose `contextPopover` is a native `NSMenu`, released half a second or more later without moving, chooses the menu's first row; a quicker release leaves the menu open with that row highlighted. In the T3 Code clone this pinned a thread and started a rename that nobody chose. A hand-built AppKit view (`menu(for:)`, or `NSMenu.popUpContextMenu(_:with:for:)`) puts the menu's top left at the pointer with the pointer 1 pt outside it: nothing is highlighted, a release 300 ms after the press leaves the menu open, and one after 600 or 1000 ms closes it; neither chooses anything (macOS 26.6.2, real right presses).

## Cause

On main before the fix, `MenuHost.context` (host/apple/Sources/ExactKit/Mac/MenusMac.swift:371) tracked the menu with `popUp(positioning: nil, at:in:)` (MenusMac.swift:86), a pop-up button's presentation: the menu's left edge is at the pointer, and the pointer is 5 pt below its top, inside the first row. Held past AppKit's click threshold, the release chose the row under it.

## Reproduction

A row with `contextPopover` naming a three-item menu; a real right press on it, held 600 ms, released in place. The first item's `press` runs.

## Fix

Pop a pointer's context menu up with `NSMenu.popUpContextMenu(_:with:for:)` and the press that asked for it, as AppKit's own context menus are. A request from the keyboard (the ContextMenu key, VoiceOver's show menu) has no press and keeps `popUp` at the node's middle.

## Acceptance

Held 300, 600 and 1000 ms, a right press on a node and on a `list` row chooses nothing, as AppKit's: the menu opens with no row highlighted, stays open after the 300 ms release and closes after the others; a ContextMenuMacTests case finds the pointer outside the menu at its top left, and a key's menu still at the node's middle.
