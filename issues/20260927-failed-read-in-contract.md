# Contract reads a failed request with `failed(x)`, and a placeholder stays a placeholder after a failure

**Status:** Fixed: `failed(resource)` tracks request failure through Contract and incremental evaluation; failed placeholders retain provenance and stay out of checkpoint answers, verified by 14 placeholder/runner regressions and all required root checks.
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

Implemented in the Contract type checker, lowering and spelling hints; the plan
opcode schema and assembler; and the runner VM, settlement and view dependency
tracking. Like `pending`, `failed` takes a name rather than a value, so it is a
compiler intrinsic, not a stdlib value call. It accepts resources only.

Admission no longer promotes a failed placeholder. Document checkpoints also
exclude placeholders, including failed ones with no request pending. The existing
failed-argument marker still preserves standing-answer arguments, suppresses
store-triggered retries, clears on changed arguments or refresh, and rolls back
with a refused commit. LLP 1054.000.002 and Contract's reference now document the
2026-09-27 behavior.

Regression tests in `contract/cli/tests/it/placeholder.rs` cover source, implicit
zero and `empty(...)` placeholders; failure, changed arguments, refresh and success;
rollback and store writes; checkpoint/carry exclusion; and reads from derives,
resource arguments, keyed rows and conditional views in incremental and full
evaluation. Invalid arguments and spelling hints are covered there and in
`diagnostics.rs`. Before the fix, the placeholder-provenance assertion failed and
the compiler rejected `failed` as an unknown function. A second regression exposed
checkpoint promotion after pending cleared. All 14 focused tests now pass.

The initial test run also found two existing `set_place` calls in `strings.rs`
missing the optional clock argument; both now pass `None` so verification can run.
Nothing remains for Charlie's ruling.

Required verification passed on this worktree:

- `cargo build --all-targets --keep-going`: finished in 47.38s.
- `cargo test --lib --bins --tests --no-fail-fast`: 1,747 passed, 0 failed,
  8 ignored across 71 test binaries.
- `cargo clippy --all-targets --keep-going -- -D warnings`: finished in 19.89s.
- `cargo fmt --all -- --check`: exit 0, no output.
- `git add -A && bun scripts/caps.mjs`: all budgets within cap, 6 categories inspected.
- `bun scripts/boot.mjs`: 2 modules reachable before first pixel, 8 wasm references.

No host-specific code changed; the shared compiler/runner path is covered by the
Contract integration tests above.
