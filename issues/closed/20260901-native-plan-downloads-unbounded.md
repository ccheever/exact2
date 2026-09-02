# Native plan downloads are unbounded

**Status:** Open
**Systems:** Apple host, Linux host
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001

`host/apple/swift/PlanURL.swift:48-125,164-248` buffers page, envelope, and plan
responses completely before applying its 16 KiB HTML scan or declared plan
length. Its session timeout is 24 hours. The SSE parser appends until `\n\n`,
so a peer that never terminates an event grows memory without bound. Linux
likewise reads an entire response before scanning (`host/linux/src/fetch.rs`).

This lets a reachable development server exhaust a native host before any of
the format's size checks run.

Stream with hard byte ceilings at every rung (page, envelope, plan, event),
cancel immediately on overflow, bound time-to-first-byte and total duration,
and cap individual SSE lines/events. Test chunked responses that cross each
limit without a delimiter or declared content length.

## Attempted resolution (2026-09-01)

The Apple loader now streams through bounded URLSession delegates with page,
envelope, plan, and event ceilings plus request/resource deadlines. Linux now
strictly bounds and decodes the envelope after receipt, but its required ibex2
transport API still returns an already-buffered `Vec<u8>`; rustls caps that
buffer at 64 MiB and Darwin's completion transport does not expose chunks.
The three in-repository paths were exhausted: the public ibex2 request API has
no streaming/limit hook, post-receipt checks are too late, and introducing a
second HTTP executor would violate LLP 1016's executor boundary. This valid
ticket stays open for a bounded/streaming ibex2 response API and Linux chunked
overflow coverage.
