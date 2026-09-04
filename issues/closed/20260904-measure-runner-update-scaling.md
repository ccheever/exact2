# Measure the cost of reevaluating every UI site

**Status:** Closed
**Resolution:** Measured 300/3000/10000-row workloads, removed demonstrated quadratic keyed, child, listener and motion costs, and retained full evaluation with desktop-only limits recorded.
**Systems:** Runner, Performance, Application composition
**Severity:** P3
**Author:** Codex, at Charlie Cheever's request
**Date:** 2026-09-04
**Related:** LLP 1005 §8; runner/src/runner.rs::update; scripts/metrics.mjs

`Runner::update` walks the instance tree and reevaluates every site after
an update. Settlement also revisits derives and resources, with argument
equality avoiding unnecessary source requests. This is a declared simple
implementation; no large-app performance regression has been demonstrated.

The review measured Caltrain's 277-node runner at about 0.32 ms for a screen
change and 0.13 ms for a timer tick on this Mac. Those in-process medians
do not establish scaling or input-to-paint latency on a phone.

Use the existing metrics path and disposable workloads to measure a local
state edit with growing unrelated UI (for example 300, 3,000, 10,000 sites),
a keyed list, and a topology change. Attribute evaluation, settlement,
kernel apply, layout, and host work separately; report nodes, changed sites,
request counts, allocations, and p50/p95 with hardware and build identity.
Measure a representative slower device before generalizing desktop results.

Done when the measurements support a decision: retain the current walk, or
implement dependency-directed evaluation for a demonstrated bottleneck.
If optimization is warranted, extend LLP 1005 with the concrete measured
target and verify unchanged trees, effects, stale-reply behavior, and clocks.
No new blocking check, benchmark registry, or speculative dependency graph.

## Investigation and fix (2026-09-04)

Implemented by Codex at Charlie's request. `node scripts/metrics.mjs --scaling
--json` now runs the disposable workloads through the existing release metrics
binary. Four warmup updates precede 40 measured updates per workload. The
workload has N unrelated keyed text rows plus eight fixed nodes; a counter
edit touches exactly one node and requests no data. Reversal requests the
row source once, touches the parent, and creates/destroys no row. Topology
alternately hides and recreates all N rows, without a source request.

The measurements exposed quadratic loops before a dependency graph was
needed. Fixed: old-row linear searches now use the same canonical keys as
duplicate checking; kernel child membership uses a set; final child lists
are committed before the runner's unique-id destroys, avoiding one sibling
rebuild per removed row; the web host reads handler declarations in one
walk on creation; motion removal removes the four declared properties
instead of scanning every other node's slots once per destroyed node.
All sites still reevaluate, keys still retain row state, destroy receipts
keep their order, and kernel validation still gates one atomic commit.
Trade: a replacement can temporarily keep old and new kernel nodes live
within that commit, so peak allocation may rise during a large replacement;
this investigation did not measure allocator peaks.

Machine: Apple M5 Max, arm64 macOS 25.6.0, rustc 1.97.0
(2d8144b78, 2026-07-07), release. Baseline source commit
`0494ffe07e389269f056fa7915e4af8daa1c66a1`; final measured executable SHA-256
`fbea5f6fa755b28573b7ac5e1da35260fa44f2094fca85957cc61bb30a16f9cf`.

Milliseconds, p50 / p95; web includes runner and batch serialization, not DOM:

| Rows / action | Runner before | Runner after | Layout after | Web before | Web after |
|---|---:|---:|---:|---:|---:|
| 300 / bump | 0.18 / 0.20 | 0.17 / 0.19 | 0.02 / 0.03 | 0.18 / 0.21 | 0.18 / 0.19 |
| 300 / reorder | 0.23 / 0.25 | 0.20 / 0.22 | 0.02 / 0.03 | 0.23 / 0.26 | 0.21 / 0.22 |
| 300 / topology | 0.28 / 0.30 | 0.27 / 0.30 | 0.08 / 0.09 | 0.72 / 0.83 | 0.51 / 0.62 |
| 3000 / bump | 3.28 / 3.40 | 1.87 / 1.91 | 0.28 / 0.31 | 3.37 / 3.50 | 1.96 / 2.01 |
| 3000 / reorder | 6.96 / 7.61 | 2.16 / 2.21 | 0.28 / 0.30 | 6.87 / 7.61 | 2.28 / 2.35 |
| 3000 / topology | 14.11 / 15.02 | 2.72 / 2.90 | 0.88 / 0.94 | 47.53 / 51.87 | 5.36 / 6.34 |
| 10000 / bump | 29.70 / 30.71 | 6.47 / 6.64 | 1.33 / 1.51 | 30.48 / 31.38 | 6.96 / 7.18 |
| 10000 / reorder | 64.79 / 67.57 | 7.62 / 8.53 | 1.54 / 1.88 | 64.95 / 69.47 | 8.29 / 9.17 |
| 10000 / topology | 164.49 / 169.23 | 9.36 / 10.02 | 3.47 / 5.28 | 566.01 / 713.97 | 19.59 / 23.11 |

Decision: retain the full walk and remove the demonstrated quadratic costs.
A 10,000-row replacement still produces roughly 1.2 MB of JSON when shown;
these numbers do not establish browser frame time or phone performance.
Layout uses the monospace measurer. Settlement, evaluation and kernel apply
are timed together; allocations and a physical slower device were not
measured. We make no device-wide performance claim from this desktop result.
The report names those limits rather than presenting inferred phase or
allocation counts. Revisit dependency-directed evaluation if an actual app
still misses its interaction budget after these fixes.

Verification includes the existing runner/Contract keyed-state, duplicate-key,
rollback, stale-reply, timer and host suites; a numeric-key fixture verifies
`0` and `-0` retain identity, replaced rows leave no stale listeners, and a
hidden/shown subtree is coherent. Motion removal keeps neighboring nodes'
slots and queued frames intact.
