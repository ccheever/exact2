# Independent HTTP refuses ceilings over about 32 MiB on Apple, though 64 MiB is documented

**Status:** Open
**Systems:** the Apple executor (`host/apple/src/executor_core.rs`), `exactIndependentHttp.maxResponseBytes` (`docs/reference.md`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone (`examples/t3-code`, PR #1047)
**Date:** 2026-10-26

## Symptom

`docs/reference.md:942` says an independent request's `maxResponseBytes` may be 1 byte to 64 MiB, and
`reservation` checks the same range (`executor_core.rs:1125`). A request with a ceiling above about 32 MiB is still
refused before it starts, with "native response ceiling exceeds the lane's byte budget". The T3 Code clone's PDF
preview asked for 64 MiB, then 50 MiB, and was refused both times; it now asks for 31 MiB (`media-frame.ts`), so a
sent PDF larger than that cannot be previewed.

## Cause

`reservation` charges a job twice its ceiling, for `Vec` growth while collecting (`executor_core.rs:1157`):
`limit = bytes + body * 2 + MAX_HEADERS * 2`. The independent lane's budget is 64 MiB (`BYTES[1]`, `:36`), and a
`limit` over the budget is refused (`:1161`). So the largest ceiling that can run is a little under 32 MiB.

## Ask

Make the documented range and the admitted range agree. Either:

- collect into a buffer sized from `Content-Length`, or reserve exactly, so the charge is the ceiling and not twice
  it; or
- raise the independent lane's budget to cover a 64 MiB ceiling; or
- document 32 MiB as the largest ceiling on Apple and check it in `reservation`, so the refusal names the real limit.

Other hosts were not checked.
