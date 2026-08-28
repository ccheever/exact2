RFC 0491 W0-A draft artifact — performance-precommitment (human-readable companion of performance-precommitment.json; regenerate both with `bun scripts/generate-kernel-w0a-precommitment.mjs`).

# RFC 0491 W0-A performance precommitment — SIGNED

**Status: SIGNED.** The structure and the derivation rule below were
precommitted per Charlie Cheever's 2026-08-25 ruling (via orchestration
session exact-9e, decision register item 12). The drafted numbers were
ratified as drafted by Charlie Cheever on 2026-08-26 (relayed via
orchestration session exact-9e, packet ruling item 2; verbatim: "rec
fine"). The signature block below is completed and
`performance-precommitment.json` `status` is `signed`; these are the
binding Phase 1–5 gate values.

- **Pinned box:** bones-m4-mac-mini (fleet: Bones, M4 Mac mini, 10-core, 16 GB; Exact Native evidence box per fleet rules; pinned per Charlie 2026-08-25 via exact-9e register item 12) — Apple M4, 10 logical cores.
- **Capture:** `docs/kernel-refresh/w0a/performance-baseline.capture.json` (sha256 `2b0f41942ebbab435c6347237792d78eed3917eee685e920f0b1ee6fa8e7225e`), harness `kernel/benches/w0a_baseline.rs`, fixture `exact-native-kernel-layout-1k-v1` (1000 nodes, fnv1a64 `12f09a9610e91fa9`).
- **Source pin:** commit `0e2c9246766325b92b827c022fb63741a63c0795`, `kernel` tree `34e13fca4cfa3ebda3ac4de1454d0f66540c96ee`, `kernel/src` tree `ce8dc1772b67e353966bef71dfc4eccac9b56644`; legacy-oracle freeze pin `22acb34cd3d19fdcee88bf4a4c4234a981bbb521` (measured src matches pin: false).
- **Session noise floor (paired A/A):** median 0.063%, p95 0.094%.
- **Regression rule (every axis):** no worse than baseline beyond the recorded noise floor — timing limits are baseline × (1 + noise-floor p95); count limits are the max recorded steady-state run × (1 + observed run spread). Every later "≤ baseline" claim is a paired same-machine, same-session comparison (RFC 0491 §WS-A); cross-session numbers are trend context only.
- **Positive targets:** layout and allocation only (drafted below); all other axes are maintenance-only and carry their regression limit.

| Axis | Kind | Baseline | Noise floor | Regression limit (draft) | Positive target (draft) |
| --- | --- | --- | --- | --- | --- |
| layout-full-1k | timing-ns | 340.6 µs | 0.094% | 341.0 µs | 289.5 µs |
| mutation-styles-1k | timing-ns | 159.0 µs | 0.094% | 159.2 µs | — |
| export-typed-1k | timing-ns | 53.5 µs | 0.094% | 53.5 µs | — |
| allocation-full-relayout-1k-calls | count | 1,643 | 2.009% | 1,677 | 822 |
| allocation-full-relayout-1k-bytes | count | 722,780 | 1.449% | 733,252 | 361,390 |
| allocation-mutation-styles-1k-calls | count | 0 | 0.000% | 0 | — |
| allocation-build-plus-first-layout-1k-bytes | count | 11,154,900 | 0.000% | 11,154,900 | — |
| bytes-per-node-node-struct | count | 576 | 0.000% | 576 | — |

Positive-target rules (draft): layout-full-1k — median ≤ 0.85× baseline (≥15% improvement), and the improvement must exceed the comparison session's recorded noise floor (drafted). allocation-full-relayout-1k — ≤ 0.5× baseline steady-state allocator calls per full relayout (drafted); same factor for bytes.

**Integrated device measurement (wake → layout → receipt → presentation,
RFC 0491 Phase 5 exit conjunct 4): TREND-ONLY.** It is recorded and
reported on the pinned hardware class but does not gate pass/fail; a
non-regression envelope plus sampling protocol may be adopted at Phase 5
only if variance proves tight, and only by an explicit signed amendment of
this document.

## Signature

- [x] **Charlie Cheever** — I ratify the numbers above as the RFC 0491
      per-axis regression limits, the layout/allocation positive targets,
      and the trend-only device disposition. (Date: 2026-08-26)

Signature provenance: Charlie Cheever ruled the recommendation fine as
drafted on 2026-08-26, relayed via orchestration session exact-9e (packet
ruling item 2; verbatim: "rec fine"). This box was checked and
`performance-precommitment.json` set to `"status": "signed"` as the
mechanical recording of that decision; the agent did not accept on the
author's behalf. This document's numbers now bind the RFC 0491 Phase 1–5
gates; any repin, disposition change, or number change is a signed
amendment.
