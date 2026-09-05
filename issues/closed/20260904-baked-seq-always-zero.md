# The anti-rollback floor for a fresh binary is always seq 0

**Status:** Closed
**Resolution:** Fixed by authenticated embedded release metadata and the maximum embedded, selected, and accepted-record floor. A strict production seq-41 native bake now proves Current/equivocation/restart behavior, two failed seq-43 boots followed by durable next-open demotion with its floor retained, and deliberate rollback to older bytes at higher seq 45.
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


## Verification (2026-09-05)

The implementation already on main (`2a57836`, `c8d514d`, and the native launch
review fixes through `b794220`) satisfies the ticket; this closure adds the
previously missing production proof against `29b45be`, with no code change.

An isolated copy of current Caltrain used the actual `exact-linux-update`
composition, target `aarch64-apple-darwin`, and its real data-source grants.
An explicit production genesis bake supplied the plan and complete asset roster
to `publishStream`, which emitted an Ed25519-signed sequence-41 publisher
receipt. Rebuilding with `EXACT_UPDATE_TRUST=production` and that
`EXACT_UPDATE_RECEIPT`, with genesis unset, produced a native binary whose
`--exact-receipt` matched both the bake output and signed canonical entry digest.
The runtime used the CPU painter and stdio agent; a loopback origin override
changed transport only, while the baked production verification key remained
authoritative.

Thirteen separate native launches proved:

- A fresh install reports embedded/running sequence 41 and refuses head 40.
- `Current` at 42 persists the accepted canonical digest while entry zero
  runs. Same-sequence authenticated equivocation is refused without changing
  the record; sequence 41 remains refused after restart.
- A signed, undecodable sequence-43 plan is selected whole. Two actual boot
  refusals persist failures 1 and 2. The **next open** clears selection and
  failures, durably records 43's digest as bad, and retains accepted floor 43
  and its digest; the fallback refuses a sequence-42 head.
- Valid sequence 44 reaches first pixel and becomes last good. Sequence 45
  then stages and boots the exact older sequence-41 plan and asset roster,
  proving an intentional rollback uses a higher sequence. Head 44 is refused
  afterward.

Focused `exact-update` and `exact-linux-update` tests passed with explicit
development trust for Cargo validation. The strict production drive needed no
implementation correction in the newly authorized verification round. Its
receipts, native binary, per-launch records, and logs are retained in the ignored
`target/llp-ship/20260905-production-sequence/` directory; `validation.json` binds
the binary hash, receipt digest, and runtime results. Every owned process exited.
