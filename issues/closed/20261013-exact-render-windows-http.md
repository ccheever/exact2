# exact-render does not compile on Windows, leaving its native HTTP executor unqualified

**Status:** Closed
**Systems:** render host, Windows, native HTTP
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-13
**Related:** LLP 1027.006; LLP 1120

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

Exact `16347325632e431d7588764ef29be026f34419e7`, based on
`a01c62241440300f3644ecde83592606262c58e7` and consuming Ibex
`c4aa839ea35c3eef875de12309d048fa8df2123d`, adds a platform-neutral linger
seam. Unix retains its `UnixStream` and `poll` implementation. Windows uses a
private loopback TCP wake pair and WinSock `WSAPoll`, preserving the 2 second
drain deadline, 64 KiB read cap, 256 held-connection cap, bounded inbox,
immediate half-close, and actual `Stopper` drain.

On native x86_64 Windows, with four Cargo jobs and `EXACT_HTTP_ASYNC` unset:

- `cargo test --locked --offline -j 4 -p exact-render --test it
  a_fetch_runs_through_the_native_executor -- --exact` passed its one real
  loopback test (36 filtered).
- `cargo test --locked --offline -j 4 -p exact-render --lib --tests
  --no-fail-fast` passed 154 tests with nine existing ignored tests and no
  failures, including all linger cases and
  `a_drained_server_answers_what_it_took_and_stops`.
- `cargo clippy --locked --offline -j 4 -p exact-render --all-targets --
  -D warnings` and `bun scripts/caps.mjs` passed.
- The real `weatherlight-render.exe` (SHA-256
  `7d37c2280844e17f5bcef370ac2d4b2670ca7c669cf8da9b937dcf01d8be2feb`)
  served its generated app plan over loopback. `/.exact/health` returned 200
  and `/` returned 200 with 113,075 bytes of rendered Weatherlight HTML
  (SHA-256
  `d5316910d47a246ef0ce579103444b75be19d9693217e06c47f97280430d363a`).
  The owned server PID was recorded before launch and was the only process
  stopped after the drive.

The enclosing Weatherlight build resolved Hermes from authenticated v5 receipt
schema `ibex/hermes-upstream-pinned-receipt/2`, receipt SHA-256
`32c11e970e9783fc8f4b5171700e146c0146a17ad7593a8237c70a3f9b34903e`,
and archive SHA-256
`db9233d08c25a33d7b1cdd2d0eb9428e98020fa6de7ab396b0adfb241b61d1e0`.
The standard web build compiled and ran the native Weatherlight renderer before
stopping at Weatherlight's existing colour-warning gate; that unrelated failure
is retained beside the successful server-drive receipt rather than represented
as a successful whole-app build.

An independent native Linux run at the same Exact commit passed 156 render
tests with nine ignored tests and no failures, including the loopback
drain/redirect/deadline coverage, followed by all-target, all-feature
`exact-render` clippy. Its worktree remained clean.

The original locked-run refusal before the dependency lock update, the first
Windows socket-type compile error, the Weatherlight warning-gate failure, all
successful command logs, PID cards, response receipt, and hashes are retained
under
`C:/Users/Charlie Cheever/targets/exact2-consumer-release-windows/logs/native-followup`
and the run ledger. No async scheduler mode was enabled.
