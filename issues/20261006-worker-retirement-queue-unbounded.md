# Bound the native owner queue and retire redundant snapshots

**Status:** Open
**Systems:** data placement, native executors
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P1
**Related:** LLP 1027.002 D3/D5; LLP 1041; data/src/placed.rs

A slow placed source can accumulate an unlimited sequence of retirement notifications, each owning another copy of the scoped store.

`data/src/placed.rs:289` creates an unbounded `mpsc::channel<Job>`. `forgotten` at lines 590–623 sends a `Job::Forgotten` after each notification and clones `envelope::snapshot(store, grants)`. That path bypasses the native executor's bounded request admission. The owner consumes notifications serially behind any running answer. There is no queue count or byte bound, and identical notifications are replayed individually.

Reproduced against the current compiled `exact-data`: activated a `Placed<Slow>` worker whose answer waits on a channel, put a 256 KiB string in its granted store, dispatched one answer, and called `forgotten(&store, &[])` 128 times before releasing it. All 128 calls returned while the owner was blocked; all 128 notifications replayed afterward. The snapshots alone carried 32 MiB. Repetition has no configured bound.

Use bounded admission or a coalesced latest retirement view that preserves the required ordering relative to answer/resume turns. Do not block the UI thread waiting for queue capacity. Account for snapshot bytes as well as job count, and preserve mutation/durable-effect ordering.

Acceptance: a blocked worker under repeated commits retains bounded jobs and bytes; redundant retirement views do not accumulate; current calls remain ordered, forgotten resource calls do not unnecessarily start, and admitted mutation effects are preserved. Include the controlled blocked-owner reproduction in the placement tests.
