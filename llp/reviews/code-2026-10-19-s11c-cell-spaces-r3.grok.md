# Code review: Stage 11 (c): the terminal's spaces in whole cells, laid out by the kernel (Env::cell_spaces), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `bd8de9c01a`.
- **Method:** one brief (sha256 `cf2588f17c7e5ce23e60b882d2f7717ae5d1564e636a7a41324ca9d8c59d6487`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; four should-fix (cover padding latent; scroll-padding/-margin off the grid; laid_margins' percent calc residue; §10's md label), nits
- **Disposition:** the four and three nits taken in 5d9b8565e6; the two-basis tree test, transform/region predicates and control measurement left (not on the terminal's path), said in the sign-off request

**Blocking:** none. On the paths the terminal host actually runs, conditions (1)–(5) hold. `Env::cell_spaces` defaults false, `set_env` keeps the existing flag (`kernel/src/kernel/environment.rs:81–88`), and the only production caller of `set_cell_spaces(true)` is `host/terminal/src/host.rs:235`. No schema, table, or digest change in the range. Conform 5/5 was not re-run; this review did not build.

## Should-fix

1. **`kernel/src/kernel/cover.rs:149–158` and `:194` overwrite the rounded padding.** `to_taffy` rounds, then `apply` / `without_inset` assign `style.padding_*.floor_lp` from the raw dimension. `BoxInset::of` (`kernel/src/id.rs:196`) publishes `(engine − cover).max(0)` as CSS padding, so `resolved_padding` follows the unrounded value. **Fix:** `style.padding_*.spaced(env, axis).floor_lp(env, added)` (top/bottom `ROW`, left/right `COLUMN`). The host’s added inset stays host px. With the flag off, `spaced` is the identity. Latent today: the terminal never calls `set_host_cover`. Same class, also latent: `kernel/src/grouped.rs:566–605` adds an unrounded platform delta onto already-spaced padding, and `authored_space` (`:705–713`) uses `at_basis` with no `spaced`. Only Apple reads `space_above`, and that host leaves the flag off.

2. **`runner/src/runner/collection.rs:430–445` and `:457–468` leave scroll-padding and scroll-margin off the grid.** Both resolve the raw style (`resolve` → points, else 0; `at_basis(env, 0.0)`). `into_view.rs:144–151` subtracts that scroll-margin from the cell-rounded margin, and `:188` insets the snapport by that scroll-padding. An authored 8, 10, or 12 px value puts a virtualized keyed jump off the row grid; the painter’s half-away round of a half-row offset then skips a row. Default 0, so the landed tests pass. These are scroll-* lengths, outside the named padding/margin/gap, so this does not fail condition (1). **Fix:** when `kernel.arena().env().cell_spaces`, run the resolved point length through `cells::space` on its axis; leave a real percentage unrounded (scroll-padding already drops non-points to 0). Then §10’s claim at `llp/1121-tamber.rfc.md:4740` that scroll and `scrollIntoView` need no change is true only for the zero default. Terminal `reveal_to` (`host/terminal/src/host.rs:802`) ignores both properties.

3. **`Kernel::laid_margins` (`kernel/src/kernel/geometry.rs:309–321`) returns the unrounded px residue of a percent calc.** The doc says a percentage is none. `spaced` leaves a `Calc` with a nonzero percent alone, and `at_basis(env, 0.0)` returns only that px term (14 from `calc(10% + 14px)`), which is neither 0 nor the laid-out value. **Fix:** after `spaced`, if the dimension still has a percent, return `0.0`, matching the doc and the old non-number path. Windowed vertical margin bindings are numbers ≥ 0, so the 8 px and 14 px jump tests stay valid.

4. **§10’s run line labels the wrong `md`.** `llp/1121-tamber.rfc.md:4738` correctly says the terminal column’s sm and md (8 px) are 0 rows and the clone’s 14, 12, and 10 px are 1 row. The run at `:4757` (“md 8, 16”) is the scale fixture’s design `space.md = 12px` after rounding (8 px across, 16 px / 1 row down). Across gaps `0, 8, 8, 16, 16` cannot tell 8 from 12. **Fix:** say that md in the run is the design’s 12 px.

## Nits

- **Placement.** The new LLP 1101 row is `llp/1101-terminal-apps.rfc.md:458`. The `cell_borders` row is `:466`. The prose says “beside the border rule below.” Move the row to sit against `:466`.
- **9 px is unasserted.** `kernel/src/style/cells.rs:139–149` jumps from 8 to 10. The formula gives 9 px one row (`(9/16 − 0.5).ceil() == 1`). Add `(9.0, 1.0)` and the across tie `space(4.0, COLUMN) == 0.0`.
- **`Dimension::spaced` (`cells.rs:58–61`)** treats a compare-tree as points when `value` at basis 0 equals `value` at basis `1e4`. A tree that agrees only at those two bases gets rounded. Walk a zero percent coefficient instead. Practical `0%` trees and `calc(0% + 14px + 0vw)` are covered (`:200–207`).
- **Eligibility checks read raw style.** `kernel/src/transform.rs:91–93` and `:114–127`, and `kernel/src/region/state.rs:898–899`, treat an authored 8 px margin or padding as nonzero while layout uses 0. Use `spaced` in those predicates. Transform is refused on the terminal; the region check only skips a fast path. The fallback layout is still the rounded one.
- **`control_measurement.rs:203–218`** hands unrounded button padding (and `image_gap` at `:199`) to the host measurer. The terminal measurer and button paint ignore them (fixed `+2` columns, 1-cell inset). Matters only if a `cell_spaces` host starts using those fields.
- **`scripts/scale-check.mjs:111`** `Math.sign(0) * …` can yield `-0` on a negative tie. `space()` returns `+0` (`cells.rs:41–42`, asserted at `:156`). Padding expectations are nonnegative, so the check does not fail.
- **`set_cell_spaces` is public**, same pattern as `set_cell_borders`. Any host can turn it on. A debug assert is optional.

## Look-fors that check out

- **Env.** `PartialEq` includes the new field, so a toggle restyles every live node and drops the layout memo (`environment.rs:136–137`). `Env` is not `Hash` and the field is not on the wire.
- **Negatives.** `space(-8, ROW)` is `+0`; `space(-24, ROW)` is `-16` (`cells.rs:155–156`). Margin keeps the sign (`:193`). Padding `floor_lp` clamps after rounding, so a negative padding’s used value is 0.
- **calc / env / viewport.** `resolve` turns env, region, viewport, and a basis-free compare into points, and those round. A real percent stays (`cells.rs:184`, `:194–195`). `kernel/tests/it/env.rs` covers `env()` plus `calc(env() + 12px)` rounding 12 → 16, and the flag turning back off.
- **Content boxes** (`resolved_padding`, field/svg/paint/resize rects, the content-height probe) read the engine’s used padding. They agree with layout whenever no cover rewrote it.
- **(4) and (5).** The even box shrinks to content; scale-check’s failure text includes “equal rows above and below.” `spaces_are_whole_cells_so_rows_follow_the_layout` paints gap-sm items 0, 1, 7, and 30 on the px row and reveal of item 30 shows 21–30. `a_keyed_jump_aligns_rows_by_their_margins_in_whole_cells` covers margins 8 and 14.
