# Code review round 1: per-column shadow values (b7f366d7ed), Astra

RIGHT

No BLOCKING, SHOULD-FIX, or NIT findings.

- Runtime folding agrees: [`shadow_text`](/tmp/rv-shcols-astra/contract/cli/src/tamber_base.rs:73) uses the kernel’s `fold_lengths` and scale table. App-defined bases are resolved during Tamber folding; residuals retain only unset bases. Package designs explicitly cannot define base names, so they introduce no missed override.
- Per-shadow numbers follow the same substitution and fold as their text. Numeric zero is normalized; text normalization follows Tamber’s serializer and the kernel’s calculation narrowing.
- Explain selects the first tuple choosing each default/when line. Resolve handles every tuple independently. Text shadows, inset, mixed lists, platform colours containing base names, absent keys, and terminal exclusion are handled consistently.
- D1’s length path and other non-shadow answer paths are unchanged. The 1.15 clause, documented terminal exception, §10 note, and shadow QUEUE removal are present.
- Tests construct expected shadow text separately, using `scale::evaluate` for expected numbers as the plan requires. They are independent of the new wire helpers, though intentionally share the kernel evaluator. The positive column assertions would fail without this implementation.

Read-only review; no builds or tests run. `git diff --check` passed.
