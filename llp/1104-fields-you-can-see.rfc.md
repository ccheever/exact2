# LLP 1104: Fields you can see — a visible text field by default

**Type:** RFC
**Status:** Draft r1, 2026-10-06. Charlie decided the direction (LLP 1102 §0, §3.15: "visible by default is right"); this document is the design before building.
**Systems:** Contract lowering (`contract/lower/src/tags.rs` `input`/`textarea` `fixed_styles`, `controls.rs` for input types), the web host's control reset (`host/web/index.html`), the JS target (no change beyond the rows), the Apple and Linux hosts (they already paint authored rows; their native field chrome stays off), docs, conformance, apps in the repo that relied on a bare field
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-06
**Implementer:** Claude (Opus 5.5) lanes, after review
**Amends:** LLP 1064 D6 (a field is a bare box); LLP 1069.011 (a button keeps its rule: bare by default, `appearance="auto"` for the platform's)
**Related:** LLP 1102 §3.15 (the finding: 11 trials found an invisible field only from a screenshot; the reviewers' notes on the revert list and the measurement path); LLP 1069.001 D3/D6 (checkbox, radio, range and date controls are the platform's); the web is the standard (`CLAUDE.md`)

## 1. Summary

A bare `input` or `textarea` draws nothing: no border, no background. It looks like plain text until an author styles it. The web's own default is a visible field, and every builder expected one.

This RFC makes a text field visible by default, by giving it **default style rows**, as the browser's UA stylesheet does: a border, padding, a corner radius and a field background. Authored rows override each one, and `appearance="none"` drops them all to the bare box.

Buttons are unchanged: a bare `button` stays a bare pressable, and `appearance="auto"` gives the platform's button.

## 2. Decisions

### D1 — Default rows, not native chrome

The visible field is ordinary style rows that the compiler adds to a text field, not the platform's own field chrome (`UITextField.borderStyle = .roundedRect`, `NSTextField`'s bezel, the browser's UA field).

This is the cheap and exact route:
- **Measurement is free.** The rows are a border and padding in the box, which the kernel already measures on every host. Native chrome has its own metrics that would have to enter measurement per platform (`TextInput` measures by a different path from `Control`, `kernel/src/layout.rs`).
- **Parity is free.** Every host paints the same rows: the web as inline style, Apple and Linux through their painters, which already paint authored borders and backgrounds.
- **Authors compose with it.** `border-radius=12` changes the radius and keeps the rest. Native chrome is all or nothing.

The cost is that the field is not pixel-for-pixel the platform's. It is a neutral, recognizable field. A later `appearance="auto"` on fields could opt into true native chrome, as buttons do.

### D2 — The rows

For `textarea` and for an `input` whose type is a text kind (`text`, `email`, `password`, `search`, `tel`, `url`, `number`, or no type):

| Row | Default |
|---|---|
| `border` | `1px solid light-dark(#c6c6c8, #48484a)` |
| `border-radius` | `6` |
| `padding` | `6px 8px` |
| `background-color` | `light-dark(#ffffff, #1c1c1e)` |

The values are a starting point, close to iOS's and the web's light and dark field colors, and should be checked in screenshots on each host before landing (D5).

Checkbox, radio, range, date, time, datetime-local and file inputs keep LLP 1069.001's platform controls: no rows.

### D3 — How an author changes it

- **Any authored row wins** over its default, row by row. `fixed_styles` already work this way for `column`'s `display`.
- **A class's rows win too**, as an authored row does.
- **`appearance="none"` drops the four default rows,** giving today's bare box. It is the opt-out for a field drawn entirely by the app (a chat composer, an inline title editor).

### D4 — The web host

`host/web/index.html`'s `all: unset` on `input, textarea` stays. The default rows arrive as the node's inline style, as any authored row does, so the reset and the rows compose without a revert list. A focus ring stays the browser's (`outline` on `:focus-visible`), as for buttons.

### D5 — Verification before landing

- Screenshots of a bare field, a styled field and an `appearance="none"` field, light and dark: web, macOS, iOS.
- Conformance: the JS target against the wasm oracle on a fixture with all three.
- The repo's apps: each `input`/`textarea` either looks right with the defaults or gets `appearance="none"` or its own rows. Breaking a look is acceptable (ideal end states); an invisible change is not.
- The bench's next round: whether builders still style fields only because they look missing.

## 3. Cost

| Part | Estimate |
|---|---|
| Lowering: the rows in `tags.rs`, text-kind detection with `controls.rs`, `appearance="none"` on fields, tests | half a lane-day |
| Screenshots and tuning on web, macOS and iOS; conformance fixture | half a lane-day |
| The repo's apps and docs (the guide's controls section, the agent pitfall that becomes obsolete, LLP 1064 D6's amendment) | half a lane-day |

About a lane-day and a half.

## 4. Open questions

1. Should `select` get the same rows? It draws the platform's menu button on every host today, which is already visible. Proposed: no.
2. Should a disabled field dim its rows (`opacity`), as the web's UA does? Proposed: yes, `opacity: 0.5` while `disabled`, overridable.
3. The exact colors and padding (D2): Charlie's eye, from the screenshots.

## 5. Revisions

- r1, 2026-10-06: first draft.
