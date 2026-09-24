# Journal lines carry the agent clock only, so a fast reply can read as a long stall

**Status:** Open
**Systems:** Runner journal, Apple host, Linux host, web host, Agent API
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report D16 (2026-09-24); LLP 1012 §2; LLP 1012.000 D4 (clock replies' waitedMs)

The Crew port (report of 2026-09-24, D16) padded a drive with `clock +N` and reads while a fetch was really in flight. The agent clock reached t=38000 in about two seconds of wall time. The reply was pumped at whatever the agent clock read when it arrived (`Session.swift`), and journal lines are stamped with that clock only (`runner/src/runner.rs`). So an ordinary 200 ms request read as a 38-second stall, and a cycle went into chasing a latency bug that did not exist.

Fixed alongside: `clock settle` has one bound per call, driver errors name the operation, and under `--timing platform` the driver's clock starts from the app's. LLP 1012.000 D4 proposes `clock replies`, a wait for requests that leaves the clock alone and reports `waitedMs`.

Still missing is the reply's real latency in the journal. Each host's executor knows when it started a request and when its outcome came back. Add the wall milliseconds to the `fulfil` line the runner already writes (which now also carries the outcome summary), e.g. `fulfil 7 (state) [HTTP 200, 190785 bytes] after 212 ms`. Measure in the executor (`host/apple/src/executor_core.rs` and its Linux and render users; the web glue's fetch) and carry it on the outcome, never from the agent clock.

Done when a drive that advances the agent clock while a request is in flight shows the request's true wall latency beside its fulfil line on the web, macOS and iOS.
