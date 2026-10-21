# exact-render does not compile on Windows, leaving its native HTTP executor unqualified

**Status:** Closed
**Systems:** render host, Windows, native HTTP
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-13
**Related:** LLP 1027.006; LLP 1120
**Resolution:** fixed by `b195e96dc`: a platform-neutral linger seam (Unix keeps `UnixStream` and `poll`; Windows uses a loopback TCP wake pair and `WSAPoll`), and `a_fetch_runs_through_the_native_executor` passes on native Windows

The Ibex adoption requalification ran Exact's storage, chosen-document and
Hermes-backed TypeScript paths natively on Windows. The focused actual HTTP
proof could not compile because `exact-render`'s shutdown linger mechanism is
Unix-only.

At Exact `dfa6a26344cf238f4b642805492806795e01ddfd`, run on x86_64 Windows:

```text
cargo test --locked --offline -j 4 -p exact-render --test it \
  a_fetch_runs_through_the_native_executor -- --exact
```

Compilation fails in `host/render/src/linger.rs`: it imports
`std::os::unix::net::UnixStream`, `std::os::fd::AsRawFd`, `libc::poll`,
`libc::pollfd`, `libc::POLLIN` and `libc::nfds_t`. No HTTP request starts.
This is a render-host portability boundary; it does not invalidate the native
Windows GUI host proof or the engine-backed request lifecycle tests.

## Scope

Give the linger wakeup and readiness loop a Windows implementation, or isolate
the Unix mechanism behind a platform-neutral internal seam. Preserve the Unix
shutdown/drain behavior and bounds. Do not replace the actual loopback exchange
with request-construction-only evidence.

## Acceptance criteria

- `exact-render` compiles on Windows without weakening its bounded shutdown.
- `a_fetch_runs_through_the_native_executor` passes on Windows against its real
  loopback server and the adopted Ibex portable HTTP executor.
- Existing Unix linger, drain, redirect and deadline tests still pass.
- The native Windows result records the Exact and Ibex SHAs, authenticated
  Hermes receipt where the enclosing lane uses Hermes, command, test count and
  retained failure evidence.

## Resolution

Exact `b195e96dc953733589e54167286da92f59725389`, with current main
`0d2d2e28e972ae5bf603105c794b45fecc6a2b08` merged and the vendored Ibex pin
`e60b3e0570315f31710744c1187f3cec5574fedf`, adds a platform-neutral linger
seam. Unix retains its `UnixStream` and `poll` implementation. Windows uses a
private loopback TCP wake pair and WinSock `WSAPoll`. A notification remains
pending until a byte is actually consumed; interrupted reads and writes retry
without losing it, and wake EOF or a hard error marks the inbox gone and
releases queued and held sockets. Later handoffs after that failure close
immediately.

The implementation preserves LLP 1048.000 D10's 2-second per-socket hold,
64 KiB read cap, 256 held-connection cap, bounded inbox, and immediate
half-close. The server's separate `Stopper` waits up to twice the render deadline plus
10 seconds for render workers, then drains the closing thread's remaining
2-second socket holds.

On native x86_64 Windows, with four Cargo jobs and `EXACT_HTTP_ASYNC` unset:

- The six focused linger tests passed. They cover delayed notification
  delivery, interrupted sends, coalescing, EOF cleanup of queued peers, two
  independent real wake cycles, post-thread handoff, and the held-socket bound.
- `cargo test --locked --offline -j 4 -p exact-render --test it
  a_fetch_runs_through_the_native_executor -- --exact` passed its one real
  loopback test (36 filtered).
- `cargo test --locked --offline -j 4 -p exact-render --lib --tests
  --no-fail-fast -- --test-threads=1` passed 118 unit tests, 37 integration
  tests, and one look test, with nine existing ignored tests and no failures.
  This includes `a_drained_server_answers_what_it_took_and_stops`.
- `cargo clippy --locked --offline -j 4 -p exact-render --all-targets --
  -D warnings` and `bun scripts/caps.mjs` passed.
- `cargo build --locked --offline -j 4 --profile host-dev
  -p weatherlight-linux --bin weatherlight-render` built the real
  `weatherlight-render.exe` (SHA-256
  `f11ba349477977a47f2db2a79e0e5c7bf41126ed6832b1e2b782d1d2afbd0885`).
  That binary served its generated app plan over loopback.
  `/.exact/health` returned 200 and `/` returned HTTP/1.1 200 with
  `text/html; charset=utf-8` and 100,584 bytes of rendered Weatherlight HTML
  (SHA-256
  `87c2c20abbe594f2681e2863c1defee908ac76648ce1b52d906c92773d6c75e5`).
  The owned server PID was recorded before launch and was the only process
  stopped after the drive.

The enclosing Weatherlight build resolved Hermes from authenticated v5 receipt
schema `ibex/hermes-upstream-pinned-receipt/2`, receipt SHA-256
`32c11e970e9783fc8f4b5171700e146c0146a17ad7593a8237c70a3f9b34903e`,
and archive SHA-256
`db9233d08c25a33d7b1cdd2d0eb9428e98020fa6de7ab396b0adfb241b61d1e0`.
The standard web build compiled and ran the native Weatherlight renderer before
stopping at Weatherlight's existing colour-warning gate. That surrounding
build failure is retained beside the successful direct render-binary build and
server drive; it is not a whole-app build pass.

An independent native Linux run at the same Exact source passed 120 unit tests,
37 integration tests, and one look test, with nine ignored and no failures.
All six linger tests and all-target, all-feature `exact-render` clippy passed;
the worktree remained clean. Its receipt archive SHA-256 is
`90a519daf438f0603cfe9172a7b6b763910f6021a816a04f3c87ae32acac7420`.

The earlier `16347325632e431d7588764ef29be026f34419e7` checkpoint is retained as
superseded evidence: independent review found that it could clear a pending
notification before the wake byte arrived. The original locked-run refusal,
the first Windows socket-type compile error, the review-triggered corrections,
the Weatherlight warning-gate failure, all successful command logs, PID cards,
response receipt, and hashes are retained under
`C:/Users/Charlie Cheever/targets/exact2-consumer-release-windows/logs/native-followup`
and the run ledger. No async scheduler mode was enabled.
