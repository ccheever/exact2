# On macOS 27, Korean 2-Set composes in a field without marked text

**Status:** Open
**Systems:** the macOS host's key path during an input method's composition (`host/apple/Sources/ExactKit/KeyEvents.swift` `passesComposition`, `composes`; LLP 1005, "A chord during an input method's composition"), the field editor (`Mac/TextAreaMac.swift` `FieldEditor`)
**Severity:** P2
**Author:** Claude (Opus 5.5), measuring `KeyboardEvent.isComposing`
**Date:** 2026-10-25

## Symptom

macOS 27.2 (26B118), a development build of a fixture whose field logs each key
and its `isComposing` and whose Enter sends when not composing. Korean 2-Set,
the keys `g k s Return` posted at the HID event tap:

- KFix (macOS host): `d:ㅎ:false;d:ㅏ:false;d:ㄴ:false;d:Enter:false`, and the
  Enter sends. Every key reaches the `key` handlers.
- Chrome 156 on the same Mac, same keys, the web host: the input method marks
  the syllable (`compositionstart`, `input` with `isComposing`), the Enter that
  ends it arrives with `isComposing` true, and only a second Enter sends.

The guide and LLP 1005 say the macOS host keeps a composition's keys from
handlers. That holds only when the field has marked text.

## Cause

Logging the field editor shows the input method calling `insertText` with
`ㅎ`, `하`, `한` and again `한` on Return, each replacing the last, and never
`setMarkedText`. `hasMarkedText()` is false at every key, so
`passesComposition` passes them all and `composes` reports none composing.
The same input method gives Chrome's text input client marked text, so its
choice depends on the client. LLP 1005's composition rule (2026-10-07, on
macOS 26) was written against marked text in this field editor; whether macOS
27 or a change to the field editor ended it is not known.

## Fix

Find what the input method reads from the client before it marks text (the
attributes `validAttributesForMarkedText` returns, `markedRange`, the field
editor's `NSTextInputContext`) and make the field editor answer as an
`NSTextView` that composes Korean with marked text, such as TextEdit's. Check
with a person typing Korean 2-Set, then with `KeyUpMacTests`.
