# Preserve failures for every skipped conditional async check

**Status:** Open
**Systems:** async lane, failure attribution
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** scripts/async.mjs:243–245

A failing iOS touch check disappears from the lane's persistent failure set on the next commit that does not touch host/apple. If the same failure returns on a later Apple commit, it is misclassified as new.

In `once`, when `result.checks.ios` is absent, only prior failures beginning `ios: ` are carried forward. The conditional `ios-touch` check is also skipped, but its `ios-touch: ` failures are dropped before `state.failures` is written. Both checks share the same trigger.

Reproduction by following the current state transition: start with `state.failures = ["ios-touch: existing failure"]`; process a result with neither conditional check and no new failures. The filter preserves nothing, leaving an empty baseline. The next identical ios-touch failure is then “fresh.”

Carry forward failures for each check that was not executed, rather than special-casing the ios prefix. Keep an executed passing check able to clear its own failures.

Acceptance: failure → skipped commit → same failure does not create another issue or clear the outstanding failure; failure → executed passing check does clear it. Cover both iOS checks and any other conditional/tier check using the same state mechanism.
