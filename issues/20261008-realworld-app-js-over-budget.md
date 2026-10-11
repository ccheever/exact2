# RealWorld's shipped app.js is 38.7 KiB against its 38 KiB budget

**Status:** Open
**Systems:** scripts/metrics.mjs (`JS_TARGET_KIB`), host/web-js (the JS target), apps/realworld
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P2 (a gated metric)
**Related:** a431625bc..57eeaa189 (`exactBodyFrom`, LLP 1108 D6 R2); LLP 1071

The async lane's metrics show `web app.js: realworld 38.7 KiB brotli-11 production; budget 38 KiB, VIOLATION` since 57eeaa18. Per check: 37.2 KiB (be15451e, cb762285), 37.7 (342a6eb2), 37.9 (19c41b8c), 38.6 (57eeaa18), 38.7 (e3e88d4f onward). The step over the line is 19c41b8c..57eeaa18, whose only JS-target commits are `exactBodyFrom` (a431625bc and its three review rounds, host/web-js's fetch path), which every app that fetches links. The budget was raised 30 → 38 KiB by Charlie (scripts/metrics.mjs, `JS_TARGET_KIB`).

Decide: raise RealWorld's budget (39 KiB), or link `exactBodyFrom`'s path by use (only a source that names it needs the file read, grants check and Blob path), which should take back ~0.7 KiB.
