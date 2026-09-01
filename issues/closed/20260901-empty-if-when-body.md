# `if` / `when` with no indented body parse as empty, and the next sibling always runs

**Status:** Closed
**Resolution:** If and when constructs now require non-empty bodies and reject empty branches during parsing.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01

`block()` returns `Ok(vec![])` when the next token is not `Indent` (`contract/syntax/src/parser.rs`). `if k == "Enter"\n  n = 1` at the same indent as `if` is an empty `then` plus an assignment that always runs. The same for `when on` followed by a sibling `text`. That is accepting a misleading program, not a syntax error. `match` is safer because missing arms are `contract-match-arms`.

Fix: require a non-empty indented block for `if`/`when` (and for `else`). Reject with `syntax-empty-block` at the construct's span. Add a reject fixture.
