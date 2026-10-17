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


## Ruling (Charlie, 2026-10-16)

Option 2, "for now": press inside the item's action, as AppKit does, for every native
menu that defers its pick today (`MenusMac.pick` and `ChooserMac.choose`).

- Validate the pick, then press, in the action; the once-token stays.
- A batch that removes the invoker or an ancestor while `nativeTracking > 0` leaves that
  view in the window, hidden and out of the accessibility tree with its subtree, until
  `popUp` returns, then removes it, as `leaving` does for a popover AppKit is still taking
  away (`PopoversMac.swift`). Removal by a children batch or a root replacement is the
  same case as destruction.
- `close()` neither schedules the landing on the trigger nor gives the focus back when
  the choice already moved the focus (`MenusMac.swift`, the landing at about 398 and
  `makeFirstResponder(previous)` at about 412). A sheet's field is in its own window, so
  the page window's first responder still looks unmoved, and without this VoiceOver is
  sent back to the old field whatever the press timing.
- Measure against a like presentation: the low-priority name above means the repro's
  dialog was drawn in the page, while the hand-built one was a sheet, which AppKit names
  itself. Repeat the six runs with a `<dialog>`, which this host presents as a sheet
  (LLP 1115.002 D7.2), and with a pick that navigates the invoker away; update
  `testAPickedItemPressesInTheTurnItsMenuClosed` to the new order. iOS is unchanged.
- Option 3 is refused: a louder name cuts off the field's words.
