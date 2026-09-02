# Native plan downloads are unbounded

**Status:** Closed
**Resolution:** Fixed: ibex2's fetch Request gained max_body enforced during receipt by all three transports (Darwin had no ceiling at all and now cancels mid-transfer through a data delegate), and the Linux loader names a ceiling per rung
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

## Resolution (2026-09-02)

The Apple half landed on 2026-09-01. The Linux half was blocked on what the
earlier round named correctly — ibex2's request API had no way to say how large
a response may be — and the blocker was removed in ibex2 rather than worked
around here.

**Why the first attempt dead-ended.** It went looking for a *streaming*
response API, found LLP 0059.000 §5's standing exclusion ("bodies buffer
whole"), and concluded the boundary could not be fixed from outside. Streaming
was never what a bounded read needed. The body can still be delivered whole; it
is the buffer that needed a lid, and only the transport — the one rung holding
the socket — can hold one.

**In ibex2** (`~/projects/ibex`, uncommitted alongside this):

- `fetch::Request` gains `max_body`, with `DEFAULT_MAX_BODY` (64 MB) when a
  request names none, and `over_limit()` so the refusal reads the same
  whichever transport is underneath. `Transport::send`'s contract now binds:
  refuse a declared length already over the ceiling before reading the body,
  and stop receiving the moment what arrives would pass it.
- `RustlsHttpTransport` was already reading through `take()`; it now takes the
  caller's ceiling instead of a private 64 MB one, refuses an over-limit
  `Content-Length` up front, and the agent gained connect and read deadlines so
  a peer that accepts a connection and never answers — or answers a byte at a
  time — is bounded in time as well as size.
- `DevTcpTransport` bounds the head separately from the body, so a head that
  never ends is refused too (and the terminator search stays linear).
- `DarwinTransport` is the one that had **no ceiling at all**:
  `dataTaskWithRequest:completionHandler:` hands over one already-buffered
  `NSData`, so by the time the size could be measured the memory was spent. It
  now runs through `NSURLSessionDataDelegate` — `didReceiveResponse:` cancels
  on an over-limit declared length, `didReceiveData:` cancels mid-transfer —
  which is the same shape `PlanURL.swift` already used on the Swift side.
- LLP 0059.000 §3.5 and §5 and LLP 0057 §3 record the rule; four tests in
  `crates/ibex2/tests/fetch_limits.rs` hold every transport to it.

**Here**, `host/linux/src/fetch.rs` names a ceiling for every rung — page and
envelope 64 KB, plan bounded by the byte count its own envelope declared (and
that count refused up front if it exceeds 64 MB), matching `PlanURL.swift` — and
two tests drive `fetch_app` against a server that answers and never stops.

**Measured, before and after.** Against a chunked response with no declared
length and no terminator, the pre-change Apple transport accepted 256 MB of it
at a peak RSS of 861 MB — about 3× the bytes on the wire, since the `NSData`,
the C copy, and the Rust `Vec` all coexist. After: refused at 64 KB. The Linux
host driven at a live endless server now prints `response exceeded the
65536-byte limit`, falls back to the baked plan, and peaks at 56 MB.

**Verified** by running: the four transport tests green on macOS (so against
`DarwinTransport`) and on Linux (`ccheever-minisforum`, so against
`RustlsHttpTransport`); ibex2's full suite green on both, bar one pre-existing
case-sensitivity failure in the module loader on Linux; exact2's five checks
green; `node scripts/smoke.mjs linux` green; and the Linux host booted from the
live dev-server URL, plan intact.

**Still open, deliberately.** Linux has no SSE half yet (it waits on a Linux
display, per QUEUE), so the per-event cap remains Apple's, where it already
landed.
