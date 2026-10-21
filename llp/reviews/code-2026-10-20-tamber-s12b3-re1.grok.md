# Narrow re-check after round 3: LLP 1121 stage 12 B3 (dcea053f2b), Grok

**RIGHT.** Round 3's three findings are fixed, and ordinary shadow folding is unchanged.

`scale::is_length_item` is the shared predicate: a `px` length, the exact unitless `0`, a bare base name, or `calc`/`min`/`max`/`clamp` in any case. `fold_lengths` folds only those items, and only when they name a base. `base_scale::shadow` dropped its `named_in` test and classifies both the authored words and the row's CSS with that predicate. `-exact-platform-color(ios named:space.md, …)` fails it, so the inserted box-shadow spread and text-shadow blur stay the row's `0px`, and the colour stays `BoxShadows`/`TextShadow` `css()` with no `var()`. The new web-js check locks both kinds.

QUEUE's per-column entry now matches theme2 and `tamber_base.rs`: each residual is null in `lengths`, and `length_texts` (only then) lists every length's text, px included. The 1.15 clause already said this.

`shadow::looks` counts a length function only when `measures` finds a dimension or a name that is a length token, dial, named length, or unset base, at any depth. The CSS parser lowercases function names. `n = min(1, 2) max(3, 4) calc(5)` therefore misses the sniff and takes the earlier colour fold. `calc(space.md * 2) calc(space.sm) black` still has two length functions and gets "write its kind", as does `0 max(1px, 2px) calc(2px * 3) black`.

For a parenthesized item the predicate is the previous `fold_lengths` test; a bare base still folds; any other function is still copied through, so kernel `measure_at` is unchanged on those texts. Px-only shadows never enter the JS rewrite. The number-function sniff returns to the refusal main already had, which is what the 43-app plan compare claims. That compare was not re-run.
