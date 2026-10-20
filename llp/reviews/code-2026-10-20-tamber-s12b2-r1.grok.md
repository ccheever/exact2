# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, round 1 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `d05d20fbb1`.
- **Method:** one brief (sha256 `0cc4912f79bbbf7f36118133830467c22f1b044a471a769558d9c484d8d84e79`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX).
- **Disposition:** taken in the next amend (a sum's finiteness).

No blocking findings. One should-fix: a sum can overflow to a non-finite residual, and the text that stores does not fold.

## SHOULD-FIX

`sum` returns a residual (and a px sum) with no finite check. `product` grew that check in this commit and then returns.

```458:463:tamber/src/measure.rs
            (a, b) if a.length() && b.length() => {
                let sign = if op == '+' { 1.0 } else { -1.0 };
                let (Some(a), Some(b)) = (a.residual(), b.residual()) else {
                    unreachable!("two lengths")
                };
                based(a.plus(b, sign))
```

```530:539:tamber/src/measure.rs
    let finite = match &out {
        Some(Quantity::Px(n) | Quantity::Number(n)) => n.is_finite(),
        Some(Quantity::Base(r)) => r.finite(),
        None => true,
    };
    match out {
        Some(_) if !finite => fail(
            "tamber-kind",
            format!("`{text}` does not fold to a finite value"),
        ),
```

`calc(space.md * 1e308 + space.md * 1e308)` is two finite products, then a sum whose coefficient is `+∞`. `calc(space.md + 1e308px + 1e308px)` does the same to the px part. `num` prints that as `inf` (`tamber/src/measure.rs:164`, `num/src/text.rs:142`). The kernel's calc keyword is `infinity`, not `inf` (`kernel/src/style/calc/parse.rs:358`), so `scale::fold` fails. `columns` then returns `None` on the `?` (`contract/lower/src/scale.rs:57`), and `place` skips the value (`contract/lower/src/theme.rs:1359`), so the refusal is "in some app mode" rather than the finite-value error.

A sum of finite values overflows to infinity, not NaN. The printer is still wrong if a NaN is ever stored: a NaN px is neither `< 0` nor `> 0`, so `residual.rs:219` drops it and `calc(space.md + …)` becomes `calc(space.md)`; a NaN coefficient prints `NaN`, which the kernel accepts as `nan` and `narrow` turns into 0 (`kernel/src/style/calc/mod.rs:221`). The same `finite()` check on `sum`'s result covers the coefficient, the px, and a non-finite constant that `min`/`max` would otherwise keep (`measure.rs:405`).

## What checks out

**Normal form.** Like terms add in first-seen order, zero coefficients drop, `* 0` and `a - a` become px, and `calc(space.md)` (`coeff == 1`, `px == 0`) becomes `Length::Named`. `min`/`max` fold px arguments into one, dedupe the rest, and leave a single survivor unwrapped. Nested `calc(max(…) + 1px)` and `clamp` print as one text per structure. `clamp` folds as `mid.min(hi).max(lo)`, matching the kernel. Division prints as `* 0.5` (and similar). A leading negative coefficient prints `space.md * -1`, which both the kernel (signed number, `*` needs no spaces) and a browser accept. `+` / `-` are printed with the spaces the kernel requires. `Shortest` is plain decimal, so a finite coefficient round-trips through the kernel parser. Compiler and hosts both `evaluate` that stored text, so they agree with each other.

**Kind and `bases`.** An untyped `calc(space.md * 2)` is a length because `kind_of` asks `measures()` (`tamber/src/check.rs:767`), and `named_length` already treats an unset base as a dimension. The new ident arm in `inferred()` is not that path, so existing plans and CSS stay as they are. `bases` records `Length::Named` and `Length::Base::bases()` in order, deduped, only while the name is still a vocabulary base (`check.rs:1105`).

**Other `Names` paths still refuse.** An unset base reaches `Scope::named` (`check.rs:1314`) with `tamber-base-expression`, which names shadows and the queue item. A shadow token that is `Length::Base` is the same error (`shadow.rs:259`). A font size that is a bare base stays "a named length no font size takes"; a token over a base is `tamber-kind` and quotes the expression (`typeface.rs:174`). Number, motion, light-dark, color-mix and relative color go through `named`, so an unset base gets that same refusal; a `Length::Base` token used as a color is "a length, not a colour". A dial default is still "a dial's value is a literal". `rem`/`em` and `%` keep their calc messages. A base times or over a length is the dimension error (`measure.rs:519`).

**Kernel.** `measure_at` (`theme.rs:260`) records the token and each base, prefers a design definition (including a live edit and an alias), and shares the depth cap of 8 with aliases. A value that does not fold (`1rem`) makes the read `None`. The result is an f32 point length via the existing calc parser. `named_in` is a whole-word scan (`.` `_` `-` included), so `space.md2` and `x.space.md` do not match. A replacement with spaces outside parentheses is grouped.

**Terminal and negatives.** Cells are substituted as `Nch`; calc multiplies by `COLUMN` (8). Layout's `cell_spaces` rounds once, per axis (`COLUMN` 8 across, `ROW` 16 down), and keeps the sign. There is no second rounding and no clamp of a negative to 0. The conformance row `calc(space.lg * 2)` is 40 / 40 / 48 / 48 px and 32 px on the terminal (2 cells × 2 × 8), which is 4 columns by 2 rows. A negative finite px is refused per column, by name, ranges included (`params.rs:363`). Because the host stores a px literal, a nonnegative row sees a negative literal and refuses it; it does not take the math-function clamp-to-0 path.

**Compiler.** `columns` evaluates all five columns. The inherited-row rule uses the written tuple count (`theme.rs:1367`), so one expression or one bare base may be inherited; a token that already varies by axis still may not. A bare base in a ranged param is checked as `calc(name)`. A text style or hairline still takes `tamber-range-unit`.

**JS, export, schema.** `expression` writes `calc(var(--exact-…))`. Theme-row values are ident-scanned, and any base name declares all nine properties. An SVG attribute gets `folded()` px of this build's column (web's is neutral). Release static rows are that px; dev token CSS stays the `calc(var())`. `tamber gen` and the DTCG export fold the neutral column and keep the source expression. Resolve/explain stay unit `named`, amount null. The commit does not touch schema, digest or wire. Shadows and per-column resolve/explain are the two queue items this stage left.

## Tests

The new tests cover canonical text, per-column refusal of a negative, margin accepting a negative, param range, dimension and unit refusals, both shadow paths, DTCG source-plus-neutral-px, the runner's column fold, a design-set base, `min`/`max`/`clamp`, terminal `2 * COLUMN + 4`, substitution grouping, JS `calc(var())` with the nine declarations in release and dev, and the SVG px.

They do not cover a non-finite sum, a printed expression parsed back by the kernel (`* -1`, `/ 3`, a sum inside `min`, `clamp` with the low bound above the high bound), motion / light-dark / color-mix / dial / font-size / number refusals, or a live edit of a base under an expression that is already resolved. The runner test skips the terminal padding assert; the conformance fixture is what locks 32.
