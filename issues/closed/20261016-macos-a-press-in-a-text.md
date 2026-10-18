# macOS: a press in a text view does not light-dismiss an open popover

**Status:** Closed
**Resolution:** fixed by MenuHost completing a press whose release a view's tracking loop took (NSApp.currentEvent after the loop), PopoverLightDismissMacTests
**Systems:** host/apple macOS, menus, pointer events
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-16

## Problem

On macOS an open `popover="auto"` stays open after a press in a text view outside it: an
`input` or `textarea` (its NSTextField's field editor), a search field (NSSearchField), or an
app module's NSTextView. The T3 Code clone's real-input session (2026-10-16, row 4): the
Project scripts menu stayed open after a click in the composer and after one in the sidebar's
Search threads; Settings' Environment scope menu stayed open after a click in Search settings.
A click on empty page space closed it. On the web the same press closes the popover.

## Cause

The host light-dismisses as HTML's "popover light dismiss" does
(https://html.spec.whatwg.org/multipage/popover.html#popover-light-dismiss): it records the
topmost clicked popover at the pointer-down and closes the popovers above it at the
pointer-up, when the up is over the same one. It hears both through the application's local
event monitor (`ExactViewMac.swift`, `MenuHost.pointer` in `MenusMac.swift`). NSTextView's
`mouseDown` (a field editor is one) and NSControl's tracking run their own loop that takes
the mouse-up from the queue with `nextEvent(matching:)`; a local monitor sees only what
`NSApplication.sendEvent` dispatches, so the host never hears the release and the press never
completes. The agent's `tap` passes because it hands the release to `MenuHost.pointer` itself.

## Fix

At the pointer-down `MenuHost` schedules a check for when the run loop is back in its default
mode. A tracking loop runs in the event-tracking mode, so the check runs after it ends, and
before the application takes its next event (measured). If the application's current event
is then a release of that button in that window, no older than the press, a loop took it, and
the host completes the press with it, at its point. HTML's timing stays: the popover closes at
the release, and a press dragged onto the popover keeps it open.

AppKit's own transient popover (`NSPopover.Behavior.transient`) and its menus close at the
mouse-down outside instead. Changing that timing would depart from LLP 1021's "light dismiss
… per the spec" and is not part of this fix.

Tests: `PopoverLightDismissMacTests` (a plain NSTextView and an NSSearchField outside a popover,
a field inside it, and a press dragged from the text view onto the popover). The two outside
cases fail before the fix.
