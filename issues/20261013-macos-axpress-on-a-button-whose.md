# macOS: AXPress on a button whose press changes text answers kAXErrorAttributeUnsupported although the press lands

**Status:** Open
**Systems:** host/apple macOS, accessibility
**Severity:** P3
**Author:** Daehyeon Mun
**Date:** 2026-10-13
**Related:** issues/closed/20261011-macos-axpress-on-an-inline-text.md (X78); #529; LLP 1115

## Summary

When another process sends `AXPress` to an Exact `button` through `AXUIElementPerformAction` (the call VoiceOver's
VO-Space and Switch Control make), the press lands, but the call returns `kAXErrorAttributeUnsupported` (-25205)
instead of `kAXErrorSuccess` (0). This occurs when the press changes text on screen. A button whose press changes
nothing on screen answers 0. Both kinds of Exact button do this: the native button (`appearance="auto"`, AppKit's
`NSButton`, its element `NativeButtonCellMac`) and a `NodeView` button (`appearance="none"`,
`NodeView.accessibilityPerformPress` in `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift`). A client that checks the
result, such as an automation tool or an assistive app, takes a press that worked for a failed one, and may retry it.

The probable cause is the one #529 found for an inline run (#529 traced it with lldb for the run's press, not for a
button). AppKit's request handler does not use the action's `Bool`. It answers with the first accessibility error
recorded while the request is open. A button's press runs inside the request: `NodeView.accessibilityPerformPress`
calls `presenter.press` directly, and the native button's cell clicks its `NSButton`. The update that follows reads
values back from `NodeView`s (for the run: a paragraph's `accessibilitySubrole()`, `isAccessibilityRequired()`).
`NodeView` overrides the legacy `accessibilityAttributeValue`, so AppKit asks through the legacy API and records
"attribute unsupported" for each attribute the view does not have. The hand-built oracle below shows the same answer
from one such read.

## Why this arose

#529 fixed the inline run by running its press after AppKit's request returns. Its measurements showed the same answer
for the two buttons whose press changes text. Buttons are outside that issue's scope, and `NodeViewMac.swift` is also
edited by main PR #327.

## Reproduction

The repro app of #529's record (`app.contract` in the record), made with `bun scripts/exact.mjs new <dir>`:

```text
row display="flex" gap=12
  button press=buttoned testId="btn"
    text "Button"
  button appearance="none" press=buttoned testId="flat"
    text "Flat button"
  button press=hushed testId="quiet"
    text "Quiet button"
text `Buttons: ${buttons}` testId="bcount" font-size=24
```

`buttoned` increments `buttons`; `hushed` increments `quiet`, which nothing shows. Run `bun exact.mjs mac`, launch the
built binary directly, and from another process with the accessibility grant and the screen unlocked run
`axpress-oop <pid> <app log> btn flat quiet` (the record's probe).

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| `AXPress` on `btn` (native `NSButton`; the press changes `Buttons:`) | macOS 26.6.2 (25G83), Apple Silicon | main `0a36b5c40`, and #529 head `77a394bc9` | -25205; `Buttons: 0` → `Buttons: 1`; still the same element | 0 | record, BEFORE and AFTER |
| `AXPress` on `flat` (`NodeView` button; same press) | same | same | -25205; `Buttons: 1` → `Buttons: 2` | 0 | same |
| `AXPress` on `quiet` (its press changes nothing on screen) | same | same | 0 | — | same |
| Hand-built AppKit: an `NSButton` whose action sets an `NSTextField`'s string | macOS 26.6.2, `swiftc` | — | 0 | — | record, "button-oracle" |
| Hand-built AppKit: the same, and the action also reads `accessibilitySubrole()` from an `NSView` that overrides the legacy `accessibilityAttributeValue` | same | — | -25205, the press lands | — | same |

Record: https://raw.githubusercontent.com/ccheever/exact2/b0b9bf0528149298f87b29d669252bd2bd363a0a/fw/macos-axpress-inline-run/axpress-oop-record.txt
(the probe and the oracle sources `axpress-oop.swift.txt`, `urlcount.m.txt` and `button-oracle.swift.txt` are beside it).

Not run: VoiceOver itself; iOS.

## Constraints

- The press still lands once, and the element is still the button's afterwards.
- A click, the keyboard, and the agent's `tap` behave as today.
- #529 ran the inline run's press after the request returns (`DispatchQueue.main.async`), as AppKit's `NSMenuItem`
  runs its own AXPress. The same can work here. Another possible fix is to stop the legacy-API reads during the update,
  so that no error is recorded.

## Acceptance criteria

- In the repro, `AXUIElementPerformAction(btn, kAXPressAction)` and `AXUIElementPerformAction(flat, kAXPressAction)`
  from another process return 0, `bcount` goes up by one for each, and each element is still the button's afterwards.
