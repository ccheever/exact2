# Independent HTTP refuses or silently holds large ceilings on Apple, though 64 MiB is documented

**Status:** Open
**Systems:** the Apple executor (`host/apple/src/executor_core.rs`), `exactIndependentHttp.maxResponseBytes` (`docs/reference.md`)
**Severity:** P2
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

Below that it can still never start. A worker starts a job only once its whole charge is free in the lane, and waits
for it otherwise (`next_job`). Every open stream holds its own charge in the same lane: a socket's 1 MiB ceiling,
charged twice, about 2 MiB. With the T3 Code clone's local server and one paired server connected, a 31 MiB ceiling
(about 62 MiB charged) waited forever with no error: still pending after 40 s, while `curl` fetched the same URL in
4 ms. The clone now asks for 24 MiB (#1049), which fits beside about seven sockets; with more, it would hang again.

## Ask

Make the documented range and the admitted range agree. Either:

- collect into a buffer sized from `Content-Length`, or reserve exactly, so the charge is the ceiling and not twice
  it; or
- raise the independent lane's budget to cover a 64 MiB ceiling; or
- document 32 MiB as the largest ceiling on Apple and check it in `reservation`, so the refusal names the real limit.

Separately, a job whose charge cannot fit beside the lane's long-lived streams should fail (or be bounded by its
timeout) rather than wait with no end.

Other hosts were not checked.

## Partly resolved

A download written to a file (`fetch(url, {exactBodyTo})`, LLP 1108 D6 R2, 2026-10-26) is charged only the 1 MiB
it can hold, whatever its ceiling (up to 1 GiB), so it is neither refused nor left waiting beside open streams. A
preview that reads a file the host downloaded, as the T3 Code clone's PDF preview can, no longer needs a large
ceiling. A reply read into memory still has the 2× charge and the 32 MiB limit, and can still wait with no end.
