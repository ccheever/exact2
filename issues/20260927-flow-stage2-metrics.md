# `metrics.mjs` reports a still layout's cost and a multi-shape worst case for text-around-shapes Stage 2

**Status:** Open
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
