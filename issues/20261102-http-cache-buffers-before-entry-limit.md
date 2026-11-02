# The render HTTP cache buffers complete responses before applying its size limit

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, render server, HTTP cache
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, a cacheable outbound response with no Content-Length can make the render server buffer an arbitrarily large body, despite the cache's 8 MiB per-entry and 64 MiB total limits.

`host/web-js/http-cache.mjs:64-73` checks a declared Content-Length, then calls `response.clone().arrayBuffer()` and checks `entryBytes` only after the complete body is in memory. The clone consumes the source independently of the render's reader. It can continue after that reader stops, and pending stores are outside the total-byte accounting. `host/web-js/render.mjs:88` shares this cache across renders.

Run `bun /tmp/exact-review-http-cache-limit.mjs`. The actual cache receives a local scripted 16 KiB stream with no Content-Length and `Cache-Control: max-age=60`, configured with `entryBytes=1024`. It pulls all 16 KiB, never cancels, then discards the entry: `{"entryBytes":1024,"receivedBytes":16384,"cancelled":false}`. This small reproduction proves the collection order without allocating a large body; it does not measure a live render server's peak memory.

Read the cache branch through a byte counter and stop collecting once the per-entry limit is exceeded. Cancel that branch without waiting for the render's branch, and bound the aggregate bytes held by concurrent pending stores as well as completed entries. Check the path with missing Content-Length and with a declared length smaller than the actual stream.

No duplicate found in the available exact2/exact issue corpora, including closed tickets. `exact2/issues/closed/20260924-render-server-buffers-static-files.md` fixed local static-file buffering in the Rust server; it does not cover this outbound HTTP cache.

## Resolution

Read cache clones incrementally under per-entry and shared pending-byte bounds. Reserve memory for the final contiguous copy, evict completed entries as needed, and release reservations on every exit without awaiting tee cancellation.

## Verification

`bun test host/web-js/http-cache.test.mjs` passes oversized chunked/underdeclared bodies, concurrent budgets, cancellation and ordinary cache reuse.
