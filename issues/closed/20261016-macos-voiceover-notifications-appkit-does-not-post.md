# macOS: the host tells VoiceOver of dialogs and menus with notifications AppKit does not post

**Status:** Closed
**Resolution:** fixed: no dialog announcement, AXMenuClosed on the menu as it leaves the window, the focus told after a menu only if its rows held it, an unfocused popover sends VoiceOver back to its trigger (AccessibilityClientMacTests); the D3 layout change stays
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1080.003, LLP 1115.000

Found by the T3 Code clone's real-input session of 2026-10-16 (session G, rows 5, 6, 10, 11, 12 and 14) with VoiceOver. Recorded with an `AXObserver` in another process, hand-built AppKit screens on macOS 26.6.2 (a sheet, a titled panel and an `NSPopover` whose field takes the focus, an `NSAlert` as a sheet and as a modal window, a sheet over a sheet, an `NSMenu`, a child window holding a list that takes the focus) never post `AXAnnouncementRequested` or `AXLayoutChanged`: each tells the focus, the focused element's `AXFocusedUIElementChanged` last, after `AXSheetCreated` or `AXWindowCreated` and `AXFocusedWindowChanged` for a window. An `NSMenu` posts `AXMenuOpened` and `AXMenuClosed` on the menu and no focus change.

The host differed in three ways:

1. A dialog drawn in the page was announced (`AXAnnouncementRequested`, high priority) as it opened. VoiceOver then said "Rename project, dialog" or "Interface font family, dialog" and never the focused field (rows 5, 11, 12), or only "Cancel, button" (row 14).
2. A menu the batch took out of the window said `AXMenuClosed` on the page's scroll view, so VoiceOver read "scroll area" (row 10).
3. Every menu that closed told the focus after it, also one whose rows never had the focus: opened by VO-Space, which moves no focus, and closed by Escape, VoiceOver went to the switch that held the keyboard focus (row 10).

Fix (host): no announcement; a menu leaving the window says `AXMenuClosed` on itself as it goes; the focus is told after a menu closes only when one of its rows held it, and a painted popover closed with the focus where it was when it opened sends VoiceOver back to its trigger. LLP 1080.003 D3's one `AXLayoutChanged` for a changed modal stays and names the same element as the focus change.

Left, by decision: AppKit posts no `AXLayoutChanged` in any of these; dropping it changes LLP 1080.003 D3 ("a changed modal posts AppKit's layout-changed notification once"), which this fix does not do.

Consequence: a dialog drawn in the page is no longer named as it opens; VoiceOver reads the focused element and, as its hint, that it is in a dialog. AppKit names a sheet or an alert from its window's notifications, which a group in the page cannot post: a `dialog` opened with `showModal(id)` is that sheet on the Mac (LLP 1115.002 D7.2), and a `dialog role="alertdialog"` is `NSAlert` (LLP 1115.000 D7.6).

Not verified here: what VoiceOver speaks. That needs a person with VoiceOver (the next real-input session's rows 3, 5, 6, 10, 11, 12 and 14).
