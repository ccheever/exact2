# The feedback endpoint buffers an oversized body before rejecting it

**Status:** Closed
**Severity:** P2
**Systems:** feedback worker, request limits
**Author:** Codex
**Date:** 2026-11-02

**Problem.** The unauthenticated `POST /diary` endpoint's 256 KiB limit does not
bound the memory or work spent reading a request. Oversized requests are read
completely into a string before rejection, so concurrent large uploads can
exhaust the worker's resources.

**Cause.** `scripts/feedback-worker.js:71` awaits `request.text()` before checking
the length. The subsequent `raw.length` check counts UTF-16 code units, rather
than incoming bytes, and admits UTF-8 payloads larger than the stated limit.

**Reproduction.** Called the actual exported worker with a local Request whose
ReadableStream supplied ten 64 KiB chunks. It consumed all 640 KiB and reached
EOF before returning 413. No network call or external worker was used.

**Fix.** Count bytes while reading, cancel at the bound, and return 413 without
draining the remaining request. A Content-Length check can reject early but
must not replace the streaming bound. Test a chunked oversized request and
multibyte UTF-8 near the limit.

## Resolution

Read request bodies incrementally under the 256 KiB byte bound, reject oversized Content-Length before reading, and cancel when streamed bytes cross the bound.

## Verification

`bun test scripts/feedback.test.mjs` passes chunked, multibyte, declared-length and valid diary cases.
