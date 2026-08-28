# RFC 0491 — where a layout pass's allocations actually come from

**Status:** measured record. **Date:** 2026-08-26.
**Authority for the axes:** `docs/kernel-refresh/w0a/performance-precommitment.json`
(signed, Charlie Cheever 2026-08-26). This file never restates or re-derives a
signed number; it records where the measured value comes from.

## Why this exists

The W0-A counting-allocator receipts say *how many* allocator calls a
steady-state full relayout of the pinned 1,000-node fixture makes. They do not
say *where*. Two signed positive targets ride on that number
(`allocation-full-relayout-1k-calls`, `allocation-full-relayout-1k-bytes`, both
`≤ 0.5x` baseline), and the program had been assuming the reduction would come
from the WS-A arena cutover — replacing the `HashMap<ViewId, Node>` storage and
the receipt walk.

Measurement says otherwise, and the margin is not close.

## The instrument

`kernel/benches/alloc_attribution.rs`. It reproduces the w0a_baseline
`allocation.full-relayout-1k` window exactly — same fixture, same alternating
style families, same gross `alloc`/`alloc_zeroed`/`realloc` accounting, same
steady-state rule — and additionally captures a backtrace for every counted
allocation, attributing it to the deepest frame in repo or vendored source.

    EXACT_ALLOC_ATTRIBUTION=<out.json> \
    EXACT_ALLOC_ATTRIBUTION_EXPECT_SHA=<sha> \
    CARGO_PROFILE_BENCH_DEBUG=2 CARGO_PROFILE_BENCH_STRIP=none \
    CARGO_PROFILE_BENCH_PANIC=unwind \
      cargo build -p exact-kernel --profile bench --bench alloc_attribution

`strip = true` on `[profile.release]` (which `bench` inherits) erases the
symbols the attribution needs; without `CARGO_PROFILE_BENCH_STRIP=none` every
frame resolves to `<unknown>` and the run reports 100% unattributed rather than
failing. The run aborts if `HEAD` is not the SHA passed in, and the artifact
records the SHA it measured.

## The instrument validates itself

The probe's totals must equal the signed axis, or it is measuring something
else. They do, on two architectures:

| | calls (even / odd parity) | bytes (even parity) |
| --- | --- | --- |
| Bones, arm64 macOS (`performance-exit.capture.json`) | 1,643 / 1,610 | 722,780 |
| Minisforum, x86_64 Linux (this probe) | **1,643 / 1,610** | 722,956 |

**Allocator CALLS are exactly architecture-independent.** Over 24 warm passes
the count is perfectly periodic from pass 2 with zero drift.

**BYTES are architecture-independent up to a constant +176 B on Linux**, on
both parities. Anyone comparing a Linux byte number against the signed limit
must carry that offset; it is not noise and it does not vary.

## The measured split — before

Steady-state full relayout, pinned 1K fixture, even-parity pass. Measured at
`c56d42e042dff98f18046072c12aaf30cc2ef6bc` (branch base `de4a00462`).

| bucket | calls | % | bytes | % |
| --- | --- | --- | --- | --- |
| vendored Taffy | **1,595** | **97.1%** | **564,640** | **78.1%** |
| exact-kernel bookkeeping | 48 | 2.9% | 158,316 | 21.9% |
| total | 1,643 | | 722,956 | |

Taffy, by site:

| site | calls | bytes |
| --- | --- | --- |
| `vendor/taffy/src/compute/flexbox.rs:573` — `Vec<FlexItem>` per container | 600 | 537,600 |
| `vendor/taffy/src/util/sys.rs:47` — `Vec<FlexLine>` per container | 600 | 14,400 |
| `vendor/taffy/src/compute/flexbox.rs:1265` — `Vec<&mut FlexItem>` unfrozen set | 395 | 12,640 |

exact-kernel, by site:

| site | calls | bytes |
| --- | --- | --- |
| `kernel/src/layout.rs:133` — `visit_layout_tree` absolute-location map + stack | 10 | 53,356 |
| `kernel/src/layout.rs:57` — `Vec<LayoutTreeNode>` growth | 9 | 65,408 |
| `kernel/src/lib.rs:604/607/614` — `validate_certified_root_admission` | 14 | 20,736 |
| `kernel/src/lib.rs:2644` — `publish_layout_changes` changed-in-subtree `Vec` | 9 | 8,176 |
| `kernel/src/lib.rs:2629` — `publish_layout_changes` subtree-id `HashSet` | 1 | 10,256 |
| remainder | 5 | 384 |

**The consequence, stated plainly: zeroing 100% of kernel bookkeeping moves
1,643 to 1,595 against a target of 822.** On the calls axis the WS-A production
arena cutover — the work the program has been describing as the allocation work
— is worth at most 2.9%. This is not an argument against the cutover, which is
RFC 0491's named architectural work for other reasons. It is an argument
against expecting these two axes to move when it lands.

## What changed, and the after

Three slices, each measured against the deterministic axis:

1. **`vendor/taffy` patch 3** — the unfrozen set becomes a lazy filter. No
   allocation, no pool, no unsafe. (EXACT-PATCHES.md patch 3.)
2. **`vendor/taffy` patch 4** — a thread-local pool of `Vec<FlexItem>` scratch
   buffers, returned on `Drop`, degrading to the incumbent behavior under
   `not(feature = "std")`. (EXACT-PATCHES.md patch 4.)
3. **`kernel/src/lib.rs`** — `validate_certified_root_admission` gains the
   W0-B presence-set early-out. It had been walking all 1,000 nodes on every
   layout pass of a fixture containing zero `List` nodes.

