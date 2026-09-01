# Node keys alias across kernel reset

**Status:** Closed
**Resolution:** Kernel reset now preserves arena generations so reused slots cannot alias old host or motion cache keys.
**Systems:** Kernel, Motion, Hosts
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1001 §3

`NodeKey` contains only slot and generation (`kernel/src/id.rs:12-19`). A fresh
arena starts generations at zero and its first allocation produces `(0, 1)`
(`kernel/src/arena.rs:259-286`). `Kernel::reset` replaces the arena with a
fresh one while bumping a separate incarnation that is not part of the key
(`kernel/src/kernel.rs:354-363`).

Deterministic reproduction: retain the first node's key, reset, then allocate
the first node in the new arena. `node_by_key` checks only slot/generation and
resolves the stale key to the new node. The current reset test checks the stale
key before any post-reset allocation, so it misses the alias.

Include the kernel incarnation in externally retained identity, or carry
generation state across reset so a pre-reset key can never resolve again. Test
the stale key after recreating nodes at the same slots, including host and
motion caches that retain keys.
