# Narrow re-check: Swift packages in the arm cache (58dc6f08e0), Grok

**RIGHT WITH CHANGES**

1. **NIT** — `host/apple/build.mjs:1110`. Package diagnostics are rewritten with `replaceAll(copy, author)` only. `copy` sits under `os.tmpdir()` (`/var/folders/…`), and that directory's real path is `/private` plus the same string. `snapshot.authored` in `host/apple/arms.mjs` rewrites the longer path first for this reason. A diagnostic that uses the real path is not mapped back; the shorter replace turns it into `/private` plus the author path.

The four checks hold otherwise. The module arm and both probe arms pass `packageStamp` (`p.module`, `p.sources`, and `packageArgs` with `packageDir` rewritten to `<packages>`) beside the copy text. Copies are taken with the app-source snapshot, before `placePackages` or any linking `arm()`; `swiftc` compiles `packageCopies`, and every linking arm hashes those copies. With packages, Catalyst, macOS, and a device build's simulator route are `lacking` with that package cause; a simulator build's simulator route is not. No packages leaves `lacking` null and the old framework-slice check, and a macOS build still dlopens its module. `placed = true` runs only after every package `swiftc` succeeds, so a later arm compiles again instead of linking a missing `.o`.
