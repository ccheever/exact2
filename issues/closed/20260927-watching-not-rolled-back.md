# A refused commit keeps the device topics its failed answer watched, and drops the ones the standing answer still needs

**Status:** Closed
**Resolution:** Already implemented: watched topics roll back with refused commits; current announce regressions pass.
**Systems:** Runner (`runner/src/runner/settlement.rs`, `runner/src/runner/commit.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1016.002

The resource's `watching` topics are rewritten during the commit (`runner/src/runner/settlement.rs:498`, `commit.rs:807`), but the commit checkpoint (`commit.rs:31-45`) and `conclude` don't include them.

**Failure:**
1. A resource's standing answer watches topic A.
2. An action changes its arguments. The new answer watches B but fails shape validation (`settlement.rs:568`).
3. State and the resource value roll back, but `watching` now says B. An announcement of A no longer refreshes the A answer still on screen.

**Fix:** put `watching` in the checkpoint and restore it on refusal, or recompute it from the standing answer when a commit concludes.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: commit checkpoints restore watched topics on refusal; Contract runner tests reproduce and verify synchronous shape-failure rollback and failed-parse topic rollback.

Implemented in `runner/src/runner/commit.rs`. The regression in
`contract/cli/tests/it/announce.rs` first failed because announcing A produced no
commit. It now verifies that A refreshes the restored answer, B does not, and a
topic added by an asynchronous reply with an invalid shape is rolled back too.

Required verification passed: root build, tests (1,726 passed; 0 failed; 8 ignored),
Clippy with warnings denied, formatting, staged caps, and boot.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max. Verification: confirmed by reading.
