# Code review round 1: B2-levels per side (e33c12527e), Grok

**RIGHT.**

The per-side walk matches the checker's `select` / `under_value`. Light drops any `when` that names an `under scheme=dark` axis, then takes the most specific holding `when` (first on a tie), else the line's own text. Dark keeps every holding `when` and uses the same specificity rule, so a longer normal-only `when` beats a shorter under-`when`, and a mixed `when` (under axis plus a normal one) is dark-only. A bare reference is followed with that same side flag, including through params, app tokens, and package tokens, up to 8 hops. `light-dark(...)` stays an expression and stops on that line, as `color-mix` does. `levels.base` (`tamber_levels.rs:316`) is set only when every tuple on both sides reaches that one base. The 1.15 clause, the "A param's levels" section, and the §10 follow-up (`llp/1121-tamber.rfc.md:5569`) all say `{light, dark}` and that this is a correction inside the open window.

The new counterexample fails on the old pair arm: that arm returned `""`, the walk stopped on `Map.pin`, and Light was `app-param` instead of `package` `reading.pin`.

1. **NIT** `contract/cli/tests/it/tamber_levels.rs:588` — no mixed-axis case, and none where a normal-only `when` is strictly more specific than an under-`when`. `value_in` (`tamber_levels.rs:187`) does the right thing; nothing locks it. Repro: `dark-style = black | dim under scheme=dark`, `emphasis = quiet | loud`, `ink = accent.soft`, `when emphasis=loud: brand`, `when dark-style=dim emphasis=loud: #111111`. On dim+loud, light must be `brand` and dark `#111111`. On a tuple where `when emphasis=loud size=lg: brand` (two normal axes) outranks `when dark-style=dim: #111`, both sides must be `brand`.
