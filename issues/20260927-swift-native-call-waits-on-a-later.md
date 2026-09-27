# A Swift module's `native.call` can hold the main thread while a `later` runs on an I/O worker

**Status:** Open. LLP 1067.000 Q5 (working direction) removes the lock when the module moves onto the main thread; until then, this stands.
**Systems:** Native executor (`js/native/ExactNative.swift`, `js/src/swift.rs`), Apple host (`host/apple/src/executor_core.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067 D3/D6, LLP 1067.000 Q5; `issues/20260927-swift-native-later-done-twice.md` (the fix that added the lock)

Each Swift instance serializes every entry through one lock, `Instance.enter`
(`js/native/ExactNative.swift:54-61`):
- `exact_native_later` calls the module's `later` inside it (`:145`), on an
  executor I/O worker (`exact-io-1`/`-2`, `host/apple/src/executor_core.rs:543-549`).
- The default `later` runs `call` synchronously (`:49-51`), so a module that
  implements only `call` holds the lock for the whole call.
- A synchronous `native.call` from a main-placed source (the default
  placement) runs on the main thread and takes the same lock (`:109`).

**Failure:** a source asks `native.later(x)` for work the module answers in
`call`, which takes 2 s. While it runs, any answer that calls `native.call`
on the main thread waits the whole 2 s, and so does every frame. A `later`
that does its work before returning, instead of dispatching it, fails the
same way.

The main thread waits on no other thread in the host. The rest is
short-held mutexes and the agent's deliberate run-loop spins.

**Fix:** LLP 1067.000 Q5. Every module entry runs on the main thread, and the
host dispatches `later`'s start there asynchronously, so no lock is needed.
If that direction is dropped, the minimum is to run the default `later`'s
`call` outside the lock and to document that `later` must return promptly.

Verification: confirmed by reading the code path; not driven.
