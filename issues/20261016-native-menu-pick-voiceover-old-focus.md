# macOS: a native menu's item that opens a dialog still lets VoiceOver re-read the old focus first

**Status:** Open
**Systems:** Apple host (MenuHost, NSMenu), VoiceOver
**Severity:** P3
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** LLP 1021 (press timing), realinput-1017k H-9, G-5, FN6

## Problem

With a long draft focused in the page, right-click a row whose context menu
(`contextPopover`, an `NSMenu`) has an item that opens a dialog and moves the
focus into its field. VoiceOver says "Closing menu", then re-reads the old
focus ("Message, edit text, …") before the dialog's field, and a long read
can bury the dialog's name, which the host queues at low priority. A
hand-built AppKit window whose menu action begins a sheet with its field
focused goes straight to "In dialog …, Project title, edit text" (4 of 4,
VoiceOver on macOS 26, measured beside a Contract repro of the same shape).

## What is done

`MenuHost` presses the chosen row as `popUp` returns, in the turn the menu
closed, not on the next main-queue turn (`ContextMenuMacTests
testAPickedItemPressesInTheTurnItsMenuClosed`). With it the repro skipped the
old focus in 2 of 6 runs (0 of 3 before); the name was heard in 5 of 6.

## Cause

AppKit sends an item's action inside `popUp`, before it returns; the host
records the pick there and presses after `popUp` returns, because a press
whose batch unmounts the invoker must not run under AppKit's tracking of it
(LLP 1021, "The chooser"). VoiceOver's question for the focus, after the
menu's AXMenuClosed, is evidently still answered before the press in most
runs: inside `popUp`'s remaining run-loop work, or before it.

## Options

1. Press inside the item's action when the press cannot unmount the invoker
   (no navigation, the invoker not in what the batch removes): needs a way to
   know that before the batch runs, or to defer only the removal.
2. Hold the invoker (retain it and keep it in the window, hidden) until
   `popUp` returns, and press inside the action as AppKit does.
3. Accept it and post the dialog's name at a priority that survives a long
   read (medium), at the cost of cutting off the field's words.

