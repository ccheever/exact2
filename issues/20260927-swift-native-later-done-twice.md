# The Swift `native.later` seam double-frees when an app calls `done` twice, and a Swift module is called from two threads unguarded

**Status:** Open
**Systems:** Native executor (`js/src/swift.rs`, `js/native/ExactNative.swift`), Apple host (`host/apple/src/executor_core.rs`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067 D5–D7

`exact_native_later` hands the app's `later(_:done:)` an ordinary escaping closure. That closure forwards every call to Rust's `done` (`js/src/swift.rs:68`), which does `Box::from_raw(context as *mut NativeReply)` each time it is called. Nothing makes the closure one-shot (`js/native/ExactNative.swift:94-99`).

- **`done` called twice.** An app calls it from both a success path and a timeout, a common completion-handler bug. The second call frees the `NativeReply` again: a use-after-free in the host process.
- **`done` never called.** The box leaks. `NativeReply`'s documented "dropped unsent, the call fails as aborted" (`js/src/native.rs:15`) never happens, so the request stays pending forever.
- **Two threads, one module.** The default `later` runs `call` on the executor worker (`ExactNative.swift:44-47`, `host/apple/src/executor_core.rs:543`). A synchronous `native.call` runs on the source's thread against the same global `module` (`ExactNative.swift:53`). A module with ordinary mutable state (a dictionary) is then accessed from two threads with no lock. The API doesn't say it must be thread-safe.

**Fix:**
- Make `done` one-shot on the Swift side (atomically swap out the context; log and ignore a second call).
- Have Rust own the reply through a handle that fails as aborted when the Swift side drops it unanswered.
- Serialize calls into one module (a serial queue, or actor isolation), or state and enforce a `Sendable` requirement.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Grok 4.7 xhigh, Astra max (code and design). Verification: confirmed by reading the code path end to end.
