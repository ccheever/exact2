# Measure the cost of reevaluating every UI site

**Status:** Open
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
