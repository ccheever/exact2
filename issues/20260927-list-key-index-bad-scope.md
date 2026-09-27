# `each item, i in items key=i` traps with `BadScope` inside `list` and virtualized lists

**Status:** Open
**Systems:** Runner (`runner/src/instance/window.rs`, `runner/src/instance/collection/mod.rs`), Contract lowering
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062 D8 (windowed lists name positions)

The compiler accepts an index-dependent key (`contract/lower/src/lib.rs:987`). The key-evaluation frames in windowed and virtualized lists fill only `Frame.item`, not the index (`runner/src/instance/window.rs:354-360`, `collection/mod.rs:389`), so `LoadIndex` traps.

**Reproduced:**
- Plain `each` renders.
- `list item-height=…` fails at boot with `Instance(Trap(BadScope{pc:0,depth:0}))`.
- A virtualized list fails the same way.

LLP 1062 D8 added the row's index but didn't update the key frames. Partial rekeying has the same omission.

**Fix:** put the logical index in every key-evaluation frame, and account for index-dependent keys on the reuse paths. Add a Contract test for each list kind.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max; reproduced by the verifier. Verification: reproduced.
