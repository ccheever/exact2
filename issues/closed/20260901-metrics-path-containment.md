# `metrics.mjs` still uses the sibling-prefix path check

**Status:** Closed
**Resolution:** Metrics now serves only declared public files through the shared canonical containment resolver.
**Systems:** Tooling
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023 Stage 1 (dev/serve/agent closed `startsWith(dist)` to `dist + '/'`)

`dev.mjs`, `serve.mjs`, and `scripts/agent.mjs` contain with `path.startsWith(dist + '/')` plus `isFile()`. `scripts/metrics.mjs` still does `path.startsWith(dist)` with no `isFile()`. `resolve(dist, '.' + '/../' + sibling)` can serve a neighbor whose path is a string prefix of `dist`.

The metrics server is loopback-only and diagnostic, so this is not the LAN hole. It is the same bug class left in the one script that times the browser.

Fix: the same `dist + '/'` + `isFile()` check the other three servers already have.
