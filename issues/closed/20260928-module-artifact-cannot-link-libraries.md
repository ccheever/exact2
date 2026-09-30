# A native module cannot link a library

**Status:** Closed
**Resolution:** Fixed by the existing xcframework slice/link support in host/apple/build.mjs; the ticket records the real Ocho macOS consumer and explicitly scopes untested iOS/framework variants.
**Systems:** Apple host (module artifact), build
**Severity:** P2
**Author:** Claude Fable 5.1, building Ocho for Eliot Hertenstein
**Date:** 2026-09-28
**Related:** LLP 1024 (§1 names Ghostty as the consumer shape), LLP 1067.000

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed (this PR)

`libexact_modules.dylib` was compiled from `host/apple/modules/ExactNativeModule.swift`
plus the app's flat `modules/apple/*.swift`, with no way to add a header search
path, a library path or a library. LLP 1024's own example consumer — a terminal
on libghostty — therefore could not be built without editing the host build,
which is the "one host change per widget" the RFC set out to remove.

The fix: an `.xcframework` beside the module's Swift (a symlink is fine, and
`.gitignore`d for a 137 MB artifact) is linked into the module. Its slice for
the build's platform and architecture is read from the xcframework's own
Info.plist (Mac Catalyst never matches): a static library's slice gives `-I`
its headers (so `import GhosttyKit` resolves through the slice's module map),
`-L` it and `-l` each archive in it; a framework's slice gives `-F` and
`-framework`. The arm cache key includes each archive's size and mtime, so an
archive copied over the old one rebuilds the module. Frameworks a static
library needs are autolinked by `import`ing them in the module's Swift; what
an archive needs but cannot declare (`-lc++` for C++ inside) is
`host.macos.link` / `host.ios.link` in `app.json`, one argument each. Ocho's
`apps/ocho/modules/apple/GhosttyKit.xcframework` (on its own branch) is the
consumer; no app in this repo exercises the path yet.

Two more findings on this Mac (Xcode 26.6, Zig 0.15.2), outside Exact but in
the way of any libghostty consumer: Xcode 26's `ld` refuses Zig's archives
("64-bit mach-o member … not 8-byte aligned"), so each archive is repacked
with Apple's `libtool -static` first, and Zig's `libtool` step for the
xcframework's combined archive silently drops those members, so the slice is
composed from the per-library archives. Ocho's branch carries the recipe that
does both. The build must also target macOS 14 (`-Dtarget=aarch64-macos.14.0`)
or `build.mjs`'s mixed-deployment-target check refuses the link.

Not done: an iOS build on a device, and a framework-shaped slice (the code
path exists; no consumer has been linked through it).
