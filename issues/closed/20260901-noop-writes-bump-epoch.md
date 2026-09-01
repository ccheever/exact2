# No op writes still advance kernel epoch

**Status:** Closed
**Resolution:** Identical prop, style, clear-style, and children writes no longer dirty nodes or advance the kernel epoch.
**Systems:** Kernel
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §3

`SetProp`, `SetStyle`, and `SetChildren` mark nodes touched/dirty without
comparing the old and new value (`kernel/src/txn.rs:390-455`). The kernel bumps
its epoch whenever the resulting receipt sets are nonempty
(`kernel/src/kernel.rs:140-158`).

Reapplying identical state therefore counts as a commit and can trigger
layout, mirror, and motion work, contrary to LLP 1001's statement that an
epoch advances only for a commit that changed something.

Suppress unchanged receipt entries (while preserving validation), and test
identical property, style, and children batches for stable epoch and empty
change sets.
