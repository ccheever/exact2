# A pill whose label changes is a new element for VoiceOver

**Status:** Closed
**Resolution:** fixed: an atom's element is kept while its range and role stay, and reads its new label and title
**Systems:** host/apple
**Author:** Daehyeon Mun
**Date:** 2026-10-30

**Problem.** The T3 Code clone's image pill in the prompt follows its upload's percentage in its face's
`label`. Each new label replaced the pill's accessibility element on macOS and iOS, so a VoiceOver cursor
on an uploading pill was lost at every report. The reference keeps one element (`ImageChipButton`).

**Cause.** `TextArea.atomsChanged` kept an atom's element only while its range, label and role stayed
the same (`host/apple/Sources/ExactKit/FieldAtoms.swift:71`); a new label made a new `AtomElement` and
posted `layoutChanged` (macOS), or replaced the owner's `accessibilityElements` (iOS).

**Reproduction.** `InlineAtomEditorMacTests.testANewFaceAloneDrawsThePillAgain`: a face from 0% to 87%.

**Fix.** An element is kept while its range and role stay; its label and title are read anew, and macOS
posts `titleChanged` for it. iOS sets the kept element's `accessibilityLabel`.

**Acceptance.** The Mac and iOS tests above keep the element and read "A quote 87%". The Mac's text view
serves the pills in `AXChildrenInNavigationOrder` (AppKit derives it from `accessibilityChildren`).
