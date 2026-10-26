# Narrow re-check: Swift packages in the arm cache (58dc6f08e0), Astra

**RIGHT**

No BLOCKING or NIT findings in the requested scope.

- **Keys:** `packageStamp` preserves each package’s module name, ordered source names, and compiler arguments. All three package-linking arm calls include it alongside copied source text. Temporary package paths are normalized. [build.mjs](/tmp/rv-armpkg-astra/host/apple/build.mjs:1104)
- **Copies:** Sources are copied before package compilation and arm keying; compilation and subsequent probes use those copies. Package compiler errors replace copy paths with authored paths. [build.mjs](/tmp/rv-armpkg-astra/host/apple/build.mjs:1052)
- **Routes:** With packages, Catalyst and macOS fallback routes are marked lacking, as is a device build’s simulator route. Both eager compilation and later fallback skip them, retaining the cause. Native macOS builds remain eligible; apps without packages retain their existing route and framework checks. [build.mjs](/tmp/rv-armpkg-astra/host/apple/build.mjs:1123)
- **Placement:** `placed` becomes true only after every package succeeds. A failure leaves it false, allowing a complete retry. [build.mjs](/tmp/rv-armpkg-astra/host/apple/build.mjs:1106)

Read-only inspection and in-memory checks of extracted HEAD logic passed for key invalidation, temporary-path normalization, snapshot stability, diagnostic mapping, retries, and route selection. Compiler and filesystem effects were mocked; no files changed, builds ran, or simulators launched.
