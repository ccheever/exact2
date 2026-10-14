# exact-render does not compile on Windows, leaving its native HTTP executor unqualified

**Status:** Open
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
