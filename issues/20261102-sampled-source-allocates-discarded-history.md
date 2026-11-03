# Sampled web sources collect every missed point even when history is disabled

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, module substitutes, motion samples
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, advancing the agent clock across a long interval makes a sampled source allocate an array for every missed grid point. The allocation is unbounded by the requested history, including the default `history=0`.

`host/web-js/samples.js:119-125` builds the entire `visible` array before taking its bounded tail. With no history it immediately discards all of it. Motion's normal 60 Hz sampling therefore collects 5,184,000 indices for a one-day jump, per reader, even though it returns no samples. A higher requested frequency increases the cost. A large jump or several readers can stall the browser or exhaust memory despite the advertised 1,024-sample ceiling.

Run `bun /tmp/exact-review-sample-history.mjs`. It imports the actual `samples.js` with a minimal clock stub, delivers the initial cursor, then advances one hour at 60 Hz with `history=0`. An instrumented Array.push counts `216000` collected grid points while `returnedSamples=0`. This bounded reproduction verifies the allocation count; it does not allocate a day of history or drive a compiled app.

Skip history collection when the bound is zero. For a positive bound, retain only the newest requested points and compute the dropped count without an unbounded array, accounting for hidden intervals. Native `runner/src/runner/modules/samples.rs:339-348` already retains only a bounded tail and does no counting walk when the bound is zero.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

## Resolution

Skip collection for history=0. For positive history, count visible index intervals arithmetically and materialize only the bounded tail, accounting for hidden spans.

## Verification

`bun test host/web-js/scheduler.test.mjs` passes one-hour 60 Hz jumps for zero and small history, dropped counts and hidden intervals.
