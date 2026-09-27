# Contract reads a failed request with `failed(x)`, and a placeholder stays a placeholder after a failure

**Status:** Open
**Systems:** Contract (analyzer, lowering, roster), plan, runner (`admission.rs`, `settlement.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1054.000.002 (the 2026-09-27 ruling), `issues/20260927-failed-request-relabels-answer.md` (the design-neutral half, built)

The 2026-09-27 fix keeps the standing value's arguments and marks the failed arguments. What's left is Charlie's ruling:
- `failed(x)` beside `pending(x)`: true while the resource's latest request for its current arguments failed, and false again once an answer lands or a new request goes out.
- A placeholder that stood in when the request failed stays a placeholder, marked failed. Remove the `state.placeholder = false` that merge a5f03a59 added.
- Document it in LLP 1054.000.002 and Contract's reference.
- Tests: a failed search shows the placeholder with `failed` true and `pending` false. Changing the arguments clears `failed`. A `refresh` retries.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
