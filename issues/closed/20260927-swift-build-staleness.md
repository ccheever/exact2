# The Swift module build keeps shipping a stale archive after a source is deleted or removed from the list, or the toolchain changes

**Status:** Closed
**Resolution:** Obsolete static Swift bridge/build path removed by LLP 1067.000; native modules use a per-session dynamically loaded artifact. Historical reproduction and fix evidence retained below.
**Systems:** JS bake (`js/bake/src/swift.rs`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1067 D7

`js/bake/src/swift.rs:58-63` rebuilds only when some input's modification time is newer than the library. Several changes never trigger a rebuild:
- A deleted input has no modification time (`None`), which never compares as newer, so the old archive, including the deleted code, keeps shipping.
- A source removed from the list leaves its compiled contents in the archive.
- A source added with an older modification time (copied with preserved times, or an older blob checked out) links a library missing its symbols.
- A `swiftc` or Xcode upgrade never rebuilds.

**Fix:**
- Treat a missing input as an error.
- Fingerprint the ordered source list, the contents, and `swiftc --version` into the output path or a stamp file.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: Swift builds fingerprint ordered input paths and contents, compiler/SDK identity, target and flags, and refuse missing inputs; fingerprint and real rebuild/link tests pass (2 tests).

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max. Verification: confirmed by reading.

## Fix verification (2026-09-27)

Running the old `swift_native` helper reproduced both failures: removing an
input from its list reused the archive, and deleting a listed input succeeded.
`js/bake/src/swift.rs` now hashes source paths in order, their bytes (including
the bridge), compiler path/version, SDK path/build, module name, target and flags.
Cargo also watches compiler/SDK files and toolchain-selection environment variables.
A failed rebuild invalidates its old stamp before compiling.

Added `js/bake/src/swift_tests.rs`: ordering, removal, content changes with old
mtimes, missing inputs, and toolchain/SDK changes; a real Swift compile test
exercises warm reuse, removal/re-addition, deletion, old-dated edits, a compiler
version change, and two named archives linked and run together.
`EXACT_JS_ENGINE=stub cargo test -p exact-js-bake --lib swift::tests --no-fail-fast`:
`test result: ok. 2 passed; 0 failed`. The real compiler and linker run; the stub
only avoids the unprovisioned, unused Hermes dependency.

Root verification: build, Clippy (`-D warnings`), format, staged caps and boot
passed; the Cargo test run totaled 1,714 passed, 0 failed and 8 ignored across
71 test binaries. The two changed JS crates also pass Clippy with
`EXACT_JS_ENGINE=stub`. `glue.js` stays below 1,500 lines; boot still reaches
only `glue.js` and `navigation.js` before first pixel.
