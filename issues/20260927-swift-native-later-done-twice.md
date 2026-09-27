# The Swift `native.later` seam double-frees when an app calls `done` twice, and a Swift module is called from two threads unguarded

**Status:** Fixed: Swift completions consume their Rust reply once or abort on drop, and module entries serialize; Rust ownership tests and the real Swift concurrency fixture pass (3 tests).
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

## Fix verification (2026-09-27)

The pre-fix Swift fixture reproduced duplicate completion, no abort on drop,
concurrent module entry (peak 18), and lost announcements. `js/native/ExactNative.swift`
now retains a locked one-shot reply, with a separate Rust discard callback in
`js/src/swift.rs`; the instance lock covers configure/call/later. Added the real
Swift fixture `js/native/tests/main.swift` and Rust ownership/retired-handler tests.
`EXACT_JS_ENGINE=stub cargo test -p exact-js --lib swift::tests --no-fail-fast`:
`test result: ok. 3 passed; 0 failed`. The stub is only for the unavailable Hermes
installation; these tests execute the real Swift bridge and Rust reply owners.

The required macOS check ran twice: `Executed 442 tests, with 1 failure` both times.
The failure is in unchanged `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
(`seen.count` was 1, expected 240, line 157), outside this ticket's code.

Root verification: build, Clippy (`-D warnings`), format, staged caps and boot
passed; the Cargo test run totaled 1,714 passed, 0 failed and 8 ignored across
71 test binaries. The two changed JS crates also pass Clippy with
`EXACT_JS_ENGINE=stub`. `glue.js` stays below 1,500 lines; boot still reaches
only `glue.js` and `navigation.js` before first pixel.
