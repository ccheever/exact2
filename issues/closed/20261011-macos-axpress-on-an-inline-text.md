# macOS: AXPress on an inline text run answers kAXErrorAttributeUnsupported although the press lands

**Status:** Closed
**Resolution:** fixed by #529
**Systems:** host/apple macOS, accessibility
**Severity:** P3
**Author:** Daehyeon Mun
**Date:** 2026-10-11
**Related:** issues/closed/20261010-macos-link-url-accessibility.md (X78); #455; LLP 1115

## Summary

Since #455 (X78) an inline run's accessibility element (`InlineAccessibility`,
`host/apple/Sources/ExactKit/TextInteraction.swift`) is valid outside the app and lists `AXPress`. When another
process sends that `AXPress` through `AXUIElementPerformAction` (the call VoiceOver's VO-Space and Switch Control
make), the press lands: the run's `press=` handler runs, and the element is still valid afterwards. The call still
returns `kAXErrorAttributeUnsupported` (-25205) instead of `kAXErrorSuccess` (0). An Exact `button` node answers 0
to the same call, and so does a hand-built AppKit view's kept `NSAccessibilityElement` child whose
`accessibilityPerformPress()` returns `true`. A client that checks the result, such as an automation tool or an
assistive app, takes a press that worked for a failed one, and may retry it.

The error does not come from Exact's own result. `accessibilityPerformPress()` returns `activateInline(id)`, and
that is `true` for this run: the agent's `tap` reaches the same function and reports `host-activation`. The press
does not replace the element either. With temporary logging, a `tap` on the run re-applied only the count's
paragraph, not the pressed run's, so its `inlineAccessibility` was kept. The cause is not known.

## Why this arose

The X78 fix (#455) recorded it under "Also seen, not fixed here", and its review asked for it to be tracked. X78's
constraint is that a run keeps its `AXPress`. It keeps the action, but the action's result is wrong.

## Reproduction

The X78 repro app, cut to its paragraph with a press-only run (made with `bun scripts/exact.mjs new <dir>`;
`app.ts` exports an empty `sources`):

```text
component X78app
  state presses = 0
  action pressed
    presses = presses + 1
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text testId="q" font-size=24
        text "Press "
        text "here" press=pressed testId="pressrun"
        text " to count"
      text `Presses: ${presses}` testId="count" font-size=24
```

Run `bun exact.mjs mac`, launch the built binary directly, and from another process with the accessibility grant
find the element whose `AXIdentifier` is `pressrun` in its window, then call
`AXUIElementPerformAction(element, kAXPressAction)` and read `count`'s `AXValue` before and after (the X78 record's
`axpress` probe).

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| `AXPress` on the run "here" (`AXStaticText`, actions `["AXPress"]`) | macOS 26.6.2 (25G83), Apple Silicon | #455 head `1b088a772` | -25205; `Presses: 0` → `Presses: 1`; afterwards still `AXStaticText` and still the run's element | 0, as for a button | record, "AFTER", `axpress` |
| `AXPress` on an Exact `button` node | macOS | #455 head | 0 | — | record, "Also seen" |
| Hand-built: a view's kept `NSAccessibilityElement` child, `accessibilityPerformPress()` → `true`, plain, with the legacy `accessibilityAttributeNames`/`accessibilityAttributeValue` overrides `InlineAccessibility` has (`AXLanguage` set or nil), and with a computed `accessibilityFrame` | macOS 26.6.2, `swiftc` | — | 0 in all four | — | record, "Also seen"; `x78-press-oracle.swift.txt` |

Not run: VoiceOver itself; an `href` run's `AXPress` result (it opens a URL); iOS. An attempt to narrow it down while #455's review
was addressed could not read the app from outside: while the session's screen is locked, macOS answers
`AXWindows`, `AXFocusedWindow`, `AXChildren` and hit tests with the application element itself, even to the app's
own process, so the window's elements cannot be reached. Investigate with the screen unlocked.

## Constraints

- The press still lands once, and the element stays the run's (X78's constraints).
- In-process readers (the agent's `tree --ax` and `tap`, a native module) behave as today.
- Next steps to narrow it, each probed from another process: `InlineAccessibility.accessibilityPerformPress()`
  returning `true` without activating; activating after the call returns; without the legacy `AXLanguage`
  overrides.

## Acceptance criteria

- In the repro, `AXUIElementPerformAction(pressrun, kAXPressAction)` from another process returns 0, `count` goes
  from `Presses: 0` to `Presses: 1`, and the element is still the run's afterwards.
- An `href` run's `AXPress` from another process returns 0 and opens its URL once.
