# Template `${…}` closes on the first `}`, so inline `match` is rejected

**Status:** Closed
**Resolution:** Template parsing now balances nested expression braces and quoted braces before closing interpolation.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01

The grammar allows inline `match s { case some(x) => a, case none => b }` in any expression, including template parts. `template` (`contract/syntax/src/parser.rs`) does `after.find('}')`, so `` `${match x { case some(y) => y, case none => 0 }}` `` takes the match's `}` as the end of `${`. A valid program is refused (`syntax-template-expr` / `syntax-expected`). Nested `}` in a string in `${…}` has the same hole.

Fix: parse `${` with a brace depth (strings/templates nested) or reuse the expression parser until a matching `}` at depth 0. Fixture: a template whose part is an inline `match`.
