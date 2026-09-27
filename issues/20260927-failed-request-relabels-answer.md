# A failed request relabels the last value as the answer for the failed arguments, and promotes a placeholder to an answer

**Status:** Open
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

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max. Verification: the relabel and the promotion are confirmed by reading; the re-ask loop is plausible.
