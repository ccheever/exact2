# Journal lines carry the agent clock only, so a fast reply can read as a long stall

**Status:** Closed
**Resolution:** Executor completion wall time is journaled independently of the agent clock; real delayed HTTP drives pass on JS web, macOS and iOS, alongside runner and native executor regressions.
**Systems:** Runner journal, Apple host, Linux host, web host, Agent API
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report D16 (2026-09-24); LLP 1012 §2; LLP 1012.000 D4 (clock replies' waitedMs)

The Crew port (report of 2026-09-24, D16) padded a drive with `clock +N` and reads while a fetch was really in flight. The agent clock reached t=38000 in about two seconds of wall time. The reply was pumped at whatever the agent clock read when it arrived (`Session.swift`), and journal lines are stamped with that clock only (`runner/src/runner.rs`). So an ordinary 200 ms request read as a 38-second stall, and a cycle went into chasing a latency bug that did not exist.

Fixed alongside: `clock settle` has one bound per call, driver errors name the operation, and under `--timing platform` the driver's clock starts from the app's. LLP 1012.000 D4 proposes `clock replies`, a wait for requests that leaves the clock alone and reports `waitedMs`.

Still missing is the reply's real latency in the journal. Each host's executor knows when it started a request and when its outcome came back. Add the wall milliseconds to the `fulfil` line the runner already writes (which now also carries the outcome summary), e.g. `fulfil 7 (state) [HTTP 200, 190785 bytes] after 212 ms`. Measure in the executor (`host/apple/src/executor_core.rs` and its Linux and render users; the web glue's fetch) and carry it on the outcome, never from the agent clock.

Done when a drive that advances the agent clock while a request is in flight shows the request's true wall latency beside its fulfil line on the web, macOS and iOS.

## Implementation and verification, 2026-09-30

The native executor captures elapsed wall milliseconds when a completion arrives,
before the UI drains it; Apple, Linux and render pass that duration to the runner's
fulfil journal. Direct host replies without an executor measurement omit it. Both
web targets measure requests with `performance.now()`; the wasm adapter emits an
adjacent reply line without changing its ABI, and the JS target logs the ticket.

Runner regression: a reply at agent time 1200 records wall time 212 ms. The shared
native executor regression deliberately delays draining and verifies that this
delay is excluded. Apple, Linux and render Rust suites pass. A production-generated
JS fixture fetched a real local HTTP response delayed by 350 ms, while its agent
clock advanced to 900000; the journal read `t=900000 reply 1; wall 353 ms` and the
reply rendered successfully. The same native
drive used Exact Live's real HTTP executor with a held local response: macOS
logged `t=900000 fulfil 1 (reply) [HTTP 200, 43 bytes; wall 359 ms]`, and the
iOS simulator logged wall 375 ms at the same agent time. Both held the request
for 350 ms and published its successful result. All three requested host drives
pass; the elapsed values are independent of the agent clock.
