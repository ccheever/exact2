# A Current head leaves no signed witness, so a lost record rolls the floor back

**Status:** Closed
**Systems:** Update store (`update/src/store.rs`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A signed head whose plan and asset roster match the embedded generation is accepted as Current and advances the anti-rollback floor, and no entry is written. If the record is then lost, the next open rebuilds the floor from the embedded sequence and from entries on disk. A correctly signed head older than the one this process already accepted, and newer than that rebuilt floor, is admitted.

**Cause.** `download` skips `write_entry` when `admit_check` reports the head is the embedded payload (`update/src/store.rs`, `download`). `finish_check` on that path writes only `record.seq` and `record.digest`. `write_boot_mark` writes the record unsynced and says a power loss loses only the mark, because `open` recovers the floor from signed entries. `signed_floor` reads `entries/` only. A damaged or empty `record.json` is `RecordFile::Fresh` (`read_record`), so the recovered floor never includes a Current head that was not stored as an entry.

**Reproduction.** A production store whose embedded sequence is 3. Authenticate a signed head at sequence 6 whose payload matches entry zero: `entries/` stays empty and the record's sequence becomes 6. A signed head at sequence 5 is rejected. Replace `record.json` with an empty file (the Fresh outcome of a truncated record) and reopen. Sequence 5 is staged.

**Fix.** When a Current head advances the floor, write a durable signed witness that `signed_floor` reads, independent of the boot record. A crash that loses `record.json` must still refuse every sequence below the highest witness.

**Acceptance.** The reproduction still rejects sequence 5 after the record is removed. A head that differs from entry zero still writes a full entry, and the existing rollback tests keep their results.

**Independent verification (Codex).** At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `/tmp/exact-review-trust-rust-repro.rs` ran against the existing `exact-update` rlib with real Ed25519 signatures. It accepted Current sequence 6 with zero stored witnesses, rejected sequence 5, wrote a boot mark, truncated the record, reopened, and then staged sequence 5. This simulates the damaged-record outcome used in `update/tests/it/durability.rs`; it does not induce a physical power loss. Relevant locations: `update/src/store.rs:800-803`, `818-828`, `1230-1245`. Closed issues `20260904-baked-seq-always-zero.md` and `20260924-update-concurrent-store-rollback.md` do not cover recovery of a Current-only floor.

**Resolution.** Current heads persist an authenticated envelope before advancing the record. Recovery includes these witnesses, and a witness can acquire payloads later without losing its signed floor.

**Verification after the fix.** All 54 update integration tests passed. Signed Current heads survive removed and truncated records, reject an older signed head, and materialize after a new binary changes its embedded payload.
