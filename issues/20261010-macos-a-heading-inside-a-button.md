# macOS: a heading inside a button is not exposed, because the host makes every button an accessibility leaf

**Status:** Open
**Systems:** host/apple macOS, accessibility
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x74-macos-heading-inside-button.md; LLP 1080.002 (`tree --ax`); LLP 1115

## Summary

On the web, a heading inside a button stays in the accessibility tree. In Chrome, `<button><h2>Legacy
features</h2></button>` is button "Legacy features" › heading "Legacy features" [level=2]. Chrome does not hide a
button's heading under ARIA's "children presentational". The Exact web build does the same. It renders the Contract heading as `<span role="heading" aria-level="2">` inside
the `<button>`, and `tree --ax` shows the heading under the button.

The macOS host drops it. `NodeView.accessibilityChildren` returns nil for every node that `actsAsButton`
(`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:358-361`; `Accessibility.swift:46`: a `button`, or a view
whose role is button, link, checkbox, radio or switch). The doc comment above it says "A control is a leaf, as
UIKit makes one: VoiceOver reads its name" (`:342`). The button is an `AXButton` named "Legacy features" with no
children, and the heading's `AXHeading` (`TextInteraction.swift:55-70`) is never reached. A heading outside a
button is exposed as usual.

## Why this arose

T3 Code's Settings › General ends with "Legacy features", a Base UI `CollapsibleTrigger` (a button) that holds
`<h2>Legacy features</h2>` and a chevron (`LegacyFeaturesSection`, `SettingsPanels.tsx`). Playwright's ARIA
snapshot of the running reference shows button "Legacy features" › heading "Legacy features" [level=2]
([ref-legacy-aria.txt](https://raw.githubusercontent.com/ccheever/exact2/98e34bd659cfadd18ece51a01a5f85fc61828137/settings-headings/ref-legacy-aria.txt)). The
clone's task `settings-headings` (T3 PR #404, merged `917ddd341`) found that it could not put the heading inside
the button on the Mac. It puts an sr-only level-2 heading right before the button instead (`SettingsSrHeading`,
`examples/t3-code/settings-kit.contract`; `settings-rows.contract` `CoreSections`, on the T3 branch). The
heading list and levels match the reference, but the heading is the button's sibling, not its child.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `d413487a8489d305bf214c276665931a3faff1f0`. Its
`app.ts` exports an empty `sources`.

```text
component X74app
  state open = false
  action toggle
    open = not open
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      text "Settings" role="heading" aria-level=1 testId="title"
      button press=toggle aria-expanded=open testId="trigger"
        text "Legacy features" role="heading" aria-level=2 testId="trigger-heading"
      when open
        text "Folded rows" testId="rows"
      text "Plain heading" role="heading" aria-level=2 testId="plain-heading"
```

Run `bun exact.mjs mac`, then `bun exact.mjs agent <web|macos> --json --size 420x300 "tree --ax" "tap trigger"
"tree --ax"`. `tree --ax` reads Chrome's tree over CDP on the web and the host's NSAccessibility tree on macOS
(LLP 1080.002). It reports no finding on either host.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| `tree --ax`, before and after `tap trigger` | macOS 26.6.2 (25G83), Apple Silicon | main `d413487a8` | button "Legacy features" (view 3) has no children. The heading (view 4, `trigger-heading`) is not in the tree. "Settings" and "Plain heading" are `AXHeading` | the heading under the button, as on the web | image (right), record M |
| Same, web | Exact web (JS target) in Chrome 155.0.8059.39 | main `d413487a8` | button "Legacy features" › heading "Legacy features" [level=2] (view 4, a `span role="heading"`) | (the reference) | image (left), record W |
| Hand-written HTML: `button > h2`, `button > span[role=heading]`, `div[role=button] > h2` | headless Chrome 155 (CDP `Accessibility.getFullAXTree`) | — | each: button › heading [level=2] | — | record |

![X74: a heading inside a button, on the Exact web and on macOS](https://raw.githubusercontent.com/ccheever/exact2/5dcf1f60e9b0c24fea18103b894a6ce5f0f5e40d/fw-issues-20261010f/x74-heading-in-button-web-macos.png)

Record (contract, commands, both trees, the HTML oracle):
https://raw.githubusercontent.com/ccheever/exact2/5dcf1f60e9b0c24fea18103b894a6ce5f0f5e40d/fw-issues-20261010f/x74-record.txt

Not run: #327's head, iOS (UIKit also hides an accessibility element's children), Linux, and VoiceOver itself.
A hand-built AppKit `NSButton` is a leaf `AXButton` with a title and no children, so AppKit has no button with a
heading inside to copy. A SwiftUI `Button` whose label has the header trait was not read; that probe did not
work.

## Constraints

- The web is the parity oracle (`CLAUDE.md`), and the author wrote the heading (`role="heading"`, `aria-level`).
  LLP 1115: author > platform > CSS default.
- The leaf stays for what the author leaves unsaid. A button's text, icons and images are its name, and
  VoiceOver reads the button once, by that name. Only a descendant that has its own role and that the web would
  expose is affected, such as a heading.
- The button keeps its role, name, `aria-expanded` and press, and it stays one Tab stop. The heading inside it
  takes no focus and no press of its own.
- `aria-hidden` inside the button still hides (`accessibilityChildren` returns `[]` for an
  `accessibilityElementsHidden` node).
- Main PR #327 (head `14493d253`) edits `NodeViewMac.swift`, but only to route `mouseEntered`/`mouseMoved`/
  `mouseExited` through `presenter.trackPointer`. It changes neither `accessibilityChildren` nor
  `updateRoleAccessibility` (read from its diff, not run).

## Acceptance criteria

- In the repro on macOS, `tree --ax` shows heading "Legacy features" [level=2] (view 4) under button "Legacy
  features" (view 3), before and after the tap, as on the web.
- The button's name, its expanded state and its press work as today.
- A button that holds only text or an image (no role of its own) is still a leaf `AXButton` with no children.
- A macOS XCTest or a `tree --ax` parity case covers a button with a heading inside.
