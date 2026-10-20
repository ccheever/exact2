# macOS: VoiceOver is not told an alertdialog's title, a menu's roles, or a group's role description

**Status:** Closed
**Resolution:** fixed: alertdialog announced on open, focus-holding modal lands VoiceOver on the focus, menu roles are AXMenu/AXMenuItem with AXMenuOpened, role description served out of process (AccessibilityClientMacTests)
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1115.000, LLP 1080.003

Found by the T3 Code clone's real-input session of 2026-10-16 (rows 5, 10, 13 and 15) with VoiceOver, and confirmed here by reading what another accessibility client is served and told (`AXUIElementCopyAttributeValue`, an `AXObserver`):

1. An `alertdialog` that opens and moves the focus to its Cancel button is read as "Cancel, button" only. VoiceOver reads an `NSAlert`'s title and message as it appears (AppKit posts `AXSheetCreated` for its sheet). The host posted only `AXLayoutChanged` pointing at the dialog, which the focus change a moment later interrupts.
2. A `role="menu"` box was no element (`AXUnknown` to VoiceOver) and its `role="menuitem"` rows were `AXButton`s, so VoiceOver said neither "menu" nor "menu item". WebKit serves them as `AXMenu` and `AXMenuItem` (Core-AAM) and posts `AXMenuOpened` as a menu shows, as AppKit's menu does.
3. A labelled `role="group"` was served no `AXRoleDescription` to another process, so VoiceOver omitted "group". Cause: `NodeView.accessibilityRoleDescription()` called the `accessibilitySubrole()` getter while serving the request; in-process reads answered "group", another process got nothing (a menu likewise).
4. A `role="dialog"` popover whose search field took the focus as it opened (`autofocus`) was read as "Interface font family, dialog", not the field. `AXLayoutChanged` sent VoiceOver to the dialog after the field had the focus. AppKit's `NSPopover` posts only `AXCreated` for the popover and `AXFocusedUIElementChanged` for its first responder (measured), so VoiceOver reads the field.

Not the host's (row C21): the split button's "Git action options" part reports the whole row's frame because the clone authors that button `width="100%"` under the primary half, so its popover hangs from the row's left edge. A browser reports the same box.

Fix (host, `Accessibility.swift`): an `alertdialog` that becomes the modal outside a sheet is announced as an `alert` is, assertively, with its name, "dialog" and its description; a modal that already holds the focus sends VoiceOver to the focus, not to itself; `menu` and `menubar` are `AXMenu` and `AXMenuBar`, the menu item roles `AXMenuItem` (a checked one with `AXMenuItemMarkChar` "✓"), and a menu posts `AXMenuOpened` as it comes into view; the role description reads the host's own subrole. Tests: `AccessibilityClientMacTests`, which read through the AX API from a thread of their own and observe the notifications.

Not verified here: what VoiceOver speaks. The speech needs a person with VoiceOver on (the next real-input session's rows 5, 10, 13 and 15).
