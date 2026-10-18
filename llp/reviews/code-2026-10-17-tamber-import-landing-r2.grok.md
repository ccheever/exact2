# Code review: An import is checked where it lands (LLP 1121 D14, H's follow-up), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD at the time (`225aa2718`, `837bbbc46`), the landing check and its first round.
- **Method:** one brief (sha256 `d0444e526777d3fae31e8f9b0d57a1088736b86414c3c58bc9292354599e830c`), shared by both reviewers, blind to the other review, with the first round's records beside the worktree. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0: an untyped token's kind when the design does not fold; P1s: kept whens invisible to blame, a when compared with its default).
- **Disposition:** the last round, applied without re-review in the second-round commit: every item, each with a test in `tamber/tests/it/import_kinds.rs` (`the_landing_check_keeps_what_holds_and_refuses_axes_that_do_not`).

---

1. **P0 — An untyped token changes kind when the design does not already fold.** `tamber/src/import/landing.rs:142`

`have` is the folded kind only when `check_design` returns `Ok`. On `Err` it is only the written kind (`d.kind`). A token whose kind is inferred (`tw.brand = 8px`) is skipped, and the later fold never sees a new diagnostic if the import also clears the old one. `apply_all` then checks a design that folds, and writes the colour.

Failing input, `tailwind::import_with` with any vocabulary:

```tamber
tamber app

theme
  tw.radius.md: length = 1rem
  derived: length = calc(tw.radius.md + 1px)
  tw.brand = 8px
```

```css
@theme { --radius-md: 4px; --color-brand: #ff0000; }
```

`tw.radius.md` becomes `4px` and `derived` folds. `tw.brand` is proposed as `#ff0000`. Accept turns the length into a colour. A written `: number` is still caught; this path is not. The same hole accepts a length onto an untyped colour. `docs/tamber-protocol.md:538` and `llp/1121-tamber.rfc.md:2991` say a colour the design already has of another kind is unmapped.

Fix: on the `Err` path, infer the existing kind the way `kinds()` does (written kind, otherwise the value, walking names in the design). Do not wait for a clean `check_design`.

2. **P1 — A kept `when` on an imported token is invisible to blame, so a valid update is reported.** `tamber/src/import/landing.rs:181`

`reads` uses the import's own value and `when`s. `laid` also keeps author `when`s on axes the importer does not own (Tailwind owns none). A single candidate is charged with no test that removing it clears the diagnostic (`landing.rs:270`), and an empty culprit list falls back to every candidate (`landing.rs:284`).

Failing input:

```tamber
tamber app

axes
  mode = normal | compact

theme
  tw.radius.md: length = 4px
    when mode=compact: tw.radius.sm
  tw.radius.sm: length = 2px
  derived: length = calc(tw.radius.md + 1px)
```

```css
@theme { --radius-md: 8px; --radius-sm: 1rem; }
```

`8px` is a valid length. The compact `when` still names `tw.radius.sm`, so `derived` fails only because `--radius-sm` is `1rem`. `reads` sees `tw.radius.md` as `8px` and never the kept `when`, charges `tw.radius.md`, then charges `tw.radius.sm` on the next pass. Both are unmapped. `8px` is not proposed.

Fix: walk the text `laid` actually leaves, kept `when`s included. Run the absence test for one candidate too, and do not fall back to a candidate that failed it.

3. **P1 — `tamber-under-light` on another token's `when` refuses the whole import.** `tamber/src/import/landing.rs:258`

The diagnostic is reported on the `when` line (`check.rs:1154`). Blame uses only that `when`'s text. The failure is the base side compared with the `when`'s light side. A literal `when` names no import token, `candidates` stays empty, and `landing.rs:320` charges every token still laid.

Failing input:

```tamber
tamber app

axes
  dark-style = black | dim  under scheme=dark

theme
  tw.brand = #ffffff
  derived = tw.brand
    when dark-style=dim: light-dark(#ffffff, #222222)
```

```css
@theme { --color-brand: #000000; --color-ink: #111111; }
```

The design folds today. After the import, `derived`'s default is `#000000` and the `when`'s light side stays `#ffffff`. `--color-ink` is a fine new colour. Both tokens are unmapped with "the design would not fold once the import lands".

Fix: for `tamber-under-light`, `tamber-under-kind`, and `tamber-under-role`, take candidates from the base value and the `when` value, then keep the absence test.

Verdict: WRONG
