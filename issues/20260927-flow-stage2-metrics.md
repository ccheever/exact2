# `metrics.mjs` reports a still layout's cost and a multi-shape worst case for text-around-shapes Stage 2

**Status:** Fixed: metrics reports repeated-run Stage 2 timings and actual pass/measurement counts; 9 auto-flow tests and the metrics fixture pass, with medians recorded in LLP 1043.000 §8. Required root build/test/Clippy remain blocked by unrelated pre-existing set_place test errors (below).
**Systems:** `scripts/metrics.mjs`, kernel (`kernel/src/flow.rs`, `LayoutTree::settle_flow`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1043.000 (the 2026-09-27 ruling: Stage 2 is accepted on these numbers)

Charlie accepted Stage 2's bounded re-layout on condition that the cost is measured. Add to `metrics.mjs`:
- the cost of a still layout of an admitted page (the LLP claims one comparison, with no measurement);
- the pass count and time for a page with several shapes and flowed leaves, in the worst admitted arrangement (the bound is leaves + exclusions + 2).

Record the numbers in LLP 1043.000 §8.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

## Fix and verification

`LayoutReceipt` now reports extra flow layout passes and comparison sweeps.
The existing metrics binary and `scripts/metrics.mjs` report fresh, still,
moved-shape and flow-disabled layouts for 8/32-leaf admitted pages, with raw
samples, text measurement counts and the conservative pass bound. LLP 1043.000
§8 records the repeated-run medians and the workload's limits. No layout
algorithm or host behavior changed; no new script or check was added.

Reproduction: the still-layout regression could not compile its counter
assertions before the receipt exposed those counts (`E0609`). Afterward,
all 9 auto-flow tests pass, including the new serial-propagation regression:
8/32 leaves require exactly 8/32 extra passes, measured height equals painted
height, still layouts need one comparison and no remeasurement, and removing
exclusions restores the zero-comparison fast path. The metrics-binary test
also exercises its real fixtures and cached-layout assertions.

Required root checks were run. `cargo build --all-targets --keep-going`,
`cargo test --lib --bins --tests --no-fail-fast`, and
`cargo clippy --all-targets --keep-going -- -D warnings` are blocked by the
same pre-existing `E0061` errors: `contract/cli/tests/it/strings.rs:221,223`
call `Runner::set_place` without the third `Option<f64>` argument. Both the
test file and `runner/src/runner/time.rs` are unchanged from `0c1b4308`;
fixing that separate API/test mismatch is outside this ticket.

- Kernel tests: 402 passed, 0 failed, 2 ignored.
- Metrics binary: 1 passed, 0 failed.
- Focused kernel/metrics Clippy with `-D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `git add -A && bun scripts/caps.mjs`: passed (all 6 budget categories).
- `bun scripts/boot.mjs`: passed (2 pre-pixel modules, 8 wasm references).

Remaining for Charlie: assess the measured cost against Stage 2's acceptance
condition; no new design ruling is needed to collect these numbers. The
unrelated root-check compile blocker still needs its owning lane's fix.
