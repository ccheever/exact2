# `contract/types/src/lib.rs` is one edit from a red caps check

**Status:** Closed
**Resolution:** Contract type checking was split into a focused checks module and is back below the source line cap.
**Systems:** Contract compiler, Tooling
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** rules/RULES.md (1,500 lines per source file)

`contract/types/src/lib.rs` is 1,499 lines. `node scripts/caps.mjs` fails the workspace if any source file exceeds 1,500. This file cannot take a bugfix, a diagnostic, or a split of `check_view` without first moving code.

Nearby and climbing: `runner/src/runner.rs` (1,492), `contract/lower/src/lib.rs` (1,379), `contract/syntax/src/parser.rs` (1,358), the two Apple `Presenter.swift` files (1,049 / 1,102).

Fix: split `check_view` / `check_stmts` / `refine_params_from_view` out of `types/src/lib.rs` before the next types change. Treat parser, lower, and `runner.rs` as next-to-split, not as room to grow. The Apple presenters are the same cliff on the host side (see LLP 1025).
