# macOS: VoiceOver re-reads the old field when a context-menu pick focuses a field in a page-drawn modal

**Status:** Open
**Systems:** Apple host (MenuHost, accessibility), VoiceOver
**Severity:** P3
**Author:** Daehyeon Mun
**Date:** 2026-10-18
**Related:** issues/20261018-macos-menu-pick-alert-voiceover-old-field.md, issues/closed/20261016-native-menu-pick-voiceover-old-focus.md, issues/closed/20261016-macos-voiceover-notifications-appkit-does-not-post.md, #719 (not merged)

With a long draft focused in a `textarea`, a `contextPopover` (`NSMenu`) pick whose action
opens a `column role="dialog" aria-modal` drawn in the page and calls `focus()` on its
`input` moves the keyboard focus into the box, but VoiceOver says "Closing menu Compose
message, edit text …", the draft, and does not follow the focus: 6 of 6 runs on main and
on #719, real VoiceOver on macOS 26.6.2. Opened with the focus left on the draft, the box is
read as "Box, dialog" (0 of 18 re-read); a `<dialog>` sheet whose field takes the focus is
read (0 of 16). #719's body has the repro (not committed) and each run's captions.

Cause: not found. Record with an `AXObserver` what the host posts after this pick and what
AppKit posts when an `NSMenu` item shows a group in its window and makes the group's field
the first responder; post what AppKit posts.

Acceptance: on the repro, VoiceOver reads the box's field, not the draft, in 6 of 6; the
`<dialog>`, unfocused-box and `alertdialog` picks read the draft no more than today.
