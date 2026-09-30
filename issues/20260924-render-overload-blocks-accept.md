# A slow rejected connection blocks the render server's accept loop

**Status:** Open
**Systems:** Render server, HTTP
**Severity:** P1
**Author:** Codex for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1048.000 D9–D11

When the queue is full, `host/render/src/serve.rs:144–155` reads the rejected
connection's request on the sole accept thread before writing its 503.
`set_read_timeout(200 ms)` limits each read's idle wait, not the total time
spent reading headers. A peer sending one byte every 100 ms keeps this thread
occupied even after every render worker becomes free. `close()` also drains
on this thread with a per-read timeout, extending the same problem.

Reproduced against the real `Server`, with one render worker, queue size one,
and a 500 ms render deadline:

1. Send incomplete headers on two connections, occupying the worker and queue.
2. Open a third connection and drip header bytes every 100 ms for 2.5 seconds.
3. Close the first two connections, freeing the render worker.
4. Request `/.exact/health` on a fourth connection with a one-second read timeout.

The health request timed out after 1.002 seconds despite the free worker. It
returned HTTP 200 after the third connection stopped dripping. The render
deadline does not bound HTTP header parsing. The 16 KB header limit bounds
bytes, but permits a very long stall at this transmission rate. The server
binds loopback; a public reverse proxy can expose the path when it forwards
request bytes as they arrive. Direct local clients also reproduce it.

Keep overload rejection bounded without reading/draining arbitrary peer input
on the accept loop. Apply absolute elapsed-time bounds to request headers and
connection draining, rather than relying only on socket idle timeouts. Verify
that a rejected slow peer cannot delay health checks or newly available render
capacity, and retain the normal 503 behavior.

Reviewed at `35cb7ac053cc98e6fb205d65adae8fb5d61e3de7`. Probe:
`/tmp/exact2-review-20260924/game-probe/src/bin/render-review.rs`;
output: `/tmp/exact2-review-20260924/render-review.log`.

2026-09-30 recheck: still valid. A nonblocking rejection prototype removed
the accept-loop wait, but a concurrent complete request could lose its 503
to a TCP reset when more request bytes arrived between the drain and close.
That prototype and its unstable regression were reverted after three fix
rounds. A bounded rejection executor or pollable graceful-close queue remains
necessary; merely dropping a nonblocking socket is insufficient.
