# The anti-rollback floor for a fresh binary is always seq 0

**Status:** Open
**Systems:** Delivery, update crate
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11, LLP 1030 D9, LLP 1030.001, `update/src/binary.rs`

`Embedded::seq` is documented as the anti-rollback floor for a client that has taken no update (`update/src/store.rs`). `Baked::from_compat` always writes `seq: 0`. `compat.json` has no seq to read (`contract/cli/src/compat.rs` `Compat::to_json`). `Store::check` then takes `floor = self.select().seq` — the selected entry, or `embedded.seq` on entry zero — and admits anything `>=` that. `record.seq` is written on `Check::Current` and never used as a floor.

A historically signed head still verifies (key rotation is a new cohort). Two production paths go wrong:

1. **Fresh install of a later binary on the same stream** (1030 D9: a host bug-fix binary keeps its streams). Entry zero, floor 0. Origin at seq 41 whose plan matches is `Current` and still leaves the floor at 0. A later check that sees seq 1 with a different plan stages it — a downgrade of the just-installed binary.
2. **Crash fallback to entry zero** drops the floor from the demoted bundle's seq back to 0. The `bad` list is eight *digests*, not seqs. Any other historical signed bundle is admissible.

Store tests use `EMBEEDED_SEQ = 3` as the floor and never drive `Baked`'s 0 against a lower published seq. `update/tests/publisher.rs` opens the production path with `seq: 0` and the fixture at seq 1.

Bake the stream seq of the embedded bundle into `compat.json` and read it in `from_compat`. Floor with `max(embedded.seq, select().seq, record.seq)`. After a `Current` that names a higher seq, that value must bind even while entry zero is selected. A deliberate rollback stays `seq + 1` pointing at older bytes (D11). Same-seq different-digest should refuse once a seq has been selected.
