# Review round 3, LLP 1121 stage 10 A2, Grok (xhigh), security review

Reviewed: 0dd5c28d8d (each finding fixed in the next sha; the landed A2 is the last re-checked one, rebased).

## BLOCKING

None.

## SHOULD-FIX

None.

## NIT

None that is worth a change. A pair-time `name` that is a string of at most 40 characters but not the mint charset is ignored and the code's name is used (`scripts/tamber-pairing.mjs:252`), while `mint` returns 400 for the same charset. It cannot smuggle `remote:` or a newline.

## Round 2

- **Deadline on the body re-check.** `pairs.live` ends the client if `expires <= clock()` before the handler runs (`scripts/tamber-server.mjs:547`, `scripts/tamber-pairing.mjs:294`).
- **Scrub release after revoke.** `allowed` runs only after `await released()` (`scripts/tamber-scrub.mjs:227`). The server predicate is `!ended.has(client)` (`scripts/tamber-server.mjs:290`), and revoke adds that mark in the same turn as `end`. A false result restores the builds and does not write.
- **`restore()` keeps the revoked mark** when the proposal's client is in `ended` (`scripts/tamber-server.mjs:907`). New holds do the same (`:957`).
- **Local token only when `access.local === true`** (`scripts/tamber-server.mjs:496`). `remote` or `tunnel` forces `local: false` first (`:523`).
- **Subscription end.** The response is removed from the set before `res.end()` (`:562`). `send` skips `writableEnded` (`:198`).
- **Pair is limited before the body** (`:639`), and `pair(..., {admitted: true})` does not take a second token.
- **Deep nesting and handler throws** become 400: `JSON.stringify` in `ask` (`:374`) and the `dispatch` try (`:623`). `parse` already catches `JSON.parse`.
- **Late `onEnd`** gets the stored reason (`scripts/tamber-pairing.mjs:279`), including `'expired'`.

## What checks out

- **Codes and tokens.** 50 CSPRNG bits, two minutes, single use, at most 8 live, burn-all-at-5, constant-time compare of every live code (`scripts/tamber-pairing.mjs:245`). Tokens are 32 CSPRNG bytes, stored as SHA-256, 12h with re-armed timers. Five failures burn every live code, so extra source addresses do not extend a guess.
- **Revoked or expired clients.** The token is dropped before listeners run. The next request is 401 via `revokedTokens`. Subscriptions, long polls, scrub, pins, inspect, outlines, and holds end in that listener (`scripts/tamber-server.mjs:650`). Held proposals stay and are marked `revoked`. A write already handed to the design server still completes, as documented.
- **No acting as someone else or as local.** A paired body `client` is overwritten from the token (`:550`). Names are `remote:<base>#n` with a charset that cannot include `:`. Local `remote:…` is 400 except `revoke`, which is local-only. `clients`, `revoke`, and `pair-code` need `allows(..., 'local')`.
- **Off-machine.** Without a paired token, only `POST /__tamber/pair` is served. Token page and tool require `access.local`. The header is the only credential; cookies are not read.
- **Secrets.** Burn, limit, paired, and revoked logs and events carry peer, client, scope, or reason, not a code or token. `clients` does not list tokens. Pair returns the token once in the body.
- **Crashes.** Unauthenticated `pair` rejects a non-object body and a non-string code before `timingSafeEqual`. Read-scope failures stay inside `dispatch`'s try. History's git invocations are argument arrays with `--no-textconv`, `--no-show-signature`, and `GIT_NO_LAZY_FETCH=1`.
- **Tests.** The new HTTP tests cover scope and client binding, cookie and LAN rejection, tunnel and remote flags, mid-body revoke and deadline, deep nesting, revoke of subscribe, poll, scrub, pin, and proposals, and a late `'expired'`. The scrub predicate itself is unit-tested with a stub `allowed`, not through `ended` on the live server; the wiring above is the same check.
