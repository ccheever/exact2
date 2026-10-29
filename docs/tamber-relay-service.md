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

The relay reads the first TLS record of each connection: the ClientHello, at most 16 KiB. It walks that record's fields and extensions for the server name, and never parses a second record: the parser is given a copy of the first record's bytes alone (and reads its argument by index, none past the record, which a test instruments); the chunks as they came, later records included, are kept unread only to be handed on whole. At most `limits.pendingPerIp` connections from one address, and `limits.pendingTotal` in all, may be waiting to become a session's or an authenticated connector's.

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
           [names: <tool> <canvas>\r\n]           names of the account's own, or the run's once named
           \r\n
relay:     READY <tool> <canvas>\n                then idle
       or  REFUSED <why>\n
relay:     CLIENT <name> <address>\n              then the client's bytes, both ways
```

- **Sessions.** Every connection of a run is in one session. A run's id is 12 hex digits of SHA-256 of its label and run id, and its names are `<id>-tool.<label>.<domain>` and its canvas twin. With `names`, both must be under one parent the account holds in the accounts file, or be that run's own names. The connector asks for its names again on every connection once named, so a run whose session lapsed, or whose relay restarted, gets the same names back. **An account's token is the account.** Share it only among people who may take each other's sessions, and revoke it (which cuts every run) when one of them should no longer. Names are an account's: no other account can ever claim them. Within one account, every run holds the same token, so the account is the boundary: a parent of the account's own is free to any of its runs once no live session holds it, and a session of the account with no connector connection left (in its grace) gives its names up to a new run at once, its waiting clients closed to dial again, and counts against no cap then (a restarted `tamber serve` is a new run and gets its names back), and a generated name is its run's by a 48-bit hash: anyone can search for a run id that hashes to it, but only the account's own token holder can claim the name with it. A session lives 60 s after its last connection. A session holds at most `limits.idlePerSession` idle connections (more are closed, said nothing), and its carried and waiting clients together at most `limits.streamsPerSession`.
- **TXT records.** A connection whose greeting carries `txt: <record> <value>` or `txt-clear: <record> <value>` is answered `TXT-OK` or `REFUSED <why>`, then closed. The record must be `_acme-challenge.<label>.<domain>`, the connector's own account's. Any other is refused, including another account's, the apex, or a name of the account's own parent (those go through the developer's own DNS, `--relay-dns-hook`). The value must be ACME-shaped. A clear takes down only its own value. Requests are limited per account, refused ones counted too.
- **Refusals.** A wrong token, an unknown label and a revoked one all get the same words, `no such account`, and cost the same work (one hash, one constant-time compare). A refused or answered connection is destroyed a second after its last line, whether or not its peer closes. After `limits.greetingsPerIp` refusals of unauthenticated greetings in a minute, an address is answered nothing (an authenticated connection's refusals never count to it). Every per-address limit counts an IPv4 address whole (mapped or not) and an IPv6 address by its /64.

## Accounts

`accounts.json`:

```json
{ "accounts": { "acme": { "token_sha256": "<64 hex>", "names": ["tamber.example.com"] } },
  "revoked": ["old-label"] }
