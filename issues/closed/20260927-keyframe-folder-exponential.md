# The keyframe constant folder bounds depth but not work, so nested palette functions compile in exponential time

**Status:** Closed
**Resolution:** Already fixed: a shared 4096-visit fold budget bounds branching calls; current lowerer work-bound and compiler keyframe/VM parity regressions pass.
**Systems:** Contract lowering (`contract/lower/src/keyframes.rs`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1055 (keyframes addendum)

`FOLD_DEPTH` (16) limits recursion depth, not the number of folds, and there is no memo (`contract/lower/src/keyframes.rs:33-106`, `:72`). A palette function that calls the next one twice per level folds 2^16 times; three calls per level is about 43M folds, and the compile hangs.

The folder is also a second evaluator for Contract expressions: `==` on `-0.0`, `%`, and number printing in templates are implemented again, with no test against the VM.

**Fix:** memoize by `(fn, args)` or cap the total number of folds, and test the folder against the VM's operators.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: cap each constant fold at 4,096 expression visits shared across calls; branching-palette and compiler-diagnostic regressions pass, with VM parity tests for operators and corrected decimal/exponent template formatting.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max. Verification: confirmed by reading; not run (it needs a contrived chain).
