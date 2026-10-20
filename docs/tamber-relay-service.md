# The tamber relay service

The relay is what `tamber serve --relay <host> --relay-account <label>` dials out to (LLP 1121 §5 stage 10c). A design server behind a NAT holds a few idle connections to it, and the relay hands each client's TCP stream down one of them unopened. The client's TLS ends in the developer's own `tamber` process, so the relay never sees a token, a design or a canvas. `docs/tamber-protocol.md` covers the server's side: the connector, the certificate by ACME, and CAA.

This page covers the relay itself: `scripts/tamber-relay-server.mjs`, one Bun file. For self-hosting, `bun build --compile` turns it into one binary.

```
bun scripts/tamber-relay-server.mjs --config relay.json
bun scripts/tamber-relay-server.mjs accounts add <label> --config relay.json      # its token printed once
bun scripts/tamber-relay-server.mjs accounts revoke <label> --config relay.json   # for good
bun scripts/tamber-relay-server.mjs --help                                        # every config key
```

## One port, the ClientHello's name deciding

The relay reads the first TLS record of each connection: the ClientHello, at most 16 KiB. It reads only as far as the server name.

- **The relay's own name** (`domain`). TLS ends here, on the relay's only certificate. This is the control endpoint. Connectors greet it, and ACME TXT records are asked for here.
- **A live session's name.** That is `<id>-tool.<label>.<domain>` or `<id>-canvas.<label>.<domain>`, or a name under one of the account's own parents. One line, `CLIENT <name> <address>`, goes down an idle connection of that session, then the client's bytes as they came, both ways. The relay holds no key for these names and reads nothing past the first record.
- **Anything else** is closed with nothing said in clear: no server name, an unknown one, or no TLS at all.

**Limit:** a ClientHello split over more than one TLS record is closed. No mainstream browser sends one today.

ECH (Encrypted ClientHello) is out of scope. A client offering ECH for a session's name would show the relay only its outer name. ECH and SNI routing don't mix, and the relay doesn't offer it.

## The connector's protocol (`EXACT-RELAY/1`)

On the control endpoint, over the relay's own TLS (WebPKI-verified by the connector, with no flag to turn that off):

```
connector: EXACT-RELAY/1 <label>\r\n
           authorization: Bearer <token>\r\n
           run: <32 hex digits>\r\n               one random id per run, on every connection
           [names: <tool> <canvas>\r\n]           names of the account's own
           \r\n
relay:     READY <tool> <canvas>\n                then idle
       or  REFUSED <why>\n
relay:     CLIENT <name> <address>\n              then the client's bytes, both ways
```

- **Sessions.** Every connection of a run is in one session. The first time a run greets without `names`, it gets a fresh id (12 hex digits) and its two names under `<label>.<domain>`. With `names`, both must be under one parent the account holds in the accounts file. A session lives 60 s after its last connection, so a reconnect keeps its names.
- **TXT records.** A connection whose greeting carries `txt: <record> <value>` or `txt-clear: <record> <value>` is answered `TXT-OK` or `REFUSED <why>`, then closed. The record must be `_acme-challenge.<label>.<domain>`, the connector's own account's. Any other is refused, including another account's, the apex, or a name of the account's own parent (those go through the developer's own DNS, `--relay-dns-hook`). The value must be ACME-shaped. A clear takes down only its own value. Requests are limited per account, refused ones counted too.
- **Refusals.** A wrong token, an unknown label and a revoked one all get the same words, `no such account`. After `limits.greetingsPerIp` refusals in a minute, an address is answered nothing.

## Accounts

`accounts.json`:

```json
{ "accounts": { "<label>": { "token_sha256": "<64 hex>", "names": ["tamber.example.com"] } },
  "revoked": ["<label>"] }
```

- **Tokens** are kept as their SHA-256 only and compared in constant time. They are never logged.
- **`names`** are the parents an account may use with `--relay-name`. One account's parent may not be another's, or under or over another's, nor under the relay's own domain.
- **Labels are never given out again.** A revoked label stays in `revoked`. Every label the relay has seen is appended to `labels.seen` beside the file. A file that drops a seen label, or revokes one and lists it as an account too, is not taken.
- **Reloading.** The file is read again when it changes, and on SIGHUP. A file that isn't taken leaves the accounts before it in place. An account removed, revoked or given a new token is cut at once: every connection and every stream it carries.

## Config keys

| key | |
|---|---|
| `domain` | the relay's own name. Sessions are `<id>-tool.<label>.<domain>` and `<id>-canvas.<label>.<domain>` |
| `listen` | `host:port` for clients and connectors, one port (443 deployed) |
| `tls.cert`, `tls.key` | PEM files for the relay's own name only. Checked at start: the certificate names `domain`, is valid now and matches the key; otherwise no start |
| `accounts` | the accounts file (above). `labels.seen` is kept beside it |
| `dns.command` | puts an ACME TXT record up or takes one value down. It gets `EXACT_ACME_ACTION` (`publish` or `clear`), `EXACT_ACME_RECORD` and `EXACT_ACME_VALUE`, runs in its own process group, and has a deadline |
| `limits.perIpPerSession` | client connections a minute from one address to one session (30) |
| `limits.streamsPerSession` | clients carried at once for one session (64) |
| `limits.sessionsPerAccount` | sessions one account holds at once (4) |
| `limits.txtPerAccount` | TXT requests a minute from one account (10) |
| `limits.greetingsPerIp` | refused greetings a minute from one address before it is answered nothing (10) |
| `limits.waitMs`, `limits.graceMs`, `limits.helloMs` | a client's wait for an idle connection (2000), a session's life after its last connection (60000), a hello's or greeting's time (10000) |

Paths are relative to the config file.

## What waits on hosting (the seams)

- **Domain:** `domain`. It needs wildcard DNS for `*.<label>.<domain>` pointing at the relay, and a way to set TXT records under it (`dns.command`, a script for the DNS provider's API).
- **The control certificate:** for `<domain>` itself, got by ACME at the provider like any server's. The relay holds no other.
- **Accounts:** `accounts add` and `accounts revoke` at first. A sign-in flow comes later.
- **Hosting:** raw TCP on 443, so a VM or Fly.io; plain HTTP platforms can't do SNI pass-through.
- **The Public Suffix List:** submit `<domain>` the day it exists. Add its `_psl` TXT record, then open a PR to publicsuffix/list giving the reason: each account's names must not be same-site with another's. It takes weeks to reach browsers. Until then, `tamber serve --relay` says the limit at start.
- **Rust:** a Rust relay behind the same protocol is queued for when the hosted one needs it, measured by sessions or memory per session.
