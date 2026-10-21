# Code review: Stage 11 (c): the terminal's spaces in whole cells, laid out by the kernel (Env::cell_spaces), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f7f2acc7b0`.
- **Method:** one brief (sha256 `39f89621f23d29d6fbde042dbaa1de19d15ad76f5d174b4fd5e9acca561df71f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; three should-fix (multi-column gap unrounded; calc(0% + px) unrounded; LLP 1101's 9 px range), one nit (the even failure text)
- **Disposition:** all taken in a623af8488

No blocking findings. Flex and grid padding, margin, and gap go through `cells::space` only when the terminal host sets the flag, and the stage-11 numbers and tests match that path.

## Should-fix

**1. Multi-column `column-gap` is not rounded.** `kernel/src/fragment.rs:182-187` returns the raw `column_gap` (or `font_size` when the gap is unauthored). `taffy_multicol` stores that at `fragment.rs:204`, and `kernel/src/fragment/cut.rs:404-407` places column `k` at `k * (w + gap)`. Flex and grid gap is rounded at `kernel/src/style.rs:1363-1369`. A terminal block with `column-count` or `column-width` and `column-gap: 12px` is laid 12 px apart; the same length on a flex or grid row is 8 px. The stage-11 fixture is a flex `gap-sm` list, so it never hits this.

Fix: in `taffy_multicol`, when `arena.env().cell_spaces`, pass `cells::space(gap(s), cells::COLUMN)` (the gap is across). Add a kernel test that a block with `column-count` and `column-gap: 12px` lowers with gap 8 under the rule and 12 without it.

**2. `calc(0% + 14px)` is a point length that stays unrounded.** `Dimension::spaced` (`kernel/src/style.rs:207-211`) rounds only `Dimension::Points`. `calc()` of one px unit, `env()`, and viewport lengths become `Points` in `resolve` (`style.rs:145-162`) and are rounded. A sum keeps a zero percent (`kernel/src/style/calc/simplify.rs:116-117`), and the compact form of percent-plus-px is `Dimension::Calc` (`kernel/src/style/calc/mod.rs:246-248`), so `calc(0% + 14px)` is `Calc(0, 14)`. `resolve` returns that unchanged, and `floor_lp` (`kernel/src/style/floor.rs:40-41`) sends a calc handle. Taffy then uses 14 px, while `14px` on the same axis becomes 16. That misses condition (1): a calc whose used value is points on every basis. A calc with a real percentage is the stated limit and should stay.

Fix: in `spaced`, when the flag is on and the resolved form is `Calc(p, v)` with `p == 0.0`, return `Points(cells::space(v, cell))`. Test `padding-top: calc(0% + 14px)` → 16 under the rule, and `calc(10% + 14px)` still a percent calc.

**3. LLP 1101 §4’s row range drops 9 px.** `llp/1101-terminal-apps.rfc.md:458` says down, 8 px is 0 rows and 10–24 px is 1 row. `cells::space` (`kernel/src/style/cells.rs:35-40`) is nearest with ties toward zero: 8 → 0, and anything above 8 through 24 inclusive is 1 row. 9/16 = 0.5625, so 9 px is 1 row. The kernel test (`cells.rs:113-124`) jumps from 8 to 10, so it does not lock the boundary.

Fix: say “above 8 px through 24 px is 1 row”, and assert `space(9.0, ROW) == ROW`.

## Nit

**Condition (4)’s phrase is not the even-line a failure prints.** Behavior is in place: the `1ch` box shrinks to its content, and `scripts/scale-check.mjs:259-268` checks equal opposite edges and whole cells. The failure text at `scale-check.mjs:262` is still “not the same on opposite edges”. The comment at `252-255` and LLP 1121 §10 (`llp/1121-tamber.rfc.md:4746`) already say equal rows above and below.

Fix: put “equal rows above and below” in that failure string.

## Assessment

Conditions (2), (3), and (5) hold on the flex/grid path. `cell_spaces` defaults false (`kernel/src/style/env.rs:308`, `:354`). The only `set_cell_spaces(true)` is `host/terminal/src/host.rs:235`, before the tree exists. With the flag off, `spaced` and the gap closure return the original length, and this commit does not touch `schema.json`, so other hosts stay byte-identical and the digest does not move. I did not rebuild. `Env` is `PartialEq` (the new field participates), not `Hash`, and not on the wire. `set_env` keeps the flag via struct update.

`-0.0` and negatives are right: a tie returns `+0.0` (`cells.rs:37-38`, bits asserted at `:130`); `space(-24, ROW)` is `-16` (`:129`); margin goes through `to_lpa`, which keeps the sign. Padding is clamped at zero after rounding.

§10’s numbers agree once the two `md` values are kept apart. The base terminal column is 0, 8, 8, 16, 16 px (`kernel/tables/schema.json:5970-5974`), so sm and md are 0 rows down. The scale design overrides `space.md` to 12 px (`host/web-js/conformance/scale/app.tamber:9`); 12 px is 8 across (the 1.5 tie) and 16 down. The run line “md 8, 16” (`llp/1121-tamber.rfc.md:4751`) is that pair. lg/xl 16, 16, gaps 0, 8, 8, 16, 16 across, the clone’s 14/12/10 px as 1 row, and even 0/1/0/1 all match. The new §4 row is in the same table as the border row; it is not adjacent, and the text points at the border rule below.

Residual, not findings: nothing else on the terminal path recomputes a content box from unrounded padding, margin, or gap. `cover.rs:149-158` and `:194` write `floor_lp` of the authored padding back over the lowered style, but the terminal never sets a cover and Apple/Linux never set `cell_spaces`. Grouped margins, button `resolve_geometry`, and `scroll-padding` / `scroll-margin` are the same kind of split and are outside this host or outside the three properties. `set_cell_spaces` after nodes exist would not restyle plain px (`environment.rs:157-160` filters on `uses_env`, which ignores `Points`), same as `set_cell_borders`; the terminal calls it on an empty kernel.

Test gap: no test that an `env()` or viewport length that resolves to points is rounded, that a real-percent calc is left alone (10% padding is covered), or that a multicol gap is rounded.
