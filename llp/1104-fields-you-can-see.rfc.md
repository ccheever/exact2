# LLP 1104: Fields you can see — a visible text field by default

**Type:** RFC
**Status:** Built r3, 2026-10-06 (§6), except D4's focus states, which are deferred. Charlie decided the direction (LLP 1102 §0, §3.15: "visible by default is right"). r2 folded in one blind pass by Astra and Grok (both READY WITH CHANGES, §5). This amends LLP 1102 §0's "the platform's field" to these default rows.
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

**Which fields.** Rows go on the text-field arm of lowering (`controls.rs`, which already builds a separate tag for checkbox, radio, range, date, time, datetime-local and file). They apply to:
- `textarea`;
- an `input` whose type is `text`, `email`, `password`, `search`, `tel`, `url`, `number`, or none.

`hidden`, `color`, `month` and `week` lower as text fields today and get no rows: a hidden input must paint nothing.

**The rows.** Longhands, in the forms `fixed_styles` stores and the kernel accepts (a shorthand like `border` has no `StyleId`, and a value the kernel refuses would be dropped silently):

| Rows | Default |
|---|---|
| `border-top/right/bottom/left-width` | `1` |
| `border-top/right/bottom/left-style` | `solid` |
| `border-top/right/bottom/left-color` | `light-dark(#c6c6c8, #48484a)` |
| `border-*-radius` (four corners) | `6` |
| `padding-top`, `padding-bottom` | `6` |
| `padding-left`, `padding-right` | `8` |
| `background-color` | `light-dark(#ffffff, #1c1c1e)` |
| `color` | `light-dark(#000000, #ffffff)` |

`color` is set so a field's text tracks its fill. Without it, a parent's fixed black text would land on the dark fill.

The values are a starting point and are tuned from screenshots (D5).

**Size.** The field's box grows by its padding and border. The kernel and every host already measure text-field padding and border (`TextInput` returns content size, and the box adds the rest), so nothing new is needed. But an authored `width` on a field now means the content width plus 18 px, because the box stays `content-box` as CSS's is. `field-sizing` and a textarea's `rows` keep working; the padding sits outside them. Apps that set a field's width are checked in D5's pass.

### D3 — How an author changes it

- **Any authored row wins** over its default, row by row: fixed rows are pushed first, then a class's rows, then the node's own attributes overwrite the same row. A class that sets `background-color` keeps the default border.
- **`appearance="none"` drops the default rows,** giving today's bare box. It must be a literal, read the way a native button's `appearance` is (class, then own attribute). It is a compile-time switch that omits the rows, not a CSS `appearance` value emitted to the page. It removes only the injected rows: authored rows and a textarea's wrapping stay.
- **A conditional class.** `class=(c ? A : B)` emits `none` for a row one branch omits, and the runner clears it to the kernel's default, not this tag default. So for each of these rows the lowering emits the field default instead of `none` when a branch omits it, keeping the field visible across the switch. This needs a test that switches between a class that sets `padding` and one that does not.
- **`appearance="auto"` on a text field** is left for a later decision (native field chrome). It is refused for now, rather than restoring search decorations and number spinners on top of the rows.

### D4 — The web host and focus

`host/web/index.html`'s `all: unset` on `input, textarea` stays. The rows arrive as the node's inline style, as any authored row does.

**Focus.** r1 claimed the browser's focus ring stays; it does not. The reset restores `:focus-visible` only for buttons. A visible field needs a visible focus state on every host:
- web: `input:focus-visible, textarea:focus-visible { outline: revert; outline-offset: 0 }`, for rowed fields;
- macOS: `focusRingType` stays `.none` on the native view, and the field shows focus by its border colour (an accent border while editing, a host-side paint);
- iOS: the caret is the focus indicator, as in system forms;
- Linux: the caret, plus the accent border as on macOS.

`number` and `search` lose their spinners and search chrome under the reset already; with the rows they look like text fields, which is intended.

### D5 — Verification before landing

- **Screenshots, light and dark:** a bare field, a styled field, an `appearance="none"` field, a field under a parent with a fixed `color`, and a field on a dark page. On web, macOS, iOS and Linux (Linux paints the same box).
- **Placeholder colour on each host:** web's fixed pair, Apple's 30% of the field's ink, Linux's fixed `#757575`.
- **Disabled fields.**
- **Conformance:** the JS target against the wasm oracle on a fixture with all of the above.
- **Native geometry:** a single-line field's text rect and baseline inside the content box on iOS and macOS, a number field, and a search field (UIKit's header search is projected into `UISearchController` and keeps its own look).
- **The repo's apps:** each field looks right with the defaults, or gets `appearance="none"` or its own rows, including any that set a width.
- **The bench's next rounds:** whether builders still style fields only because they look missing.

