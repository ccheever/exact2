# macOS: a modal's VoiceOver landing is chosen before the opening commit's focus() runs

**Status:** Closed
**Resolution:** fixed: VoiceOver's landing is told on the next turn, after the commit's commands, and names the element focused then (AccessibilityClientMacTests)
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1080.003, LLP 1115.002

Found by the T3 Code clone's real-input session of 2026-10-16 (session G, rows 3 and 14) with VoiceOver, and reproduced here by an `AXObserver` in another process on a test app whose actions open the dialog and call `focus()` in the same commit.

An action that opens a modal and calls `focus()` on a control in it (an alertdialog's Cancel) applies its batch first, and the session delivers `focus()` after it. The accessibility pass that sent VoiceOver to the modal ran inside the batch, so it saw no focus in the modal and named the modal's group in its `AXLayoutChanged`; the button took the focus afterwards. VoiceOver stayed on the group (row 14, one of four openings) or read only the button. The same order sent VoiceOver to the page behind a sheet after Escape closed a dialog over the sheet (row 3), and to "scroll area" when a commit took a menu away and its `focus()` gave the focus back to the trigger (row 10): the pass then named no element and posted on the page's scroll view.

Fix (host): the pass schedules the landing on the next turn of the main queue, after the commit's commands; the landing names the element focused then.

Tests: `AccessibilityClientMacTests` (an alertdialog whose commit focuses Cancel; a dialog closed over another modal with `focus()` giving the focus back; a menu taken away by a batch whose `focus()` gives the trigger the focus). Not verified here: what VoiceOver speaks.
