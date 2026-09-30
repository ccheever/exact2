# Concurrent update-store owners can roll back the signed sequence floor

**Status:** Closed
**Resolution:** Fixed: a process-safe exclusive file lock is held before recovery/sweep and shared with active download snapshots; signed-floor, independent-process ownership and active-temporary regressions pass.
**Systems:** Update store, Native delivery
**Severity:** P1
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1026 D11; LLP 1030 D3a

Two `Store::open` calls for the same directory retain independent in-memory
records. No filesystem/process lock enforces the exclusive-owner assumption
in `update/src/store.rs:502`. `finish_check` rechecks against the current
object's record, not the latest disk record, and `write_record` replaces the
disk record unconditionally. `collect` then deletes entries absent from that
object's selected/pending/last-good/running/floor set (lines 1343 onward).

Reproduced with `Trust::Production`, a baked Ed25519 public key, and correctly
signed heads over `canonical_bytes`:

1. Start with a fresh store at embedded sequence 0.
2. Open owners A and B against the same directory before either checks.
3. Have A check a valid signed head at sequence 2: `Staged { seq: 2, ... }`.
4. Have B check a valid older signed head at sequence 1: unexpectedly
   `Staged { seq: 1, ... }`.
5. Open a fresh owner C: it selects sequence 1. B's collection removed the
   sequence-2 entry, so recovery from signed witnesses cannot restore the floor.

The probe uses sequential calls after the two opens; no narrow timing race
or signature forgery is needed. The precondition is overlapping owners, such
as two native app processes using the same app store directory. A replayed
older signed head can then undo an update this installation already accepted.
Boot marks and temporary-file sweeping also rely on the same unenforced
ownership assumption.

Enforce a single writable owner with a process-safe lifetime lock, or
serialize and refresh all record/collection mutations as transactions. An
atomic rename alone is not synchronization. Verify with two independent
owners that sequence 1 cannot replace accepted sequence 2, the higher signed
witness survives collection, and opening another owner cannot sweep an
active download's temporaries. Include production signatures in the regression.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Full signed probe:
`/tmp/exact2-review-20260924/game-probe/src/bin/update-review.rs`;
output: `/tmp/exact2-review-20260924/update-production-review.log`.
