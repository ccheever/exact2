# macOS: an input is AXUnknown, a page-drawn modal dialog is not announced, and VoiceOver loses a painted menu's focus

**Status:** Closed
**Resolution:** fixed: an input's role is its cell's (AXTextField), a page-drawn modal dialog is announced as it opens, a painted menu tells its focused row as its selection and says it closed (AccessibilityClientMacTests)
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1115.000, LLP 1080.003

Found by the T3 Code clone's real-input session of 2026-10-16 (session E, rows 9, 10, C6 and C25) with VoiceOver, and confirmed here by reading what another accessibility client is served and told (`AXUIElementCopyAttributeValue` from a thread of its own, an `AXObserver`):

1. Every plain `input` was `AXUnknown` to another process, so VoiceOver read "Search fonts", the keybinding recorder and "Project title" with no "edit text". A `type="search"` input was `AXTextField`. Cause: `Field` (TextAreaMac.swift) overrides `accessibilityRole()` for the number stepper and returned `super`'s otherwise, which is `NSView`'s `AXUnknown`; an `NSTextField`'s role is its cell's, and the override is what AppKit serves out of process. Not caused by the popover, `keycapture` or the placeholder.
2. A `role="dialog" aria-modal` drawn in the page that moves the focus into its first field as it opens was never announced ("Add Environment, dialog", "Rename project"). VoiceOver reads a sheet's or a panel's title as it appears; #647 announced only `alertdialog`.
3. A painted `role="menu"`: after ↓ the focus was on the `AXMenuItem`, but VoiceOver never read it, and after Escape it read an element behind the page's modal instead of the invoker. Measured with an `NSMenu` and the same observer: AppKit's menu posts `AXMenuOpened`, then `AXSelectedChildrenChanged` on the menu as the highlight moves (its `AXSelectedChildren` the highlighted item, which is `AXSelected`), then `AXMenuClosed`. The host posted only `AXMenuOpened`.

Fix (host): `Field`'s role is its cell's unless it is a number's; a modal `dialog` not in a sheet and not in a popover is announced as an `alertdialog` is (an `NSPopover` says nothing of itself, so a dialog in one is still not); a menu serves its focused row as its selected child and the row as selected, says `AXSelectedChildrenChanged` as a row takes the focus, says `AXMenuClosed` as it goes, and the focus it gave back is posted again after that. Tests: `AccessibilityClientMacTests`.

Not verified here: what VoiceOver speaks. That needs a person with VoiceOver (the next real-input session's rows 9, 10, C6 and C25).
