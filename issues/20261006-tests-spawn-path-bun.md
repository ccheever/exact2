# Rust tests spawn PATH bun instead of the pinned one

**Status:** Open
**Systems:** verification, bake, host/web
**Severity:** P3
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** package.json packageManager; scripts/app.mjs pin check

`package.json` pins Bun 1.4.2. `scripts/app.mjs` refuses an older Bun and names the install. On this machine `PATH` finds `~/.bun-1.3.12/bin/bun` first and `~/.bun/bin/bun` is 1.4.2.

Rust tests that shell out do not all use that pin. `bake/src/lib.rs` and `canvas/tests/cases.rs` honor `$BUN` and otherwise spawn `bun`. `host/web/tests/it/*.rs`, `update/tests/it/publisher.rs`, and `semantics/difftest/src/js.rs` use `Command::new("bun")` with no `$BUN`. A root `cargo test` then fails in `contract` (delivery), `exact-bake`, `exact-js` (`pure_utilities_match_the_web_executor`), and `exact-web-js` paint tests by printing the pin error from `app.mjs:317`. Those failures are the runner, not the assertion.

Resolve the pinned binary the way `app.mjs` already describes (or one `$BUN` lookup shared by the tests) so a machine with both versions installed tests the product.
