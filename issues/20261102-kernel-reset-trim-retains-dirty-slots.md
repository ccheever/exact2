# A trimmed kernel can panic on its first layout after reset

**Status:** Closed
**Severity:** P3
**Systems:** kernel/src/arena.rs; kernel/src/layout/publication.rs
**Author:** Codex
**Date:** 2026-11-02

Reviewed source revision: `5b94c216a6fee8fca42026f5cb2e9a80063da03a`.

`NodeArena::reset` clears node columns and sparse derived state but leaves `layout_dirty` intact (`kernel/src/arena.rs:267-313`). A subsequent `Kernel::trim` truncates the now-free columns (`arena.rs:332-370`). The next layout's publication walks the retained dirty slots and reads their parents without checking whether they remain live or allocated (`kernel/src/layout/publication.rs:27-38`). A valid sequence of public APIs therefore panics instead of producing a fresh layout.

Executed `/tmp/exact-kernel-reset-trim-probe.rs` against the checkout's warm kernel artifact:

1. Apply 500 View nodes, attach their root, and leave the first layout pending.
2. Call `reset()`, then `trim()`.
3. Apply a new single View root and compute its layout at 200×100.

The new arena contains one slot. The process exits101 at `kernel/src/arena.rs:404` with `index out of bounds: the len is 1 but the index is 1`. No malformed wire input, host callback, network or application process is involved. This demonstrates the public kernel lifecycle; a current production caller using this exact reset/trim sequence was not established.

Clear `layout_dirty` during reset along with the other per-node derived state. Cover a reset with unpublished dirty nodes, a trim, and a smaller replacement tree.

Deduplication: searched current issues and both local `exact2/issues` and `exact/issues`, including closed tickets; no matching reset/trim dirty-slot issue was found.

## Resolution

Arena reset clears unpublished layout-dirty slots before trim can remove their columns.

## Verification

`reset_discards_unpublished_layout_before_trim` passed: 500 dirty nodes are reset and trimmed, then a one-node replacement lays out successfully. The kernel library and integration suites passed.
