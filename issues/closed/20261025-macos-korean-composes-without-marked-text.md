# On macOS 27.2, Korean 2-Set composes in an AppKit field without marked text

**Status:** Closed
**Resolution:** the platform's behaviour: a stock `NSTextField` gets the same, so the macOS host keeps it, and Chrome differs (LLP 1005, `KeyboardEvent.isComposing`)
**Systems:** host/apple (`KeyEvents.swift` `passesComposition`, `composes`; `Mac/TextAreaMac.swift` `FieldEditor`)
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-25

## Problem

macOS 27.2 (26B118), Korean 2-Set, the keys `g k s Return` posted at the HID
event tap. A development build of an Exact app whose field logs each key and
its `isComposing`, and whose Enter sends when not composing, heard
`d:ㅎ:false;d:ㅏ:false;d:ㄴ:false;d:Enter:false`, and the Enter sent. Chrome 156
on the same Mac, with the same keys, marks the syllable (`compositionstart`,
`input` with `isComposing`) and reports the Enter that ends it as composing.
LLP 1005's rule that the macOS host keeps a composition's keys from handlers
holds only while the field has marked text.

## Cause

The input method inserts each syllable in place of the last (`insertText`
with `ㅎ`, `하`, `한`, then `한` again on Return) and never calls
`setMarkedText`, so `hasMarkedText()` is false at every key. Two controls,
same keys and source:

- A bare AppKit app with one stock `NSTextField` and the window's own field
  editor: `hasMarkedText()` false and `markedRange()` empty at each key and
  80 ms after it, while the text goes `ㅎ`, `하`, `한`.
- The Exact app with no `key` handler (only `input`): the same `insertText`
  calls and no `setMarkedText`.

So neither the `key` handler running before AppKit nor a re-render between
keys causes it: AppKit text views get Korean without marked text here, and
Chrome's text input client does not.

## Resolution

No change. A hand-built AppKit app's field acts on that Return, as the Exact
host does (LLP 1115: the platform's answer wins), and `isComposing` reports
what AppKit says. A web build of the same app keeps Chrome's behaviour.
