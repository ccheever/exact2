# Empty Contract file panics instead of a typed refusal

**Status:** Closed
**Resolution:** Compiling an empty Contract now returns a typed missing-component refusal.
**Systems:** Contract compiler
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1006 (`analyze-no-component`), LLP 0382 (fail closed loudly in debug)

`contract::compile("")` parses to a `File` with no components. `types::check` always calls `contract_syntax::expand`, and `expand` indexes `file.components[0]` (`contract/syntax/src/inline.rs`). `analyze-no-component` exists and is the specified refusal, but analyze never runs: types panics first.

The same panic hits a file that is only `shape` / `fn` / `style` / `test` with nothing merged in.

Fix: refuse an empty component list before expand (reuse `analyze-no-component`, or a types-pass id). Add a reject fixture. `compile("")` and `compile("shape S\n  a: number\n")` must return a typed error, never panic.
