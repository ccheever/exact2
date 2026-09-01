# Duplicate declarations silently overwrite source

**Status:** Closed
**Resolution:** The parser now rejects duplicate declarations and singleton sections with stable diagnostics instead of overwriting them.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1006

Several source declarations are silently last-write-wins instead of unique:
repeated `props`/`inject` blocks, repeated component `view`, duplicate match
arms, multiple `every` lines in one task, duplicate shape fields, duplicate
component names, and duplicate function/action parameter names. The parser
assigns or inserts over prior values (`contract/syntax/src/parser.rs:636-686,
821-919,1002-1017`), and the type pass does not consistently recover the
collision.

Compiler probes for duplicate components, shape fields, views, and action
parameters all emitted plans. Refuse duplicates at the second declaration with
one stable diagnostic family, preserving both source locations where useful.
Add a reject fixture for every declaration namespace and cardinality rule.
