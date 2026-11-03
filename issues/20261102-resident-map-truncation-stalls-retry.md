# An interrupted source-map response leaves its fetch pending

**Status:** Closed
**Severity:** P2
**Systems:** Design inspection, resident web dev loop
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`pullMap` in `scripts/tamber-inspect.mjs:42-60` settles on a complete response or an error on the outgoing request. It does not handle an aborted or prematurely closed response, and it has no deadline. If the resident child closes after sending the headers and part of its source map, the promise remains pending.

`residentMap` awaits that promise inside its retry loop (`host/web-js/dev.mjs:910-921`). An interrupted transfer therefore never reaches the next attempt. The affected plan has no map in `residentMaps`, so its native build's source inspection remains read-only until another map announcement starts a new fetch. This can occur when the resident child restarts during the transfer.

**Verification.** `/tmp/exact-review-map-abort.mjs` imports the actual `pullMap` and serves an owned loopback TCP response with valid map metadata, a complete HTTP header, and ten bytes of the advertised body. The server closes the connection after 20 ms. Under the pinned Bun, the promise was still pending after one second (`{"settled":false}`). No application or external service was used. Existing `tamber-inspect.test.mjs` coverage checks hashes, paths and plan identity, but not an interrupted response.

**Fix.** Settle once with `null` on response abort/error or close before completion, release buffered data, and bound the whole request with a deadline. Preserve the existing length, hash and plan checks.

**Acceptance.** A truncated response and a stalled response both settle within the bound, allowing the caller to retry; a valid complete response still returns its verified map.

No matching ticket was found in this checkout or the older Exact issue directories.

## Resolution

Settle map downloads on response errors, aborts and incomplete closes. Bound the whole exchange with a timeout and release partial data so the next request can retry.

## Verification

`bun test scripts/tamber-inspect.test.mjs` passes truncated and stalled responses followed by a valid retry.
