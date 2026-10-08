I'll review only the tip commit, starting with the project rules and the review notes it claims to fix.The prior review blocked on three length-clamp gaps. I'll inspect only HEAD and check whether those fixes hold.The commit is small. Next I’ll check the nonnegative rows against the schema and whether `Expr::floor` can claim a floor the expression can go below.The review is still open on the tip commit. I'll re-check the floor, the nonnegative rows, the decoder budget, and the digest against the code before writing the verdict.Nothing blocks landing. The three Astra findings are fixed. One should-fix remains on round-trip at the term cap.

## Should-fix

**`kernel/src/style/compare.rs:606` — a wrap can be stored whose written CSS no longer parses.** `at_least_zero` keeps the wrap when the stored tree plus two nodes still passes `well_formed` (64 stored nodes). `css()` then writes a `calc()` for every nonzero plus, and that `calc()` is two parser terms the store does not count.

On `width`, this is 64 parser terms and 22 stored nodes, so it is accepted and wrapped:

`min(calc(env(safe-area-inset-top) - 1px), …)` with that `calc` repeated 21 times.

The floor is −1, so the row holds `max(0px, min(calc(…), …))`. Parsing that text counts `max`, `0px`, and the same 64 inner terms (66) and returns the size error. The handle has no CSS text that comes back to it. `a_comparison_keeps_its_css_and_folds_what_reads_nothing` (`kernel/tests/it/compare.rs:167`) only covers short strings, which do round-trip.

Fix: parse the CSS of the wrap and refuse with `NONNEGATIVE` unless that parse returns the same handle. The “62 terms” wording counts stored nodes, so it describes trees whose canonical text has no folded `calc()`.

## Nit

**`kernel/src/wire/codec.rs:856` — the new budget assertion is one node loose.** `Expr::decode` (`compare.rs:213`) uses the same rule as the parser (`compare.rs:416`) and `well_formed` (`compare.rs:134`): increment, then refuse when `terms > 64`. A 64-node tree is kept; the 65th node is refused before its tag is read. The 64×64 tree stops at byte 319, inside the first inner `min`. `position() < 329` is also true for a cap of 65, which stops at byte 324. Fix: `assert_eq!(r.position(), 319)`.

## Checks

**Rows.** All 31 `codec: "dimension"` rows are accounted for. The match at `kernel/src/style.rs:476` holds these at zero: `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`, the four paddings, `flex_basis`, the four border radii, `r`, `rx`, `ry`, `column_width`. Margins, `top`/`right`/`bottom`/`left`, `cx`/`cy`, and `x`/`y` stay able to go negative, which is what CSS allows. Gap, border widths, `column-rule-width`, `shape-margin`, `stroke-width`, and `font-size` are `f32` rows, so this path never sees them.

**`Expr::floor` (`compare.rs:157`).** Sound when an inset is ≥ 0 and a viewport basis is ≥ 0, which is what LLP 1001 states and what `with_viewport` stores. A point term’s floor is its value. A viewport coefficient `n < 0` returns `None` (no finite floor); `n >= 0` floors at `plus`, the value at basis 0. `min` drops out if any argument has no floor, then adds `plus` once. `max` keeps the greatest finite child floor (an unbounded argument cannot pull the max down). `clamp` floors at `lo`’s floor: the value is `lo.max(val.min(hi))`, so it is `lo` when `lo > hi` and otherwise at least `lo`. `plus` is added once, at the same node `value()` adds it. A floor that is too low only adds a redundant `max(0px, …)`.

**Wrap, intern, web.** The intern key is the encoded bytes. `max(0px, expr)` floors at ≥ 0, so a second parse returns that handle instead of wrapping again. Below the cap, the written text parses back to it. Depth matches the message: an 8-deep tree that needs the wrap is refused; 7-deep fits. `host/web/src/css.rs:809` writes `c.css()`, so a wrap is `max(0px, …)` and a folded negative point is `0px`. The existing web snapshots `clamp(15px, env(…), 60px)` and `min(50vw, 300px)` floor at ≥ 0 and stay as written. A JS binding still emits its authored text; the browser clamps these properties.

**Unitless `0`.** No contract, test, or doc uses a bare `0` inside a CSS `min()`/`max()`/`clamp()` length. The kernel test that used to accept it now uses `0px`. Contract `max(0, n)` and grid `minmax(0, 1fr)` are different grammars and still parse. `0px` is not a valid `f64`, so `token_length` still accepts it.

**Digest.** `compare.rs` is in `CODEC_PATHS`, so the digest has to move. Recomputing `build.rs`’s hash (domain, prose-stripped canonical schema, production codec sources split at `#[cfg(test)]`) yields `0x1475_11c5_ed63_ffcb`, the snapshot at `kernel/src/wire/codec.rs:714`. The `codec.rs` edits are under `#[cfg(test)]` and do not enter the hash. No other file hardcodes a digest literal.

**Tests without the fix.** `a_size_or_padding_comparison_never_resolves_below_zero` fails: `min(-8px, -2px)` stays `Points(-8)`, and `min(-4px, env(safe-area-inset-top))` both keeps that CSS and resolves to −4 at inset 62. `max(0, 10px)` and `max(env(…) - 20px, 0)` parse instead of reporting `"zero too"`. The wide-tree test’s `position() < 329` fails because the first inner `min` is fully read (byte 333) before `well_formed` rejects it. The digest assertion fails if the constant is left at the old value.

Nothing blocks landing.
