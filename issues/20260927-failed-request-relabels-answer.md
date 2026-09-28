# A failed request relabels the last value as the answer for the failed arguments, and promotes a placeholder to an answer

**Status:** Fixed: failed arguments have a separate rollback-safe marker, the standing answer keeps its own arguments, and store changes don't retry a failure (Contract runner regressions). Charlie's 2026-09-27 ruling (LLP 1054.000.002) is built in `issues/20260927-failed-read-in-contract.md`: `failed(x)` is readable, and a placeholder stays a placeholder after a failure.
**Systems:** Runner (`runner/src/runner/admission.rs` `release_failed`, `runner/src/runner/settlement.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1054.000.002 D4 (placeholders), LLP 1016 D5; decided at merge a5f03a59

When a request fails, `release_failed` sets `state.args = failed_args` and `state.placeholder = false` (`runner/src/runner/admission.rs:99-102`). Settlement then reuses the value because `s.args == args` (`settlement.rs:392-409`). The argument relabel is Seth's `f05a95c0`. The placeholder promotion was written while resolving merge `a5f03a59` against LLP 1054.000.002.

**Failures:**
- Search results for "Menl" show under the query "Menlo Park" with `pending` false, and nothing the view can read says the request failed.
- A declared placeholder (`else empty(…)`) becomes "the answer", so "no results" and "the request failed" look identical.
- For a store-reading resource, reuse also requires an unchanged store revision (`settlement.rs:403-404`). If the store moved, the resource is asked again, so a source that fails every time can loop. This case is plausible but has not been run.

**Fix (needs Charlie's ruling):**
- Keep the standing value's arguments, and hold a per-resource "failed for these arguments" marker that settlement consults. Don't re-ask until the arguments change or a `refresh`.
- Let Contract read the failure, for example `failed(x)` beside `pending(x)`.
- Keep a placeholder a placeholder.

The design-neutral part is implemented in `runner/src/runner.rs` and
`runner/src/runner/{admission,commit,settlement}.rs`. Settlement keeps evaluated
query arguments separately from the standing answer's provenance, including
while a newer request is pending. The failure marker suppresses re-asks across
store revisions, clears on changed arguments or refresh, and restores on refusal.
Successful answers clear it. `contract/cli/tests/it/placeholder.rs` reproduced
both argument relabeling and a re-ask after a store write; both now pass, with
coverage for retry, success, rollback and full evaluation.

Current placeholder promotion is deliberately retained: a failed placeholder is
still marked non-placeholder and shown with `pending` false. A regression records
that behavior without endorsing it; no Contract-readable failure state is added.

Required verification passed: root build, tests (1,726 passed; 0 failed; 8 ignored),
Clippy with warnings denied, formatting, staged caps, and boot.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max. Verification: the relabel and the promotion are confirmed by reading; the re-ask loop is plausible.