| axis | baseline | after | signed 0.5x target | ratio |
| --- | --- | --- | --- | --- |
| `allocation-full-relayout-1k-calls` | 1,643 | **634** | 822 | 0.386x |
| `allocation-full-relayout-1k-bytes` | 722,956 | **151,980** | 361,390 | 0.210x |

Both signed positive targets are met with margin, on the deterministic
architecture-independent statistic. **This is not the same claim as the Phase 5
exit conjunct**, which requires a paired same-session capture on the pinned box
(Bones) and also binds the three timing axes; see the caveat below.

## What is NOT claimed here

- **The Phase 5 exit conjunct is not shown met.** `kernel-p5-precommitment-axes`
  judges a paired capture on `bones-m4-mac-mini` across all eight axes. This
  record covers the two allocation axes only. A Bones re-capture is still owed.
- **`layout-full-1k` timing was already over its limit before this change**, in
  the landed exit capture: 349,729 ns current against a 340,954 ns regression
  limit. That is a pre-existing condition, not something this work introduced,
  and whether removing ~1,000 allocations per pass cures it can only be shown by
  a Bones re-capture.
- **The remaining 600 calls are the `Vec<FlexLine>` site.** It borrows
  `&'a mut [FlexItem]`, so pooling it needs lifetime erasure or an inline
  small-vector. Both targets are met without it; tracked in
  `issues/20260826-taffy-flexline-vec-per-container.md`.
- **Taffy's 2,060 generated conformance fixtures were not run.** They live only
  in the upstream git repo, not in the crates.io package this vendor copy came
  from (EXACT-PATCHES.md records this). The 89 in-package unit tests pass.

## Evidence

| what | result |
| --- | --- |
| `kernel-p5-relayout-result-equality` | GREEN — 7 pinned-1K + 43 LLP 0487 corpus cases byte-identical across both reference arms |
| `cargo test -p exact-kernel` | 694 passed, 0 failed |
| vendored taffy in-package tests | 94 passed, 0 failed |
| `cargo clippy -p exact-kernel --all-targets -- -D warnings` | clean |
| `verify-registry-consistency`, `repo-structure-placement` | green |

## Planted-defect ledger (RFC 0496 "gates prove themselves")

Run against the **committed** branch tip with a `baseline()` guard asserting
`HEAD`, a byte-clean tree, and the presence of each patch **before and after
every probe**. Every restore is `git checkout -f -- <file>` from a committed
baseline, never from an uncommitted one.

| plant | judged by | result |
| --- | --- | --- |
| **P3b** patch 4's pooled buffer retains one stale item (`truncate(1)` instead of `clear()` on reclaim, and no refill clear) | `kernel-p5-relayout-result-equality` | **RED** — "the differential reported divergence" |
| **P6** slice C's early-out fires unconditionally (`if true`) | `cargo test -p exact-kernel` | **RED** — `virtual_visibility::tests::lists_v2_v1_certificate_admission_mutants`, 662 passed / 1 failed |
| unmodified tree, before and after every plant | both | GREEN |

**P3 (an earlier form of P3b: no clear at either point) is recorded as a
result, not a failed probe.** It did not produce a wrong answer — it produced a
**non-terminating** one, running 19 minutes at 95% CPU before it was killed by
recorded PID. The pooled buffer accumulates across every container, so the
per-container item vector grows without bound and the pass goes quadratic.
Clear-on-reclaim is therefore not hygiene: it is what keeps the algorithm
linear.

### The gate is BLIND to patch 3, and its green is not evidence for it

Two plants against patch 3 came back green. Suspecting the plants rather than
crediting the gate produced a decisive canary:

> At the top of every freeze round in `resolve_flexible_lengths`, immediately
> after the `all frozen -> break` check, perturb `target_size` **and**
> `outer_target_size` on the main axis for every unfrozen item.

That is an unconditional corruption of the flex-resolution output, and it is
**GREEN on all 50 cases**. The only reading consistent with that is that the
freeze loop **breaks on its first `all frozen` check on every case**: every
flex item in the corpus is inflexible, so CSS flexbox §9.7 steps (b)-(e) never
execute at all.

So **patch 3 is not covered by `kernel-p5-relayout-result-equality`.** What it
actually rests on is stated rather than implied:

1. the equivalence argument — iteration order over `line.items` is unchanged,
   so every floating-point accumulation sums the same values in the same order;
   and
2. Taffy's own 89 in-package unit tests, which **do** exercise flex grow and
   shrink, and which pass.

The corpus gap this exposes is not one gate's problem — every layout gate
drawing from this corpus is blind to the same region. Filed as
`issues/20260826-layout-corpus-omits-flexible-items.md`.

## Carry this: editing `vendor/taffy` re-pins the choice-corpus oracle tape

`tests/layout/choice/evidence/kernel-corpus-oracle.tape.json` records
`engine.taffyVendorTreeObject` — the git tree object of `vendor/taffy` — as
provenance, and it is covered by neither `harness.sourceDigest` nor
`corpus.digest`. **Any edit to `vendor/taffy` therefore reddens
`choice-corpus-kernel-differential`** with "committed evidence is not
byte-identical to the rebuilt oracle tape", even when no layout result moved.

This is the baseline working, not corruption. The remedy is the one the check
itself prints:

    bun scripts/check-choice-corpus-kernel-harness.mjs --differential --write-evidence

Verify the regeneration rather than trusting it: on this landing, **exactly 1
of 2,381 leaf fields changed** (`engine.taffyVendorTreeObject`,
`b2a1947b4f3e...` -> `6207ffba35a5...`, which equals
`git rev-parse HEAD:vendor/taffy`) and no layout result moved. A semantic diff
of the tape before and after is what separates a provenance re-pin from a
silent behavior change.

@system @ref RFC 0491 WS-A; W0-A signed precommitment.
