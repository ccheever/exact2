# Text measurer can publish nonfinite geometry

**Status:** Closed
**Resolution:** Invalid text metrics now fail layout atomically and a later valid measurement can rebuild successfully.
**Systems:** Kernel, Hosts
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §5

`TextMeasurer` is a public injected host boundary returning arbitrary
`TextMetrics` (`kernel/src/text.rs:97-111`). Layout passes width, height, and
baseline directly to Taffy without checking finiteness or sign
(`kernel/src/layout.rs:184-212`). A buggy native callback can therefore
publish NaN, infinity, or negative geometry despite the kernel's fail-closed
frame invariant.

Validate host metrics at the boundary and return a typed `LayoutError` while
leaving the prior frames untouched. Cover every metric field with NaN,
infinity, and negative test doubles. Coordinate the error vocabulary with the
existing non-finite layout-offer issue.
