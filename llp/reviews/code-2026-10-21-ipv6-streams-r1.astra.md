# Code review: the dev loop's [::1] connections lose their requests' close under Bun, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `7ee8c7e66aef09779dca0fbb0cc464d03f1e74ffd00003ef01cb3dd8c032502e`), shared by both reviewers, each blind to the other's review; HEAD `3e8377ee5c`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 3 SHOULD-FIX, 1 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken in the round 1 commit:
  - Astra 1, Grok 1 (an upgrade over [::1] that no one takes was left open, where an HTTP server with no `upgrade` listener closes it): the socket is destroyed when the main server takes no upgrade. Tested: removing the destroy leaves the request "left open".
  - Astra 2 (Chrome may reach `localhost` over IPv4, so the dev-loop test might not test [::1]): its Chrome runs with `--host-resolver-rules=MAP localhost [::1]`, and the localhost leg is checked only where [::1] answers (said otherwise). It fails on main at that leg.
  - Astra 3 ("plan ready" comes before the listeners): the test also waits for the post-listen banner.
  - Astra 4, Grok 2 (no teardown on failure; the port probe raced the [::1] close): try/finally closes the server; request errors fail the test; the probe waits for the [::1] server's own `close`.
  - Grok 3 (a stale comment): fixed.
  Astra confirmed the order (closeAllConnections, then close) is required under Bun, whose close() lets go of the native server; a comment says so.

No BLOCKER found. The core SSE fix covers both dev loops.

1. **SHOULD-FIX — [host/web/serve.mjs:1056](/tmp/rv-dt-six1-astra/host/web/serve.mjs:1056): Unhandled upgrades leave sockets open.** Neither loop installs an `upgrade` handler. Installing one unconditionally on `six` accepts upgrade requests, but forwarding them to `server.emit('upgrade', …)` returns `false` and does nothing. The client receives neither an HTTP response nor a closure. Preserve ordinary HTTP handling when the main server has no upgrade handler, or explicitly destroy unhandled sockets. [Bun’s upgrade dispatch](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/js/node/_http_server.ts#L898-L938)

2. **SHOULD-FIX — [host/web-js/dev-page-streams.test.mjs:39](/tmp/rv-dt-six1-astra/host/web-js/dev-page-streams.test.mjs:39): `localhost` does not prove IPv6 coverage.** Chrome can choose IPv4, making both iterations exercise the working path and allowing this test to pass on main. Force and verify IPv6 while preserving the `localhost` origin; explicitly skip that case when IPv6 is unavailable. The lower-level test correctly requests `::1` directly.

3. **SHOULD-FIX — [host/web-js/dev-page-streams.test.mjs:37](/tmp/rv-dt-six1-astra/host/web-js/dev-page-streams.test.mjs:37): Readiness precedes listening.** `plan ready` is printed at `dev.mjs:233`, before the listeners start at lines 1056–1057. Browser startup supplies only an incidental delay, especially with a shared browser already running. `page.goto()` can fail with connection refused before reaching the regression assertion. Wait for an HTTP readiness response or the post-listen banner.

4. **NIT — [host/web/tests/ipv6-loopback.test.mjs:50](/tmp/rv-dt-six1-astra/host/web/tests/ipv6-loopback.test.mjs:50): Failure bypasses teardown.** The expected assertion failure on main leaves the listeners open. Use unconditional cleanup, and reject request errors instead of swallowing them into a test timeout.

The remaining checks look sound: both callers use default server settings; neither relies on main-server `connection` listeners, `req.socket.server`, or CONNECT handling. `server.address()` remains the primary listener’s address. Forwarding preserves headers and the `::1` peer, so the development gate still recognizes localhost requests as local. EADDRINUSE rejection and the other-error fallback are unchanged.

Keep the force-close-first order for pinned Bun: its `close()` clears the native handle, preventing a subsequent `closeAllConnections()` from forcing those connections closed. Node’s usual reverse-order recommendation does not transfer directly here. [Bun’s close implementation](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/js/node/_http_server.ts#L415-L458)

Read-only review; no builds or tests run.
astra exit 0
