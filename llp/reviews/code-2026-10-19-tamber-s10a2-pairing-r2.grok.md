# Review round 2, LLP 1121 stage 10 A2, Grok (xhigh), security review

Reviewed: 166b775c43 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

## BLOCKING

**Scrub release commits after revoke or expiry** — `scripts/tamber-scrub.mjs:221` and `scripts/tamber-scrub.mjs:227`

`scrub-release` skips the presence check (`scripts/tamber-server.mjs:824`). On an idle scrub it sets `open = null` and then `await released()`. On the dev loop that wait is a compiler round-trip (`host/web/dev.mjs:357`), so the event loop can run a revoke or an expiry in between. The end hook only calls `scrubs.absent` (`scripts/tamber-server.mjs:645`), and `absent` returns immediately when `open` is null (`scripts/tamber-scrub.mjs:248`). The release then sends the transaction (`tamber-scrub.mjs:228`). That write was not on the design-server pipe yet, so it is outside the documented in-flight exception. The builds do not return to the committed design. The `while (busy)` path is safe, because it rechecks `mine()` before detaching. The fix is to cancel an in-progress release from the end hook, and to write only if that client is still paired after `released()` returns.

## SHOULD-FIX

**A failed in-flight accept puts a revoked client's proposal back unmarked** — `scripts/tamber-server.mjs:894`

`accept` removes the proposal from the map before it waits (`scripts/tamber-server.mjs:974`). Revoke only stamps proposals still in the map (`scripts/tamber-server.mjs:647`). If that write then fails, or a recompute's `hold` fails, `restore` puts the original object back (`scripts/tamber-server.mjs:987`, `scripts/tamber-server.mjs:1019`) with no `revoked` flag. `hold` covers proposals created after the end, not this put-back. The revoked client still cannot send a new `accept` (401). The protocol requires the mark to stay. Stamp `ended` inside `restore`.

## NIT

**A late `onEnd` always reports `'revoked'`** — `scripts/tamber-pairing.mjs:271`

A client that has already ended invokes the callback immediately with `'revoked'`, including when the real reason was `'expired'`. Both reasons end the client, so stage C still tears the stream down. The reason itself is wrong.

## What checks out

Round 1 is fixed. The body is re-checked with `pairs.has` before dispatch (`scripts/tamber-server.mjs:543`). Paired names are `remote:<name>#n`, and a local `remote:` name is 400 except `revoke`'s target. `access.remote` or `access.tunnel` forces `local` false before the token page and the tool. The long-poll callback is initialized before `onEnd`. The limiter sweeps, caps at 4096, logs a limit once per burst, and the burn log names the peer. A bad code is 400. A lifetime past 2³¹−1 ms rearms. The module comment is the C/B contract: act as `lookup`'s client, `lastSeen` is not presence, `onEnd` is synchronous if the client is already gone, and `access.peer` is the canonical socket address.

No await sits between that re-check and `onEnd` on subscribe or the long poll, and `present()` consults `ended`, so a stream opened in that turn cannot outlive the client. The long poll's `finally` reinserts a presence row after revoke deletes it. `present()` stays false, so pins, scrub, inspect, outline, and hold do not come back.

A revoked client's own `accept` does not apply the proposal. A new request is 401. A full `accept` writes the design server in the same turn as the re-check, which is the documented exception. A partial accept only `hold`s, and that `hold` stamps `revoked`. A live `write` client may still accept the marked proposal. That is what the protocol allows.

A paired body cannot choose its `client` or its scope. A local client cannot wear a `remote:` name. `clients`, `revoke`, and `pair-code` are local-only. The token is taken only from `x-exact-tamber-token`. Off-machine, with `remote` or `tunnel` set, an unauthenticated request is answered only by `POST /__tamber/pair`. Codes are 50 CSPRNG bits, two minutes, single use, burned for every live code at five failures, compared with `timingSafeEqual`. Tokens are 32 bytes, stored by SHA-256, memory only, 12h, revocable at once. Logs, pairing events, and the client list carry no code or token.

Tests cover the cookie, remote-only `pair`, the local token being nobody on that path, subscribe, long poll 401, pin cleared, scrub end event, and an external `onEnd` (the canvas hook). They do not cover inspect, outline, or hold, expiry's effect on those streams (same callback as revoke), an in-flight `scrub-release`, or `restore` keeping the mark.
