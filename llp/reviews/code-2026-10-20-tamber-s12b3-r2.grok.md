# Code review round 2: LLP 1121 stage 12 B3 (a16ed67f65), Grok

**RIGHT WITH CHANGES**

Round 1 is fixed, and the widened length-function path is sound when the kind is written. One hint was left behind by that widening.

**1. SHOULD-FIX.** `tamber/src/shadow.rs:152`. `looks` counts a dimension, a bare `0`, and a length name, and skips `Item::Func`. An unkinded value is then inferred `Color` (`tamber/src/check.rs:814`) and folded by `tamber/src/fold.rs:205`, so the "reads as a shadow: write its kind" arm (`tamber/src/check.rs:727`) never runs.

Repro: `card.shadow: 0 max(1px, 2px) calc(2px * 3) black` — the `fn-px` fixture in `contract/cli/tests/it/tamber_base_shadow.rs:201` without `: box-shadow`. `looks` counts only `0`. `color` sees that number and returns `not_color_kind()` (`tamber/src/fold.rs:221`): "not a colour: a type token is CSS `font` syntax…". The same text with `: box-shadow` folds to `0px 2px 6px`. With no plain length, `card.shadow: min(1px, 2px) max(3px, 4px) calc(5px) #000` hits `tamber/src/fold.rs:217`, "a colour is one value". `0 calc(gap * 2) 4px black` still prompts, because two plain lengths remain. Writing the kind is the documented rule and that path folds correctly; the hint that exists for an omitted kind was not updated for the functions this change accepts.

The widening itself holds. `length` (`tamber/src/shadow.rs:358`) sends every `calc`/`min`/`max`/`clamp` through `measure::length`. A set base or a px token becomes `Px` (`space.md = 10px` then `calc(space.md * 2)` is 20px; `gap = 3px` then `calc(gap * 2)` is 6px). An unset base stays `Base` and is read via `Names::base` before `named`, including an alias and an inlined `Length::Base`. Nested functions and `min(space.lg, gap * 4)` stay residuals where a base is unset. B2 refusals still fail inside a shadow (`tamber_base_shadow.rs:226`: length times length, rem/em, number over length, mixed min). Text shadows use the same `length`. A folded negative blur is refused at check time (`tamber/src/shadow.rs:267`); a residual blur stays on the per-column check. A literal `-2px` blur is tested.

Round 1, as it stands:

- A1: the fallthrough is gone; the function fold is the path above.
- A2/G1: `looks_read` and `also_read` OR bits (`runner/src/instance/theme.rs:89`, `:97`). `state_looks.rs:273` sets a look's shadow to `0px space.md 4px` and then `space.md` to 30px.
- A3: `resolve` keeps numeric `lengths` with null for a residual, and emits `length_texts` only when one is (`contract/cli/src/tamber_base.rs:345`). The 1.15 clause names `bases` and `length_texts` (`docs/tamber-protocol.md:118`).
- A4/G2: `dotted_name` (`contract/lower/src/theme/scan.rs:420`) requires a letter first and a dot, so `-1.5px` and `rgb(...)` are not unknown tokens. A base or length on a border or gradient keeps the row refusal; on a literal shadow it points at a shadow token (`scan.rs:454`).
- A5/G5: `an_export_from_before_the_shadow_clause_still_finds_its_baseline` (`tamber/tests/it/export.rs:133`). The baseline matches the design digest, so the new CONVERSIONS sentence does not hide an old export.
- G4: `COLOUR_FUNCTIONS` is skipped with `eq_ignore_ascii_case` (`kernel/src/style/scale.rs:190`), so `RGB(...)` and `light-dark(space.md, #000)` stay whole.

`export.rs` is 1499 lines and `contract/lower/src/theme.rs` is 1496. No schema or format digest source is in the diff. I did not re-run the 43-app byte compare or the tests; the G1, G2, and A4 tests are present and assert the fixed behaviour.
