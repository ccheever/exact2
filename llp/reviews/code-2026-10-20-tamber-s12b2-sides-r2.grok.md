# Code review round 2: B2-levels per side (1d17d046d7), Grok

**RIGHT.** Round 1 is fixed. The mixed-axis hole and the light-side sentence are both in `1d17d046d7`.

`value_in` (`contract/cli/src/tamber_levels.rs:171`) matches the checker's `select` / `under_value` (`tamber/src/check.rs:911`, `:966`, `:1183`). The most specific holding `when` wins, the first on a tie. A `when` is dark-only when any condition is `under scheme=dark`, so a mixed `when` stays off the light side. A normal-only `when` that names more axes wins on the dark side as well. The same side flag is passed at every hop, for a params line, an app token, and a package token, up to 8 hops. A `light-dark()` or other expression is not a bare reference, so that line wins and the fold's two colours stay on it. A bare reference is followed with that side's flag. `levels.base` (`tamber_levels.rs:316`) is set only when every tuple on both sides reaches that one base.

The counterexample (`contract/cli/tests/it/tamber_levels.rs:588`) is red on the old pair arm. That arm returned `""`, the walk stopped on `Map.pin`, and Light was `app-param` instead of `package` `reading.pin`. `mixed_axis_whens_are_walked_per_side` (`:622`) locks a mixed `when` on an app params line (on `b`+`dim`, light is `ink` / `#111111ff` and dark is the literal / `#000000ff`; on `b`+`black` both sides name `ink`, `#111` and `#222`, because the line is `light-dark`) and a two-axis normal `when` outranking an under-`when` on a package token. Replacing `.any(under)` with `.all(under)` keeps the mixed `when` on Light and fails that assertion. The 1.15 clause (`docs/tamber-protocol.md:118`), "A param's levels" (`:569`), and §10 (`llp/1121-tamber.rfc.md:5569`) all say `{light, dark}` and this light rule, as a correction inside the open window.

`cargo test -p contract --test it tamber_levels::`: 10 passed.

No findings.
