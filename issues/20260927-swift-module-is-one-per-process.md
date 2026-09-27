# A Swift native module is one object per process, so a second session takes over the first one's announcements and data directories

**Status:** Open
**Systems:** Native executor (`js/native/ExactNative.swift`, `js/src/swift.rs`, `js/bake/src/swift.rs`), Apple sample host (LLP 1031 D10)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067 D7, LLP 1031 (sessions own and destroy their state), LLP 1024

`private let module = exactNativeModule()` (`js/native/ExactNative.swift:53`) is a single object for the whole process, and `ExactNative.announce` is a single slot (`:103-110`). Each activation's `exact_native_listen` overwrites that slot, and each activation's `configure` repoints the shared module.

**Failures:**
- Two sessions of an app that links a Swift module (the `--host` fixture) both watch `"meter"`. After B activates, `ExactNative.changed("meter")` re-asks only B's resources.
- B's `configure` points the module at B's data directory, so A's native calls read and write B's files.
- Each activation, including every dev reload, leaks one `Box<Changed>` (`js/src/swift.rs:103`). There is no unregister, and unloading a module clears only the request handler (`js/src/lib.rs:636`).
- The fixed `-module-name ExactNative` and `exact_native_*` symbols collide if two apps' modules are linked into one binary.

**Fix:**
- Give each activation an opaque instance handle, with listen/unlisten and teardown.
- Either fan announcements out to every registered session, or refuse a second activation in-process with a named error.
- Name the Swift module and symbols per app.

**Decision owed:** this sits under the design question of whether 1067's data-module seam and 1024's view-module dylib become one artifact.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max (code and design). Verification: confirmed by reading; the two-session case has not been driven.
