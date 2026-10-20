# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, round 2 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `0aec848924`.
- **Method:** one brief (sha256 `8808f3454bdd0a04c7dd61c9a84fc4c6200453f35c6deeea3c672da25690f5a3`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX, 2 NIT).
- **Disposition:** the NITs taken; the bare-base font size taken differently: stage 11 D keeps a bare base as a type's size in tamber and lowering refuses it at a `font` row, now tested (`a_base_name_is_no_font_size_a_row_takes`).

No BLOCKING findings. Round 1's fixes hold, and one font-size hole the commit says is closed is still open.

## SHOULD-FIX

**A bare base is accepted as a font size** — `tamber/src/typeface.rs:64` and `:156`

`text_style` is every `named_length` except `-exact-hairline`. Unset bases are named lengths, so `size` returns `Length::Named("space.md")` before the refusal at line 162 (`a named length no font size takes`). The same skip accepts an alias of a bare base: line 170 requires `!text_style(n)`, and `text_style("space.md")` is true, so it falls through to `Ok` at line 181.

`title: type = space.md` and an untyped `title = space.md/1.2 Inter` both compile. The new arm at line 174 does refuse an expression token in the size slot (`lift: length = calc(space.md * 2)`). A whole-value expression token is refused earlier as "not a type token" (`typeface.rs:80`), which does not mention the base. The commit message says a font size over a base is refused the way a bare one is. Hairline is already excluded; bases need the same exclusion. `text_style` itself is from stage 11; this commit added the expression arm beside it and did not cover the bare name.

## NIT

**An unfoldable ranged param is also reported as not px** — `contract/lower/src/params.rs:385`

`scale::ranged` is `None` when any column's `evaluate` fails, and the value then hits `tamber-range-unit` ("a `rem`, `em` or text style"). Theme lowering already refuses it correctly (`contract/lower/src/theme.rs:493`, naming the column and `MAX_VALUES`/`MAX_DEPTH`). The extra diagnostic describes a different failure. A bare base does not hit this: `ranged` evaluates `calc(name)` in every column.

**`calc(2 / space.md)` uses the length-times-length message** — `tamber/src/measure.rs:533`

Division is matched only as base then number. A number over a base falls into the wildcard, "a length times or over a length". `space.md / 2` and `space.md * 2px` are right.

## Round 1

All nine fixes hold, and none of them adds a new failure on the paths above.

- `unfolded` (`contract/lower/src/scale.rs:65`, `theme.rs:488`) drops a token before it enters the theme. For text this printer emits, a failed `evaluate` is the kernel limit; substitution of a real base does not return `None`. The message always cites `MAX_VALUES`/`MAX_DEPTH`, which matches that case.
- Range compare is `n < low as f32 || n > high as f32` (`params.rs:366`), inclusive, same precision as kernel points. The bound test writes the kernel's own `f32` back.
- `finite` at the end of every sum, and after every product step, refuses `1e308 + 1e308` and `x / 0 * 0`. `Residual::over` divides each coefficient. `* 1e-320 / 1e-320` comes back to `space.md` (`1e-320` is a subnormal, so the divide restores 1). `scaled` drops a coefficient that underflows to 0, so `* 1e-200 * 1e-200 + 2px` is `2px` and prints the same again.
- `Scope::base` records on lookup (`check.rs:1306`). A refused fold calls `self.at` and `check` returns `Err` without the tokens (`check.rs:225`), so a shadow probe or a failed `named()` does not ship `bases`. Kind inference uses the `named_length` closure, not `Names::base`. `influence` refolds the same idents. A hidden `when` that succeeds is recorded on purpose (the field's own comment). A cancelled read stays (`pad.g` is `3px` with `bases: ["space.md"]`).
- Shadows: bare name, alias, expression token, and inline `calc`/`min`/`max`/`clamp` all return `tamber-base-expression` and the message contains "shadow" (`shadow.rs:259`, `:262`, `:283`).

## What checks out

- **Residual.** Like terms keep first-seen order; a zero coefficient is dropped, including `-0`; `calc(space.md)` becomes the alias `space.md`; `min`/`max` fold px and dedupe by structure; `clamp` is not rewritten and both sides use `max(min(mid, hi), lo)`. Printed forms (`space.md * -1`, spaced `+`/`-`, a sum inside `min`/`clamp`, division as `* 0.5`) are what the kernel's calc parser accepts, and `the_carried_text_folds_as_the_written_one` re-evaluates them in every column. One expression, one text, for the forms the tests lock.
- **Kind.** An untyped `calc(space.md * 2)` is a length. `inferred`'s new ident-as-dimension only applies to named lengths; those already failed inside a number `calc`, so a previously shipping number token does not change kind. The commit's byte-identical plan check was not re-run here.
- **Other names.** Number, duration, easing, `light-dark`, and `color-mix` go through `named()` → `base()` and get `tamber-base-expression`. A dial default stays "a dial's value is a literal" (`check.rs:1290`) and does not implement `base()`.
- **Kernel.** Whole-word `named_in` (`space.md2` and `x.space.md` do not match). A sum substituted for a base is parenthesized; `min(...)` is not. Design value and live edit win over the table; reads are recorded (`card.pad` and `space.md`). Depth 8 is the alias cap and then fails closed. Non-fold (`1rem`) is `None`. Points are `f32` via `narrow`.
- **Terminal.** `evaluate` substitutes `Nch` and the calc multiplies by `COLUMN` (8) once. Layout `cells::space` rounds later, keeps the sign of a full cell, and does not clamp a negative expression to 0. Negative padding is the compiler's per-column refusal; margin accepts it.
- **Compiler.** Every column, including terminal, is expanded and a refusal names the column (`place`). Inherited rows use the written tuple count (`theme.rs:1381`), so one expression may be inherited. Bare `space.lg` in a range is checked per column, not `tamber-range-unit`.
- **JS.** `calc(var(--exact-…))`, all nine properties when any base ident appears, SVG and release static rows are that build's px, dev token CSS stays the `calc`. `reads_length` still skips keyword rows.
- **Outside Exact.** Neutral px, source expression kept. `tamber gen` falls back to the expression text; DTCG's `unwrap_or(1.0)` is unreachable while `vocab.sizes` chains `scale::sizes()`. Fold there is `f64`, not kernel `narrow`; the export test locks only `* 2 + 1px`, where they match.
- **Fixture.** `box.pad = calc(space.lg * 2)` with expect 40 Apple, 48 neutral/Material, 32 terminal. No schema, digest, or wire file in the diff. Resolve still returns the expression as `unit: "named"` (stage 12 D).

## Tests

They lock inlining, alias, cancelled `bases`, negative columns, margin, `when` arms, ranges (including the `f32` bound and a bare base), dimension/`rem`/`%` refusals, shadow wording, the kernel limits, the arithmetic edges, kernel re-parse, runner bindings on all five columns, live edit of the expression, JS `calc(var())` / SVG / release vs dev, and the neutral DTCG number for a dyadic expression.

They do not cover a base as a font size, motion, `light-dark`, `color-mix`, dial, or number. The runner skips the terminal padding assert (`contract/cli/tests/base_expression.rs:97`), so cell rounding is only implied. Re-parse allows `1e-3`. No browser parse.
