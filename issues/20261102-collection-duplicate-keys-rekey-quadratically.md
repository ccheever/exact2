# Repeated collection keys stall even a virtualized list

**Status:** Closed
**Severity:** P2
**Systems:** `runner/src/instance/collection/rekey.rs`
**Author:** Codex
**Date:** 2026-11-02
**Commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

A virtualized list whose data contains many copies of one key takes seconds to open, although only sixteen rows mount. Repeated keys are meant to receive separate identities and a diagnostic; their recovery path instead blocks the runner with quadratic work.

`Collection::rekey_full` starts `dup` at zero for every row, then probes `d1:<key>`, `d2:<key>`, and so on from the beginning (`runner/src/instance/collection/rekey.rs:89-102`). For N equal keys, it performs about N²/2 hash lookups and allocates a fresh identity string for each probe. Row virtualization does not bound this scan because the complete logical data set must be keyed.

Reproduced on this Mac with `/tmp/exact-runner-duplicate-key-probe.rs`, linked to this checkout's existing `target/debug/deps` runner, kernel and plan artifacts. The probe boots a plan with a virtualized list, numeric rows keyed by their value, and a constant text row. Unique and repeated cases have the same number of logical rows and exactly 34 kernel views. Plan construction is outside the measured interval.

| Logical rows | Unique keys, boot ms | One repeated key, boot ms |
| ---: | ---: | ---: |
| 1,024 | 1.74 | 88.17 |
| 2,048 | 2.58 | 356.19 |
| 4,096 | 4.24 | 1,444.90 |
| 8,192 | 7.64 | 6,198.60 |

These are single local runner boot measurements, not phone or release-build latency claims. The fourfold growth on doubling the repeated data matches the source's nested search. No network, platform app or runtime process was started.

Keep the next duplicate ordinal per canonical key while scanning, or group equal keys and assign ordinals in one pass, as eager regions already do. Preserve identity order, diagnostics and row state; validate scaling with a fixed mounted-row count.

Deduplication: searched current `issues/` and open and closed issues in the local `exact2` and `exact` checkouts. `exact2/issues/closed/20260924-windowed-list-duplicate-key-poisons.md` concerns recovery from duplicate keys in the retired windowed-list engine. `closed/20260904-measure-runner-update-scaling.md` fixes other keyed-list quadratic loops. Neither covers this surviving ordinal search.

## Resolution

Duplicate-key assignment remembers the next suffix for each canonical key instead of restarting its search at zero for each row.

## Verification

`many_equal_keys_receive_sequential_identities_without_mounting_every_row` passed with 8,192 equal keys, stable sequential identities and a bounded mounted window. Existing insert, removal and reorder tests passed.
