# `mutation … then action` runs once for several answers, and a `then` that re-sends its own mutation spins the host at the fire limit

**Status:** Open
**Systems:** Runner (`runner/src/runner/commit.rs`, `runner/src/runner.rs`), Contract analyzer
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1016.001 ("after each answer"), design question: per-answer vs latest

`arm_then` stores a single due time per mutation, `then_due[m] = now_ms` (`runner/src/runner/commit.rs:323`), and fires a parameterless action against the current slot (`:199`).

**Coalescing (reproduced).** Two `quick` sends answered before the host advances produce one `then` commit (`landings=1`). It reads the latest answer, although LLP 1016.001 promises "after each answer".

**Spin (reproduced).** A `then` action that sends its own mutation compiles. When the mutation answers at once, each `advance(0)` returns `TimerFireLimit{4096}` with `due=Some(0.0)` again, and landings climb 4096, 8192, 12288. A host that advances at the reported due time spins at 100% CPU, and `clock settle` never settles.

**Fix:**
- Rule on the semantics: either carry each completion (with its answer) in a queue, or amend the LLP to a coalesced reaction to the latest value.
- Have the analyzer refuse a `then` action that can send its own mutation, or have the runner not re-arm a `then` inside the commit its own action caused.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max (code and design), Opus 5.5 max. Verification: both reproduced with a runner probe. The design review also flagged that a dev reload drops an armed `then`; that is declared in LLP 1016.001 D3 and is not a bug.
