# Native session ID reuse verification

The real pinned T3 server on isolated port 16330 reproduced stale stream delivery crossing terminal lifetimes. The harness bound term-5, blocked main-queue delivery while writing/closing that real shell, dropped the old session, and bound the same environment/thread/terminal ID before the queue drained.

Before the fix (`before.log`), replacement notifications included an old `closed` status: `[running, closed, running, running]`. After (`after.log`), they were `[running, running]`; no old marker entered replacement output, attach count was one, and STALE_LIFETIME_CORRUPTION=false.

The production fix ties receive/ended/opened callbacks to the exact session object and attach generation; rejected old output remains acknowledged. The loopback regression queues old output, closed, and ending callbacks and proves replacement version, output, chunks, and error remain untouched. Full transport suite: 53 tests, 0 failures (`transport-after.log`).

This harness uses the production transport/session/output implementation with a headless view observer. It proves session lifetime behavior, not visible WKWebView rendering. Source hashes are in source-sha256.json. Fixture PID 89980 was stopped after verification. Pairing/server logs contain local credentials and are not publication artifacts.