## 3. Cost

| Part | Estimate |
|---|---|
| Lowering: the longhand rows on the text-field arm, the excluded types, the literal `appearance="none"`, the conditional-class fallback, tests | a lane-day |
| Focus states on web, macOS and Linux; screenshots and tuning on four hosts; conformance fixture; native geometry checks | a lane-day |
| The repo's apps and docs (the guide's controls section, the obsolete pitfall, LLP 1064 D6's amendment) | half a lane-day |

About two and a half lane-days.

## 4. Open questions

1. Should `select` get the same rows? It draws the platform's menu button on every host today, which is already visible. Proposed: no.
2. Should a disabled field dim its rows (`opacity`), as the web's UA does? Proposed: yes, `opacity: 0.5` while `disabled`, overridable.
3. The exact colors and padding (D2): Charlie's eye, from the screenshots.

## 5. Revisions

- r1, 2026-10-06: first draft.
- r2, 2026-10-06: one blind pass by Astra and Grok, both READY WITH CHANGES. Folded in:
  - longhand rows on the text-field arm, not shorthands on the base tag;
  - `hidden`, `color`, `month` and `week` excluded;
  - a literal, compile-time `appearance="none"`, and `"auto"` refused for now;
  - the conditional-class fallback;
  - a default `color`;
  - focus states, since r1's claim that the browser's ring stays was false;
  - the content-box growth;
  - Linux, placeholders and native geometry in verification;
  - the cost raised.
- r3, 2026-10-06: built (§6). Astra and Grok reviewed it blind; both said LAND WITH FIXES. Folded in:
  - fallbacks resolve by style row, so a style's own covering row wins and the sheet's last row decides;
  - three corpus fields opt out;
  - the tests assert exact colours.

## 6. As built

- **Lowering** (`contract/lower/src/fields.rs`): the D2 rows are a sheet pushed under the node's classes and attributes, as a grouped list's sheet is (LLP 1084 D7). A field is a `textarea` that is not the Markdown editor, or an `input` whose `type` is absent or one of D2's (a `shown ? "text" : "password"` choice counts when both sides do). A literal `appearance="none"` (class, then own) leaves the rows out; `"auto"` and a computed `appearance` are refused (`lower-field-appearance`).
- **Conditional classes:** where one side of `class=(c ? A : B)` leaves a row unset, that side gets what the row resolves to on that side, by style row:
  - another of that style's rows that covers it (`Red`'s `border` for the `border-color` only `Blue` writes);
  - else the sheet's last row that covers it (the order emission uses);
  - spelled as a shorthand where the row is one (`padding` as `"6px 8px 6px 8px"`, `border` as `"1px solid <colour>"` when its sides agree).

  A grouped row's sheet gets the same fill. The list's own rows (`list_rows`) are spliced in after this pass and are not covered.
- **Tests:** `contract/cli/tests/it/visible_fields.rs` (the rows, the excluded types, overrides by shorthand and class, the content-box growth, the class switch, the refusals). The content-sized composer corpus test now expects `max-height` plus the 14px of padding and border.
- **Screenshots**, light and dark, of a fixture covering D5's cases (bare, email, password, search, number, a class's fill, an authored border, `appearance="none"`, disabled, a textarea, a field under `color="#000000"`), on the web, macOS, iOS (iPhone 17 simulator) and Linux. All four draw the same field. Typed text is the field's own ink under a black parent in dark mode. Placeholders read on every host.
- **The repo's apps:** the 43 fields (and three in `contract/corpus`) that draw their own look (any authored border, padding or fill, including every `border-width=0` composer and search bar) now say `appearance="none"`, so they look as they did. RealWorld's classes set every row, so its fields are unchanged without it. The fields with no styling of their own (`windows-desk`, `native-fixture`'s searches, `markdown-stress`'s link URL) now get the default look.
- **Docs:** the agents' guide (beside `textarea rows`) and the humans' guide (Input, events, and commands).

**Deferred:**
- **D4 focus states.** A field shows focus by its caret on every host, as a bare field did before. A web `:focus-visible` ring for dressed fields only needs a marker the web host can select on. Inline styles and the JS target's static classes cannot be told apart by a selector, and there is no `outline` row an author could use to turn a ring off. So the marker is a new prop, which is its own change, together with the macOS/Linux accent border.
- **§4 Q2, a disabled field's dimming.** Not built; awaiting the decision.

