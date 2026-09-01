# GPU artifacts leak across apps and reloads

**Status:** Closed
**Resolution:** Web builds replace complete per-app output trees and Apple removes stale optional GPU artifacts before conditional copying.
**Systems:** GPU, Web build, Apple build
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1009

Web and Apple builds use shared/fixed GPU artifact paths. Building an app with
no GPU crate does not remove a previous app's `gpu.js`, wasm/glue, or
`libexact_gpu.dylib` (`host/web/build.mjs`, `host/apple/build.mjs`). A native
host can therefore load the preceding app's module.

On web reload, `pendingSurfaces` is also not cleared before GPU initialization;
old instances can drain into the new plan, and updating a queued entry changes
values but not the surface name.

Make output replacement atomic and app-identity scoped, explicitly remove
optional artifacts when absent, and generation-tag/clear pending surfaces on
reload. Test GPU -> no-GPU app builds and reloads between plans with different
surface names.
