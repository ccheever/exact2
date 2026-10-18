# macOS: the host tells VoiceOver of dialogs and menus with notifications AppKit does not post

**Status:** Closed
**Resolution:** fixed: a page dialog is named after the focus in it at low priority, the focus is told once, AXMenuClosed on the menu as it leaves the window, the focus told after a menu only if its rows held it, an unfocused popover sends VoiceOver back to its trigger (AccessibilityClientMacTests); the D3 layout change stays
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1080.003, LLP 1115.000

Found by the T3 Code clone's real-input session of 2026-10-16 (session G, rows 5, 6, 10, 11, 12 and 14) with VoiceOver. Recorded with an `AXObserver` in another process, hand-built AppKit screens on macOS 26.6.2 (a sheet, a titled panel and an `NSPopover` whose field takes the focus, an `NSAlert` as a sheet and as a modal window, a sheet over a sheet, an `NSMenu`, a child window holding a list that takes the focus) never post `AXAnnouncementRequested` or `AXLayoutChanged`: each tells the focus, the focused element's `AXFocusedUIElementChanged` last, after `AXSheetCreated` or `AXWindowCreated` and `AXFocusedWindowChanged` for a window. An `NSMenu` posts `AXMenuOpened` and `AXMenuClosed` on the menu and no focus change.

The host differed in four ways:

1. A dialog drawn in the page was announced (`AXAnnouncementRequested`, high priority) before the focus in it was told. VoiceOver then said "Rename project, dialog" or "Interface font family, dialog" and never the focused field (rows 5, 11, 12), or only "Cancel, button" (row 14).
2. A menu the batch took out of the window said `AXMenuClosed` on the page's scroll view, so VoiceOver read "scroll area" (row 10).
3. Every menu that closed told the focus after it, also one whose rows never had the focus: opened by VO-Space, which moves no focus, and closed by Escape, VoiceOver went to the switch that held the keyboard focus (row 10).
4. Every landing posted `AXFocusedUIElementChanged` for an element AppKit had just posted it for, as the view became the first responder of the active application's key window.

Fix (host): a dialog drawn in the page is named after the focus in it is told, at low priority, as a polite live region is, so its words queue after the focused control's (this keeps the naming of `20261016-macos-voiceover-alertdialog-menu-group.md` and `20261016-macos-voiceover-input-dialog-menu.md` and moves it after the focus); one VoiceOver landed on itself is not named. The host tells the focus only when AppKit did not. A menu leaving the window says `AXMenuClosed` on itself as it goes; the focus is told after a menu closes only when one of its rows held it, and a painted popover closed with the focus where it was when it opened sends VoiceOver back to its trigger while the focus is still there. LLP 1080.003 D3's one `AXLayoutChanged` for a changed modal stays and names the same element as the focus.

Left, by decision: AppKit posts no `AXLayoutChanged` in any of these; dropping it changes LLP 1080.003 D3 ("a changed modal posts AppKit's layout-changed notification once"), which this fix does not do. AppKit names a sheet or an alert from its window's notifications, which a group in the page cannot post; the announcement stands in for them. A `dialog` opened with `showModal(id)` is that sheet on the Mac (LLP 1115.002 D7.2), and a `dialog role="alertdialog"` is `NSAlert` (LLP 1115.000 D7.6).

Not verified here: what VoiceOver speaks, and that it queues a low-priority announcement after the focused control's words rather than dropping it. That needs a person with VoiceOver (the next real-input session's rows 3, 5, 6, 8, 10, 11, 12, 13 and 14).
