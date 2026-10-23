# Code review: the dev loop's [::1] connections lose their requests' close under Bun, round 1 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `7ee8c7e66aef09779dca0fbb0cc464d03f1e74ffd00003ef01cb3dd8c032502e`), shared by both reviewers, each blind to the other's review; HEAD `3e8377ee5c`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; 2 SHOULD-FIX, 1 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken in the round 1 commit:
  - Astra 1, Grok 1 (an upgrade over [::1] that no one takes was left open, where an HTTP server with no `upgrade` listener closes it): the socket is destroyed when the main server takes no upgrade. Tested: removing the destroy leaves the request "left open".
  - Astra 2 (Chrome may reach `localhost` over IPv4, so the dev-loop test might not test [::1]): its Chrome runs with `--host-resolver-rules=MAP localhost [::1]`, and the localhost leg is checked only where [::1] answers (said otherwise). It fails on main at that leg.
  - Astra 3 ("plan ready" comes before the listeners): the test also waits for the post-listen banner.
  - Astra 4, Grok 2 (no teardown on failure; the port probe raced the [::1] close): try/finally closes the server; request errors fail the test; the probe waits for the [::1] server's own `close`.
  - Grok 3 (a stale comment): fixed.
  Astra confirmed the order (closeAllConnections, then close) is required under Bun, whose close() lets go of the native server; a comment says so.

1. **SHOULD-FIX** `host/web/serve.mjs:1056` — An `Upgrade` to `[::1]` is no longer closed. Adding `six.on('upgrade', …)` turns off the HTTP server's default (no listeners → destroy the socket). The handler only re-emits on the main server, and neither dev loop has an `'upgrade'` listener (SSE only). `emit` returns false, and the socket stays open. Destroy it when `server.emit('upgrade', req, socket, head)` returns false.

2. **SHOULD-FIX** `host/web/tests/ipv6-loopback.test.mjs:58` — The port check can fail after a correct close. `server.close()`'s callback runs in the same turn as `six.close()`, and `until` returns as soon as `req` `'close'` has fired. The listen socket is released on a later libuv turn, so `freeOnIPv6Loopback` can still see `EADDRINUSE`. Wait for the companion's `'close'` (the test already has `six`) before binding.

3. **NIT** `host/web/tests/ipv6-loopback.test.mjs:11` — The comment still says those connections are the main HTTP server's own.

The unit test goes red on main for the right reason: after the client `destroy()`, `ended.six` stays `0` and the `deepEqual` fails. The shutdown half is not what fails. The browser test loads the JS loop (`host/web/dev.mjs` with no `--wasm` delegates to `devJs`) and, once `localhost` connects as `[::1]` (the canvas URL, tried first), the localhost leg stays at one build until the 10s wait throws. The `127.0.0.1` leg still drops.
grok exit 0
