# A pill whose label changes is a new element for VoiceOver

**Status:** Closed
**Resolution:** fixed: an atom's element is kept while its range and role stay and reads its new label and title; the agent's scoped `tree --ax` lists the pills
**Systems:** host/apple
**Author:** Daehyeon Mun
**Date:** 2026-10-30

**Problem.** The T3 Code clone's image pill in the prompt follows its upload's percentage in its face's
`label`. Each new label replaced the pill's accessibility element on macOS and iOS, so a VoiceOver cursor
on an uploading pill was lost at every report. The reference keeps one element (`ImageChipButton`).

**Cause.** `TextArea.atomsChanged` kept an atom's element only while its range, label and role stayed
the same (`host/apple/Sources/ExactKit/FieldAtoms.swift:71`); a new label made a new `AtomElement` and
posted `layoutChanged` (macOS), or replaced the owner's `accessibilityElements` (iOS). The Mac's pills
are in the text view's `AXChildrenInNavigationOrder` (AppKit derives it from `accessibilityChildren`), but
`agent.mjs macos tree --ax <field>` left them out: its scope kept only elements a node owns
(`AgentAccessibility.swift:193`), and a Mac pill's element was owned by none (its parent is the text
view, not the node; on iOS its container is the node).

**Reproduction.** `InlineAtomEditorMacTests.testANewFaceAloneDrawsThePillAgain`: a face from 0% to 87%.

**Fix.** An element is kept while its range and role stay; its label and title are read anew, and macOS
posts `titleChanged` for it. iOS sets the kept element's `accessibilityLabel`. The agent joins a Mac
pill's element to the field's node, as on iOS, and a scoped `tree --ax` also keeps an element no node
owns when it is under a kept one.

**Acceptance.** The Mac and iOS tests above keep the element and read "A quote 87%";
`testAPressSelectsTheAtomAndAnAccessibilityElementNamesIt` finds the pill in the navigation order and in
a scoped tree, joined to the field's node.
