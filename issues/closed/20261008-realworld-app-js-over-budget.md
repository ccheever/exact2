# RealWorld's shipped app.js is 38.7 KiB against its 38 KiB budget

**Status:** Closed
**Resolution:** Charlie ruled (2026-10-12): budget raised 38 → 45 KiB in scripts/metrics.mjs with the reason beside it (43.45 KiB shipped at fe5b26798, grown by real features; the file-body path already links by use); the link-by-use cut hunt is a QUEUE.md line
**Systems:** scripts/metrics.mjs (`JS_TARGET_KIB`), host/web-js (the JS target), apps/realworld
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P2 (a gated metric)
**Related:** a431625bc..57eeaa189 (`exactBodyFrom`, LLP 1108 D6 R2); LLP 1071

The async lane's metrics show `web app.js: realworld 38.7 KiB brotli-11 production; budget 38 KiB, VIOLATION` since 57eeaa18. Per check: 37.2 KiB (be15451e, cb762285), 37.7 (342a6eb2), 37.9 (19c41b8c), 38.6 (57eeaa18), 38.7 (e3e88d4f onward). The step over the line is 19c41b8c..57eeaa18, whose only JS-target commits are `exactBodyFrom` (a431625bc and its three review rounds, host/web-js's fetch path), which every app that fetches links. The budget was raised 30 → 38 KiB by Charlie (scripts/metrics.mjs, `JS_TARGET_KIB`).

Decide: raise RealWorld's budget (39 KiB), or link `exactBodyFrom`'s path by use (only a source that names it needs the file read, grants check and Blob path), which should take back ~0.7 KiB.

2026-10-11: The file-body path was split into a lazy chunk; RealWorld's production `app.js` no longer links its validation and read code. Built over the baked RealWorld plan, it is still 42,671 bytes (41.67 KiB) brotli-11 against 38 KiB. The issue remains open for a separate size decision; the budget was not raised.

Review fix round 1, 2026-10-11: production builds over each revision's RealWorld plan, compressed with brotli quality 11 and the raw size hint, measure 39,543 bytes at `57eeaa189`, 39,615 at `4a1d9fd1f`, 39,617 at `9ad0230e5^`, 42,031 at `9ad0230e5`, 42,294 at `c001a862b`, 43,229 on `origin/main` at `aec9a93a5`, and 42,848 on this branch after the lazy-load review fixes. The 2,414-byte jump at `9ad0230e5` is the optimistic-write overlay and its RealWorld source changes; later web changes account for the remaining growth. The split plus review fixes save 381 bytes against `aec9a93a5`, but the production bundle is still 41.84 KiB and the 38 KiB budget remains unchanged. This issue stays open.
