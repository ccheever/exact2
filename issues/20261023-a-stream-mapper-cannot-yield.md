# A stream mapper cannot yield, and a data module has no native inflater

**Status:** Open
**Systems:** the TypeScript socket and stream mappers on Hermes (LLP 1016.000 D7), the 100 ms turn budget (`js/src/lib.rs` `DEFAULT_BUDGET_MS`), the data module's web APIs
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord (`apps/excord/llp/0001-excord.rfc.md` §4.1)
**Date:** 2026-10-23

Status note: an ask

## Symptom

Discord's gateway sends a login's READY as one `zlib-stream` packet: for a large
account, megabytes of compressed JSON. Excord's mapper inflated it with `fflate`
(Hermes has no `DecompressionStream`), parsed it and applied it in one call. On an
M1 Max, a development macOS build against the fixture's padded READY
(`POST /control/ready-size`):

| READY | mapper call | result |
|---|---|---|
| 3 MB (674 KB compressed) | 129 ms | refused: over the 100 ms budget |
| 8 MB (1.8 MB compressed) | 341 ms | refused |

A refused mapper call fails the stream, so the gateway never reaches `ready`. The
reference's repair, `await scheduler.yield()`, is not open to a mapper: its call is
synchronous and returns the snapshot.

## What Excord does now

The mapper queues a packet over 16 KB compressed or 512 KB inflated, and every frame
after one while any wait, and a `digest` mutation drains the queue a bounded step a
turn: 32 KB of compressed input through `fflate`, a megabyte of a scan of the text
for `d.guilds`, the READY parsed without its guilds, or 1.5 MB of its guilds parsed
and applied. Each queued frame keeps its progress, so a digest whose turn the host
refused (its continuation never runs) is taken up by the next one. Its reply carries
a snapshot the view takes when it is newer than the stream's. A 32 MB READY then
lands with no turn over budget. The costs measured on the M1 Max: `fflate` about
40 ms per inflated MB, `JSON.parse` 7.5 ms per MB, the scan 23 ms per MB. The
default 64 MiB heap ran out on a 16 MB READY; Excord's host crates raise it to
256 MiB (`exact_js::Module::set_max_heap`).

## Ask

Either would remove most of the app's apparatus:

- `DecompressionStream("deflate")` in the data module natively (the web has it;
  Excord's would then be one call per packet, at native speed), or
- a mapper that may hand a message to a later turn of its own: an `exactStream`
  mapper returning a promise, given turns with their own budget as an answer's
  are, its snapshot the stream's when it settles.

`JSON.parse` of a whole large READY would still be one native call over 100 ms on
a slow phone; the scan stays the app's.
