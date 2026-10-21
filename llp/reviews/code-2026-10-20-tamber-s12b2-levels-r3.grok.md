# Review round 3, LLP 1121 stage 12 B2-levels, Grok (grok 4.7, xhigh)

**RIGHT.** Rounds 1 and 2 are fixed in the walker, the wire, and the server tests. `cargo test -p contract --test it tamber_levels` passed, 8 tests, 0 failed.

The walk matches the checker: app `params`, then an app token or icon (either role spelling, any case, published as the row spelling), then a package token, then a base name, then a role; literals and expressions stop; 8 hops; an `under scheme=dark` winner stays on the owning line. `levels` is present only for a bare-reference default and is JSON null otherwise. `levels.base` is set only when every tuple reaches that same base. `resolve` tuples get `level` and `name`; there is no `winning` field. The `tamber/1.15` note is one clause on the existing bullet. Changed sources are under 1499 lines (`tamber_levels.rs` 345, `tamber_base.rs` 482, `tamber_params.rs` 402, `lib.rs` 1356).

1. **NIT** `contract/cli/src/lib.rs:47` — the stage-4 `/// uses` comment now documents `tamber_levels` (it was inserted under that comment). The module file already has the right `//!`, and `tamber_uses` has its own. Delete that `///` line.
