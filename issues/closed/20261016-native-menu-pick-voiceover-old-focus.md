# macOS: a context-menu pick that removes the focused field lets VoiceOver read that field first

**Status:** Closed
**Resolution:** not reproduced on main: the navigate-away pick re-read the removed field in 0 of 23 runs with real VoiceOver; #719 not merged, as it made the `<dialog>` case worse (3 of 18, against 0 of 16 on main)
**Systems:** Apple host (MenuHost, NSMenu), VoiceOver
**Severity:** P3
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** LLP 1021 (press timing), #681, #706 (option 2, not merged), #712, realinput-1017k H-9, G-5, FN6

## Problem

With a field focused in the page, right-click a row whose context menu
(`contextPopover`, an `NSMenu`) has an item whose batch removes the invoker or
the focused field, such as Discard Changes navigating back from a compose
screen. VoiceOver says "Closing menu", then reads the removed field ("Compose
message, text", or "no action available, text") before the screen that
replaced it ("Projects, window, toolbar"): 3 of 6 runs on main.

## Measured

Real VoiceOver on macOS 26.6.2, read from its caption panel. A Contract repro:
a long draft focused in a text area, and a row whose `contextPopover` item is
right-clicked open and clicked with the pointer. Six runs per build per case,
the builds interleaved. The count is runs that read the old or removed field
before what the pick opened.

| Pick | main | #706 |
|---|---|---|
| Opens a `<dialog>` (a sheet on this host) with its field focused | 0 of 6 | 2 of 6 |
| Navigates the invoker away (Discard Changes, the compose draft focused) | 3 of 6 | 1 of 6 |
| Opens an `aria-modal` box drawn in the page | 0 of 6 | 0 of 6 |

The T3 Code clone, measured the same way:

- Its Rename project re-read was the clone's own doing. It pops its own
  `NSMenu` from its module (`T3Sidebar.swift`), not through `MenuHost`, and
  its right press moved no focus, so the composer kept the focus through the
  menu. With the right-pressed row focused, as the reference app does,
  VoiceOver re-read nothing in 3 of 3 runs. #706 changed nothing here.
- Archive chosen by type-select and Return did nothing: the host read
  `NSEvent.subtype` on a key event. Fixed on main by #712.

## Cause

Since #681, `MenuHost` records a pick in the item's action and presses it as
`popUp` returns, in the turn the menu closed, so that a press whose batch
unmounts the invoker never runs while AppKit tracks the menu in it (LLP 1021,
"The chooser"). VoiceOver asks for the focus after the menu's AXMenuClosed; in
half the runs it asks before the press, while the field the batch will remove
still has the focus.

## What to change

The ruling's option 2 was built as #706 and not merged: the press inside the
item's action, a removed invoker held hidden in the window until `popUp`
returns, and `close()` leaving a focus the choice moved. It cut the
navigate-away case to 1 of 6 but made the `<dialog>` case worse (0 to 2 of 6).
Likely cause: the press now begins the sheet inside `popUp`, and `beginSheet`
returns only once the sheet is in, running the run loop meanwhile, so the
menu's close reaches VoiceOver while the sheet is arriving.

The likely next step, from #706's branch:

1. Option 2 as built, with a `<dialog>`'s sheet begun after `popUp` returns.
2. Hold a list row too. On the Mac a `list` is an `NSTableView`, and
   `TableFeedMac` takes a removed row out of the window by its own paths
   (`adopt`, `sendHome`), which bypass the hold, so a row deleted from its own
   context menu still leaves the window during tracking.
3. Hold a window toolbar's menu item (`InvokerMenu`), which with option 2
   presses inside AppKit's action with no hold: a press that changes the bar
   can remove the item that is sending the action.

## Acceptance

- On the repro above, six runs each: the navigate-away pick reads the removed
  field in none; the `<dialog>` and page-drawn `aria-modal` cases stay at none.
- XCTests beside `ContextMenuMacTests`: a list row deleted from its own context
  menu, and a toolbar menu pick that removes its item, stay in the window,
  hidden, until `popUp` returns.

## Options (as filed)

1. Press inside the item's action only when the press cannot unmount the
   invoker.
2. Hold the invoker (retain it and keep it in the window, hidden) until
   `popUp` returns, and press inside the action as AppKit does.
3. Post the dialog's name, which the host queues at low priority, at medium
   priority, at the cost of cutting off the field's words.

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

## Closed, 2026-10-18

Not reproduced on main. The repro was rebuilt from the description above and measured with
real VoiceOver as before (#719's body has it and every run's captions); the count is runs
that read the old field before what the pick opened. #719 (option 2 as ruled, with the
three next steps above) was not merged: it made the `<dialog>` case worse.

| Pick | main | #719 |
|---|---|---|
| Navigates the invoker away (Discard Changes) | 0 of 23 | 0 of 22 |
| Opens a `<dialog>` (a sheet) with its field focused | 0 of 16 | 3 of 18 |
| Opens an `alertdialog` (`NSAlert`) | 2 of 18 | 0 of 11 (rounds 2 and 3) |
| Opens a page-drawn `aria-modal` box | 0 of 18 | 0 of 18 |
| Opens that box with its field focused | 6 of 6 | 6 of 6 |

The alert case is issues/20261018-macos-menu-pick-alert-voiceover-old-field.md. The box
whose field takes the focus is not the menu's timing (it re-reads on both builds) and is
issues/20261018-macos-menu-pick-page-modal-field-voiceover.md.
