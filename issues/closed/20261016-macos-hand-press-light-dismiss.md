# macOS: a hand's press in a text view still does not light-dismiss an open popover

**Status:** Closed
**Resolution:** fixed by MenuHost settling a tracked press from the window server's record of the release when AppKit has sent other events after it, PopoverLightDismissMacTests
**Systems:** host/apple macOS, menus, pointer events
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-16
**Related:** issues/closed/20261016-macos-a-press-in-a-text.md

## Problem

After the fix for `issues/closed/20261016-macos-a-press-in-a-text.md`, a click with a real
mouse in a field, a search field (NSSearchField), a `textarea` or an app module's NSTextView
outside an open painted `popover="auto"` still left it open. Its XCTests passed. The T3 Code
clone's real-input session (2026-10-16, rows H-6 and H-7): the first click in the composer
closed the Project scripts menu, and the next 11 did not; clicks in Search threads and Search
settings left their menus open. A small test app shows the same with a plain field, a
`textarea` and a search field. A click on a button or on empty page space closed the menu.

## Cause

`MenuHost.pointer` (`MenusMac.swift`) cannot see a release that a view's tracking loop takes.
At the press it schedules a check for the next turn of the run loop in the default mode, and
the check takes `NSApp.currentEvent` to be that release. Logging with real input
(`CGEvent.post(tap: .cghidEventTap)`) shows a different order. Once a field editor or a text
view has taken the press, AppKit queues the `mouseExited` and `mouseEntered` events of the
tracking areas it moved, and it sends them before the run loop turns. By the time the check
runs, the current event is a `mouseEntered`, so the press never completes. A click made in
the test leaves the queue empty after the release. That is why the release was still the
current event there, and why the tests passed while the app failed.

## Fix

The check no longer depends on the current event alone. If the current event is not the
release, it asks the window server when the button was last released
(`CGEventSource.secondsSinceLastEventType`, combined session state). If that release came
after the press, a tracking loop took it, and the host completes the press at the pointer's
position (`mouseLocationOutsideOfEventStream`). If the button is still held when the check
runs, it checks again 50 ms later. Events made in the application (the driver's and the
tests') never reach the window server's record, so the driver still passes its own release.

Tests: `PopoverLightDismissMacTests` clicks "by hand", in the order measured: the window
server records the release, and enter and exit events follow it in the queue. These tests
fail before the fix (presses in an NSTextView, an NSSearchField and a field, each repeated).
A press dragged onto the popover keeps it open. A press that is still held waits for its
release.

Real input, after the fix (2026-10-16). In the T3 Code clone, the Project scripts and Git
action options menus closed after 8 clicks in a row in the composer, and also after clicks in
Search threads, the terminal and a sidebar row. Settings' Environment and Project scope menus
closed after a click in Search settings. A press in the composer dragged onto the menu kept
it open and activated nothing. In a test app, a field, a `textarea`, a search field, a
WKWebView, a button and empty space each closed a painted popover. The clone's 'Open in'
menu is not a popover: it is a layer the app draws over the details card, with a backdrop
over that card only. The host does not light-dismiss it.
