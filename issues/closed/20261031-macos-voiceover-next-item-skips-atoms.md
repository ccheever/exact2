# macOS: VoiceOver's next item skips a Markdown field's atoms, and VO-Space presses none

**Status:** Closed
**Resolution:** the atom elements follow the field among its host's elements, past every wrapper that holds the field alone (LLP 1045.000 D8, §5.2; §10 Q10 reversed for macOS)
**Systems:** host/apple (macOS)
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-31

## Problem

In the T3 Code clone, VO-Right from the prompt went to the next control and skipped every
chip in it, and VO-Space on a chip opened nothing, though a click on the chip and VO-Space on
other buttons worked. In the reference (Chromium, `contenteditable`) VO-Right reaches the
chips in text order and VO-Space opens them.

## Cause

Each atom's `AtomElement` was a child of the `NSTextView`
(`host/apple/Sources/ExactKit/Mac/TextAreaMac.swift`, `accessibilityChildren`), as §10 Q10
ruled on 2026-10-25. VoiceOver's next-item command passes over a text view's own children.
When VoiceOver interacts with the text view it reads the chips' words as text, so its cursor
never rests on an atom's element, and VO-Space has nothing to press.

## Fix

The field's host lists the atom elements right after the child that holds the field
(`NodeView.atomHost`, `withAtoms`, `FieldAtoms.swift`). The host is the nearest ancestor that
holds more than the field, or that is an element itself, because VoiceOver passes through a
scroll area of one item, but enters one of several only when the person interacts with it.
A scrolling host's scroll area lists them beside the field, before its next sibling. An
atom's parent is the element that lists it, and the text view's hit test at a pill answers the
atom's element, as VoiceOver's cursor following the mouse asks. iOS is unchanged.

## Acceptance

`InlineAtomEditorMacTests` (`host/apple/build.mjs --test`):
`testTheAtomsFollowTheFieldInTextOrderPastItsWrappers`,
`testTwoFieldsUnderOneHostAndAMovedField`,
`testInAScrollingHostTheAtomsFollowTheFieldBeforeItsNextSibling`,
`testTheWindowsHitTestAtAPillIsItsElement` and
`testAPressSelectsTheAtomAndAnAccessibilityElementNamesIt`. In the T3 Code clone with
VoiceOver, VO-Right from the prompt reads each chip in text order and then the next control,
and VO-Space on a chip opens it.
