# Review round 1, LLP 1121 stage 10 A2, Grok (xhigh), security review

Reviewed: d2cb3414a1 (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

## BLOCKING

**A revoked or expired client can still act on a request that already passed authentication** — `scripts/tamber-server.mjs:516`

`principalOf` runs before `await body(req)` (line 525). `pair()` and `end()` are synchronous, but the body read is not, and nothing in `body()` imposes a deadline. After revoke or expiry, `onEnd` has already run, and the handler still uses the captured `who`:

- `dispatch` (line 596) does not look the token up again. `transaction`, `undo`, `commit`, `accept`, `propose`, `import`, `revert`, `drop`, and the reads do not consult `ended` or `present()`. A client can send headers, hold the body until after revoke, then submit the write. A `propose`/`hold` that lands after the mark loop is stored **without** `revoked: true`.
- `/__tamber/events` returns 200 on the fast path (line 577) whenever `wait` is false or any event is pending. The 401 check (line 587) runs only after the long-poll wait. The waiting-poll case in the test is the one path that is actually closed.
- `/__tamber/subscribe` writes the snapshot (lines 532–533) before `onEnd` (line 540). If the client is already gone, `onEnd` calls `fn` immediately, so the snapshot is sent and then the stream is ended.

Presence-gated methods (`pin`, `preview`, `inspect`, `outline`, `hold`, opening a `scrub`) do fail closed, because `present()` is false once `ended` is set. Re-resolve the principal after the body is in, and do it before any snapshot, poll answer, or `dispatch`.

## SHOULD-FIX

**The per-peer limiter does not bound the cost of a rejected attempt** — `scripts/tamber-pairing.mjs:202`

`take` failing still calls `say`, and the server logs that and `send`s a `pairing` event on every hit (`scripts/tamber-server.mjs:464`). One peer can push the 256-event ring and wake every subscriber as fast as it can POST. Buckets are deleted only when that same key is seen again at full (`scripts/tamber-pairing.mjs:151`). A peer that never returns, or a new `access.peer` per address, stays in the map for the life of the process. Emit the limit once per window, and sweep idle keys. This is separate from the 5-failure burn, which does bound guessing.

**A legal `--pair-ttl` kills the client early** — `scripts/tamber-pairing.mjs:220`

`parseDuration` allows up to 30 days. `setTimeout` is capped at `2**31-1` (~24.8 days) and `end(client, 'expired')` does not look at `expires` or re-arm. A 30-day token dies about five days early. Lookup is correct and this fails closed (it does not extend access). On fire, if `clock() < expires`, schedule the remainder.

**A write or scrub-release already inside `ask` is not tied to `onEnd`** — `scripts/tamber-server.mjs:620`

`waiting` is cleared only when the design server dies. `scrubs.release` sets `open = null` before `await write` (`scripts/tamber-scrub.mjs:221`), so `scrubs.absent` no longer sees it. A transaction or release that was fully submitted before revoke still lands. A post-body re-check does not cover this. Drop or ignore that client's in-flight replies, and don't apply a release that lost its client while it was writing.

**The C/B comment is not enough to implement the authorization rule** — `scripts/tamber-pairing.mjs:106`

It specifies `lookup`, `onEnd`, `allows`, and `{remote, tunnel, peer}`. It does not say that C must set `client` from `lookup` on every proxied method and stream and ignore the body (the wire does this only inside `handle`). It also does not say `onEnd` runs synchronously with `'revoked'` if the client is already gone, or that "being seen" is `lastSeen` only and does not refresh the 35s presence that owns pins, scrub, inspect, outline, and hold. For B: `GET /__tamber/token` and the tool are gated on `access.local` alone (`scripts/tamber-server.mjs:507`), and `access.peer` must be the connection address, not a client-supplied header.

## NIT

- `scripts/tamber-pairing.mjs:162` — the comment says the top 6 bits of the last byte are dropped. The code shifts the low 6 bits off a 56-bit value. The result is still a uniform 50 bits.
- `scripts/tamber-pairing.mjs:209` — a burn is logged without the peer. The protocol says burn and limit hits are logged with the peer. The peer is in scope and not passed to `say`.
- `scripts/tamber-server.mjs:517` — expiry and revoke both answer "this client was revoked". The `revoked` event carries the real reason.
- `scripts/tamber-pairing.mjs:104` — "or a WebSocket's first message" is not implemented and not specified. The protocol says the header only.
- `revokedTokens`, `ended`, and `names` grow for the life of the process. Only a successful pair adds to them, so this is not an unauthenticated leak.

## What checks out

**Tokens and codes.** `code50` is `randomBytes` trimmed to 50 bits and emitted as 10 Crockford characters. `32^10 = 2^50`, so there is no base32 bias. Tokens are 32 CSPRNG bytes, kept only as SHA-256, returned only in the pair body. The local compare is length-check plus `timingSafeEqual` (the length is the public 64 hex chars). Code compare calls `timingSafeEqual` for every remaining code of equal length, including after a hit (`!hit` is after the compare). A wrong length skips the compare; every live code is 10 bytes, and that length is public. The live-code count leaks; the code does not. Nothing in the log, `pairing` / `paired` / `revoked` events, 403 body, `clients` list, or URL carries a code or a token. `list()` has no hash. The pair CLI prints the code to stdout on purpose.

**Guessing.** Five failures increment every live code, then all of them burn. Extra peers, IPv6, and `::ffff:` mapped addresses do not raise that budget: the limiter runs first, but the burn is global and not per peer. `pair()` is synchronous, so a code minted while another attempt is in `await body` starts at `fails: 0` and still burns at 5. The 8-code cap drops the oldest without resetting the others. A limited attempt does not consume a code (covered by the unit test). The cheap pairing denial (five unauthenticated misses burn every live code, and a newly minted code can be burned the same way) is the anti-oracle ruling, not an implementation miss. It is acceptable only while the port is not exposed to an untrusted network. Once B exposes it, anyone who can POST can stop pairing for the whole two-minute window.

**Authorization, other than the stall above.** The body `client` is overwritten from the token before subscribe, events, and `dispatch`. `local` comes from the local-token compare, not the body. Scope is the code's scope. `pair-code`, `clients`, and `revoke` need `allows(..., 'local')`, so a paired `write` client gets 403. With `access.remote` or `access.tunnel`, the local token is not `tokenOf`'d, so it is 404, same as no token. `/__tamber/token` and the tool still require `access.local`. Unauthenticated traffic on those paths is 404. The only successful tokenless path is `POST /__tamber/pair`. Client ids are `name#n` and the counter is not reused, so revoke does not free the id. `--lan` and `--allow-host` do not set `remote`/`tunnel`; a leaked local token still works there until B sets the flags. That matches the split in the comment.

**After a clean revoke** (no request already past `principalOf`): `onEnd` drops presence, `channel.clear` hands back pins, inspect, outlines, and holds, `scrubs.absent` ends an open scrub and restores, waiting long polls get 401, subscriptions `res.end()`, held proposals stay marked, and the next completed request is 401. `present()` consults `ended` before the 35s window, so that window does not keep a dead client. `attend` in a poll's `finally` can recreate a presence row; `ended` still makes `present()` false. The 401-vs-404 split is acceptable: it requires the token itself, and an unknown token stays 404.

**Cookies.** The only read is `x-exact-tamber-token`. The gate still refuses a cross-site POST that carries `Origin`. The tool forwarder copies that header and not `Cookie`.

**Expiry mechanics.** Lookup and the timer both call `end()`, which is once-only (`clearTimeout`, delete, then callbacks). A clock passed into `pairing` is used for code TTL, token `expires`, and lookup. `setTimeout` is wall time. They meet when `clock` is `Date.now`. A skew fails closed on the next lookup or when the timer fires, except the 30-day cap above. Idle SSE does not call `lookup`; the timer is what ends it.

## Tests

Covered: code shape, single use, code TTL, lookup expiry with an injected clock, `onEnd` once, cross-code burn, nothing in `say` containing the code prefix, per-peer 429 leaving the code usable from another peer, the 8-code cap, revoke once, `parseDuration`, wire scope and client overwrite on `propose`, local-only methods, `clients` without the token, cookie-only hello is 404, remote hello with no token and with the local token are 404, `/__tamber/token` is 404, pair still answers, revoke ends a live subscription and a waiting poll within 2s, clears a pin, runs an external `onEnd`, marks the held proposal, and the following hello is 401.

Not covered, and the stall bug sits in the gap: a body held across revoke or expiry, then `transaction` / `accept` / `events` with `wait: false` / subscribe; inspect, outline, and hold actually cleared (the test pins, and `owners()` would see the others, but nothing sets them); expiry tearing down a stream (it only checks hello 401 and `onEnd('expired')` — same callback as revoke, so the wiring is shared, but the timer path is never run); the server log and event list containing no code (the assertion is the token only); `--pair-ttl` above `2**31-1`; a scrub-release already inside `write`. Constant-time behavior is not something the tests can prove; the compare loop is the right shape for equal-length guesses.
