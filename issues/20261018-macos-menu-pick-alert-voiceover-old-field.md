# macOS: an alert opened by a context-menu pick lets VoiceOver re-read the old field first

**Status:** Open
**Systems:** Apple host (MenuHost, NSAlert), VoiceOver
**Severity:** P3
**Author:** Daehyeon Mun
**Date:** 2026-10-18
**Related:** LLP 1021 (press timing), issues/closed/20261016-native-menu-pick-voiceover-old-focus.md, #719 (not merged)

With a long draft focused in a `textarea`, a `contextPopover` (`NSMenu`) pick that opens a
`dialog role="alertdialog"` (an `NSAlert` sheet) lets VoiceOver say "Closing menu Compose
message, edit text …", the draft, before "In alert, dialog": 2 of 18 runs on main, real
VoiceOver on macOS 26.6.2. #719's body has the repro (not committed) and each run's captions.

Likely fix: begin the `NSAlert` sheet and tell VoiceOver's landing once `popUp` returns, as
#719 tried on `daehyeon/fix-menu-pick-navigate-away` (`MenuHost.afterPopUp`; there
AlertsMac.swift:202, Accessibility.swift:689): 0 of 11 in its rounds 2 and 3. That branch
also pressed inside the item's action, where main presses as `popUp` returns, and made the
`<dialog>` case worse (3 of 18, against 0 of 16 on main), which this must not carry.

Acceptance: on the repro, 0 of 6, with the `<dialog>` and `aria-modal` picks staying at 0.

A page-drawn box whose field takes the focus re-reads 6 of 6 on main:
issues/20261018-macos-menu-pick-page-modal-field-voiceover.md.
