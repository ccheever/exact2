# The Swift module build keeps shipping a stale archive after a source is deleted or removed from the list, or the toolchain changes

**Status:** Open
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

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Astra max. Verification: confirmed by reading.
