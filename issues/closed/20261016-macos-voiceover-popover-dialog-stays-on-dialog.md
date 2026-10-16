# macOS: VO-Space on a trigger whose dialog takes the focus leaves VoiceOver on the dialog, not the focused field

**Status:** Closed
**Resolution:** fixed: the landing's AXLayoutChanged is posted on the modal when it names the focus in it, and a dialog in a popover drawn in the page is named 0.5 s after the landing (AccessibilityClientMacTests)
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1080.003, LLP 1115.002, issues/closed/20261016-macos-voiceover-landing-before-focus.md

Found by the T3 Code clone's real-input session of 2026-10-16 (session I, row H-10): VO-Space on "Interface font family" opens the font picker, a `popover="auto"` box with `role="dialog" aria-modal=true` drawn in the page, and its search field takes the keyboard focus. VoiceOver said "Interface font family, dialog", VO-F3 put its cursor on the dialog, and "Search fonts, edit text" was never spoken.

Reproduced with VoiceOver on a test app with the same markup. VO-Space on an alertdialog's trigger whose commit focuses Cancel did the same ("Remove this project?, dialog", cursor on the dialog). Opened with VoiceOver's cursor elsewhere (a pointer's press), both read the focus.

Cause: the host's one `AXLayoutChanged` for a changed modal (LLP 1080.003 D3) named the focused field but was posted on the page's scroll view. With its cursor on a trigger that the modal had just hidden, VoiceOver went to the scroll view's child holding that field, the dialog, and stayed there. AppKit's own `NSPopover` and a child window holding a focused search field post only the focus change (recorded with an `AXObserver`); VoiceOver reads them as "Search fonts, search text field", then "In …, popover". Posting the layout change on the modal, posting none, or posting it 0.4 s later each made VoiceOver read the field. Posting it on the scroll view after the focus change, or with no element named, did not.

A dialog in a popover drawn in the page was also never named: the host skipped every dialog in a popover, though only AppKit's `NSPopover` is named by VoiceOver ("In <name>, dialog, 3 items"). VoiceOver dropped a name announced in the same turn as the landing on a text field after VO-Space; one announced 0.3 s later was read after the field.
