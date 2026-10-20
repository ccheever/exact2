# Quick re-check 1 after round 3, LLP 1121 stage 10 A2, Astra

Reviewed: e8476a87b2 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

SHOULD-FIX — [scripts/tamber-server.mjs:201](/tmp/rv-rl-a2d/scripts/tamber-server.mjs:201): SSE broadcasts check only `writableEnded`, allowing expired paired subscribers to receive new design events before their expiry timer runs. Reproduced past the deadline. Check `pairs.live()` before sending to paired subscriptions.
