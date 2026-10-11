# Bound update-store reads before allocating corrupt local files

**Status:** Open
**Systems:** update store, launch recovery, graceful overload
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1026 D11; update/src/store/disk.rs; update/src/store.rs

Recovery validates corrupt files only after reading their entire contents into memory. A bad local blob or envelope can therefore exhaust memory while the app is trying to refuse it or fall back to the embedded generation.

`disk::read_card` at line 51 calls `std::fs::read`, then compares its length against the signed card. `Store::read_envelope` at line 1143 also reads the whole file before `Envelope::parse` checks the 64 KiB envelope limit. `read_record` uses an unbounded read_to_string. Network response bounds do not protect these local recovery paths.

Executed the unchanged disk module with an instrumented allocator and a sparse 4 MiB regular file whose card declared one byte. The routine refused the mismatch, but its largest allocation was 4,194,304 bytes. The hash stub was never reached; the measured allocation comes from the real read/size-check path. An arbitrarily enlarged file scales that allocation.

Open and validate a regular file once, reject unreasonable/incorrect sizes before allocation, and use bounded reads so concurrent growth cannot bypass the bound. Limit envelopes before parsing and define a practical bounded record encoding. Preserve crash recovery and signed-digest checks.

Acceptance: oversized blob, envelope and record fixtures are refused/recovered with bounded memory; file growth between metadata and read is also bounded. A refused candidate leaves the embedded/last-good generation usable and the rollback floor intact.
