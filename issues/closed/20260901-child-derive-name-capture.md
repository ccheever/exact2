# Child `derive` substitution is one-pass and can capture a parent name

**Status:** Closed
**Resolution:** Child derives now resolve within their component in dependency order without capturing same-named parent state.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017 P4c (`rename_component`)

Types infer derives to a fixpoint in any order. Inlining does not. Derives are subst'd in declaration order into `child_subst`, and `subst_expr` on an `Ident` clones the replacement *without* substituting inside it again (`contract/syntax/src/inline.rs`).

`derive b = a * 2` then `derive a = n + 1` leaves `b` as `a * 2` with a free `a`. If the parent has no `a`, the expanded root is `type-unknown-name` for a program the child type-checks. If the parent has state `a`, the child's `b` silently reads the parent — name capture, the thing `rename_component` exists to prevent. `rename_component` only rewrites `each`/`match` vars in the view, not derive names.

Fix: substitute derives to a fixpoint (or only allow a derive to mention earlier ones, and refuse the rest with a stable id). Recursively subst replacements. Fixture both orders, plus a parent slot whose name matches a later child derive.