```

- **Tokens** are kept as their SHA-256 only and compared in constant time. They are never logged.
- **`names`** are the parents an account may use with `--relay-name`. One account's parent may not be another's, or under or over another's, nor the relay's own domain, under it or over it.
- **Labels are never given out again.** A revoked label stays in `revoked`. Every label the relay has seen is appended to `labels.seen` beside the file, and every revocation too (`revoked <label>`), for good. A file that drops a seen label, or lists as an account one that is or ever was revoked, is not taken.
- **Revoking** is by the tombstone (`revoked`, or `accounts revoke`): a file that just drops an account is not taken (the label was seen), so the account stays live until it is revoked.
- **Reloading.** The file is read again when it changes (its directory is watched, so a file replaced by a rename is seen every time), and on SIGHUP. A file that isn't taken leaves the accounts before it in place. An account revoked or given a new token is cut at once: every connection (idle, carrying a client, or asking for a TXT record), every client waiting, and its DNS work under way. A session whose names are no longer its account's (a parent moved to another account) is dropped.

## Config keys

| key | |
|---|---|
| `domain` | the relay's own name. Sessions are `<id>-tool.<label>.<domain>` and `<id>-canvas.<label>.<domain>` |
| `listen` | `host:port` for clients and connectors, one port (443 deployed) |
| `tls.cert`, `tls.key` | PEM files for the relay's own name only. Checked at start: the certificate names `domain` and nothing else, is valid now and matches the key; otherwise no start |
| `accounts` | the accounts file (above). `labels.seen` is kept beside it |
| `dns.command` | puts an ACME TXT record up or takes one value down. It gets `EXACT_ACME_ACTION` (`publish` or `clear`), `EXACT_ACME_RECORD` and `EXACT_ACME_VALUE`, runs in its own process group, and has a deadline |
| `limits.perIpPerSession` | client connections a minute from one address to one session (30) |
| `limits.streamsPerSession` | clients carried or waiting at once for one session (64) |
| `limits.pendingPerIp`, `limits.pendingTotal` | connections not yet a session's or an authenticated connector's, from one address (32) and in all (4096) |
| `limits.idlePerSession` | a session's idle connector connections at once (8) |
| `limits.sessionsPerAccount` | sessions one account holds at once (4) |
| `limits.txtPerAccount` | TXT requests a minute from one account (10) |
| `limits.txtInFlight` | TXT requests one account has under way at once (2) |
| `limits.greetingsPerIp` | refused greetings a minute from one address before it is answered nothing (10) |
| `limits.waitMs`, `limits.graceMs`, `limits.helloMs` | a client's wait for an idle connection (2000), a session's life after its last connection (60000), a hello's or greeting's time (10000) |

Paths are relative to the config file.

## R4's lines, on loopback

`bun scripts/tamber-relay-gate.mjs` runs the relay, two accounts' design servers (the real connector, each wildcard by ACME through the relay's TXT records, against the ACME stand-in) and a client on loopback, and prints each line MET or FAILED with its evidence: a paired propose client reads and can't write; revocation cuts its long poll and its next request; plain HTTP is closed at the relay; a capture at the relay holds no token, code or design text; one account's names reach only its own server and its TXT records are refused to another; one account's canvas cookie opens its own canvas only (not the other's, not a tool); one session's pairing flood leaves another's untouched. The deployed R4 runs the same lines against the real host.

## Deploying it

`relay/deploy/deploy.sh <host> provision <domain>`, then `release`, run from a checkout over SSH: one Ubuntu or Debian host, firewalled to 22 and 443, the relay under a sandboxed systemd unit, released as one compiled file with a health check that switches back on failure; `rollback`, `install-cert`, `accounts-add`, `accounts-revoke`, `health`, `status` and `check` beside them (the script's header says each). The relay is outward-facing: it is deployed only to a host Charlie provides for it. `bun scripts/tamber-relay-gate.mjs` with `RELAY_GATE_DOMAIN` set runs R4's lines against the deployed relay (the file's header names the rest).

## What waits on hosting (the seams)

- **Domain:** `domain`. It needs wildcard DNS for `*.<label>.<domain>` pointing at the relay, and a way to set TXT records under it (`dns.command`, a script for the DNS provider's API).
- **The control certificate:** for `<domain>` itself, got by ACME at the provider like any server's. The relay holds no other.
- **Accounts:** `accounts add` and `accounts revoke` at first. A sign-in flow comes later.
- **Hosting:** raw TCP on 443, so a VM or Fly.io; plain HTTP platforms can't do SNI pass-through.
- **The Public Suffix List:** submit `<domain>` the day it exists. Add its `_psl` TXT record, then open a PR to publicsuffix/list giving the reason: each account's names must not be same-site with another's. It takes weeks to reach browsers. Until then, `tamber serve --relay` says the limit at start.
- **Rust:** a Rust relay behind the same protocol is queued for when the hosted one needs it, measured by sessions or memory per session.
