# Large clock seeks can hang forever

**Status:** Closed
**Resolution:** Clock seeks now have an exact safe-integer domain and a bounded resumable commit budget.
**Systems:** Runner, Agent API
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005, LLP 1012

`Runner::advance_timed` accepts every finite target and fires one commit per
due timer interval with no work bound (`runner/src/runner.rs:802-858`). A seek
to `f64::MAX` can therefore attempt effectively unbounded work and grow an
unbounded receipt vector.

There is also a deterministic non-progress case. `Carried::now_ms` is public
and unvalidated; at `2^53`, adding a 1 ms interval rounds back to the same
`f64`. The timer remains due and the loop fires the same timestamp forever.

Define the admissible clock domain and a per-call timer-fire budget. Detect
`next_ms <= previous_next_ms` before committing, return a typed refusal rather
than hanging, and bound or chunk catch-up work consistently across direct
runner calls and `clock settle` on every host.
