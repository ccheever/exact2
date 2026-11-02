# The publisher follows redirects and buffers the whole body

**Status:** Closed
**Systems:** Origin publisher (`scripts/origin.mjs`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** `exact deploy` and `exact push` against an `https` or AppDrop origin follow redirects, with no timeout and no body cap. A 307 or 308 on an AppDrop PUT is sent again, method and body included, to the `Location`. A slow response hangs the publisher. A large response is buffered before the head's size check runs. Native update clients already refuse redirects and cap the head and the blobs.

**Cause.** `HttpsOrigin.get` fetches with `redirect: 'follow'` and then `arrayBuffer()`. `AppDropOrigin.list`, `send` and the head GET in `putHead` call `fetch` with no redirect option, so the default `follow` applies. `send` is a PUT. Fetch keeps the method and body on 307 and 308. Nothing in `origin.mjs` sets `AbortSignal.timeout`. `inspectHead` checks a 64 KiB lid only after `get` has the bytes.

**Reproduction.** Point `--origin` at a loopback server that answers `PUT /from` with `307` and `Location: /to`. The publisher issues `PUT /from` and then `PUT /to`. A GET of `exact.json` that redirects off the origin is followed, and the body is parsed as the head.

**Fix.** Use `redirect: 'error'` on every publisher fetch. Cap a head GET at 64 KiB and a blob GET at 64 MiB before allocating, and set a timeout. Keep `inspectHead` as a second check.

**Acceptance.** A 307 on a head GET and on an AppDrop PUT fails the publish and does not request the `Location`. A body over the cap fails without holding the rest. A direct 200 of a normal head still publishes.

**Resolution.** All publisher HTTP operations reject redirects, use a 30-second deadline covering headers and body, and read responses within explicit byte caps. Signed AppDrop writes cannot be replayed by a redirect.

**Verification after the fix.** Real local HTTP fixtures verified that redirected reads, listing and signed writes never reached the redirect destination; an oversized streamed head was refused and a direct response succeeded. The focused publisher/push/Android manifest batch passed 21 tests. Separate local requests stalled before headers and during the streamed body both timed out at 30.001 seconds.
