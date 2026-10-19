# Code review: design tool T6 step 1 (the local tool pairs and revokes clients), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `31a9689db2`.
- **Method:** one brief (sha256 `08f547721cd7c12ce986eceb1cddc41cafdeeec46adbd64cb1710f0672629885`), shared by both reviewers, blind to the other review; commit 31a9689db2. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (1 NIT).
- **Disposition:** taken in `136b5e5f2b` (with step 2): a revoke goes to the server run that listed the client (Astra 1); the stub's refresh below write moves no inspect mode (Astra 2), a body arriving after a revoke is 401 (Astra 3), codes and tokens expire (Astra 4), a revoke hands back presence, inspect mode, pins and holds and marks held proposals (Astra 5), codes normalized and a bad code or name 400 without burning it (Astra 8, Grok 1), the events path through the scope table with last seen (Astra 9); a refused clients read is asked again each 3 s (Astra 6); tests for a refused pair-code and clients read, and, with step 2, remote mode end to end: a paired client's panel hidden, its token refused after a revoke (Astra 7). Not taken: the stub's burn after five misses, the cap of 8 codes and the rate limit (the tool never meets them; the server has them).

Nothing blocks. `~/x2bench/stage10-brief.md` is not on disk, so the wire check is `docs/tamber-protocol.md` (Pairing, Scopes, Authentication) and LLP 1121 stage 10 A2.

## Findings

1. **NIT** — A bad `POST /__tamber/pair` body does not get the server's status, and a bad `name` burns the code. `apps/design-tool/stub/tamber-stub.ts:294` looks the code up and, on a miss, always returns 403. A non-string `code`, or a string longer than 32 characters, is 400 on the server before any code is touched (`scripts/tamber-pairing.mjs:252`). The same handler then deletes the code (`tamber-stub.ts:296`) and accepts any `name`. The server returns 400 for a non-string `name` or one longer than 40 characters and leaves the code live (`tamber-pairing.mjs:253`). A body such as `{"code":"7Q2KD-M4X01","name":1}` is 200 on the stub and the code is spent; on the server it is 400 and the code can still be presented. The design tool never calls this path. A wrong code that is a normal string is still 403 and does not echo the code.

## Checklist

**1. Security.** The pairing code and the paired token stay out of logs, URLs, durable storage, error text, and the stub's database file.

The tool asks for a code with `{method:'pair-code', scope}` and never puts the code in the request (`apps/design-tool/pairing.ts:21`). The reply is held only in the `paircode` mutation and drawn as text (`app.contract:1285`). There is no `console.log` of it, it is not written into a URL, and the only persisted state in this app is `person` and `canvasSize` (`app.contract:610`, `613`). `loadClients` copies `client`, `name`, `scope`, `peer`, `seen`, and `until`, and drops the local row (`pairing.ts:36`). `call` sends the session token only in `x-exact-tamber-token` (`app.ts:86-87`). A refusal string is the server's `error.message` plus diagnostics (`app.ts:47-49`); the 403 text names the method, the scope, and `needs`, not a code or a token (`scripts/tamber-pairing.mjs:86-88`).

On the stub, `pair-code` does not touch the log. `POST /__tamber/pair` and `revoke` append `{event:'paired', client, scope}` and `{event:'revoked', client, reason}` and `save()` writes that log (`tamber-stub.ts:92`, `134-135`, `300-301`). The token is stripped in the `clients` reply (`tamber-stub.ts:161`) and otherwise lives only in the in-memory `paired` array and the `gone` set. A process restart drops both. Restart via `/__stub/restart` clears them too (`tamber-stub.ts:343`).

A paired client does not get the Clients section. It is built only when `hello.local` is true (`app.contract:1270`). Before hello arrives, `empty()` makes `local` false, so the section is absent. `loadClients` returns without calling `clients` when `local` is false (`pairing.ts:31-32`). On the stub, `pair-code`, `clients`, and `revoke` are `local` in A1's table, and `allows` requires the local token (`tamber-stub.ts:151-153`, `tamber-pairing.mjs:63-64`). A paired `write` token is 403 `{message, scope, needs}` before those handlers. The tool's own requests use the token from `GET /__tamber/token` (`app.ts:62-66`), which the real server answers only when the development gate calls the request local (`scripts/tamber-server.mjs:489-492`). The remote listener does not serve that path, so opening `/__tamber/tool/` on another machine never becomes a local session.

Treating a missing `local` as local is safe. On a successful hello the tool sets `local: ok.local !== false` (`app.ts:680`). A server that omits `local` is pre-1.13: the only token is the local one, issued only to a local request, and `pair-code` / `clients` / `revoke` do not exist. The section appears for that same person; minting a code is an unknown method and the error is shown (`app.contract:1297`). Nothing is issued. A 1.13 server always sends a boolean `local` (`tamber-server.mjs:692`). A paired token gets `local: false`, the section is not built, and the server still 403s local-only methods if the tool asked anyway. A failed hello sets `local: false` (`app.ts:672`), so a dead server fails closed. `local: null` is the same as omitted (the protocol reads null as absent) and still cannot mint a code on a server that enforces the table.

**2. Stub shapes and statuses.** The paths this step claims match the server.

| Call | Stub | Server |
|---|---|---|
| `pair-code` | 200 `{code, scope, expires}`, optional `remote` (`tamber-stub.ts:159`) | 200 `{code, scope, expires}` (`tamber-pairing.mjs:240`). `remote` is the proposed field, absent on the wire; the tool treats a missing array as no doors (`pairing.ts:26`) |
| bad `scope` | 400 (`tamber-stub.ts:156`) | 400 (`tamber-pairing.mjs:234`) |
| `POST /__tamber/pair` success | 200 `{token, client, scope, expires}` (`tamber-stub.ts:302`) | same (`tamber-pairing.mjs:280`) |
| wrong code | 403, message does not list codes (`tamber-stub.ts:295`) | 403 (`tamber-pairing.mjs:262`) |
| `clients` | local row, then paired rows, token omitted (`tamber-stub.ts:161`) | same (`tamber-server.mjs:655`, `tamber-pairing.mjs:300-302`) |
| `revoke` | 200 `{revoked}` or 404 `no paired client …` (`tamber-stub.ts:162`) | same (`tamber-server.mjs:657-659`) |
| next use of that token | 401 `{error:{message}}` with the server's GONE string (`tamber-stub.ts:123`, `128`) | same (`tamber-server.mjs:111`, `499`) |
| above scope | 403 `{message, scope, needs}` (`tamber-stub.ts:153`) | same (`tamber-server.mjs:651`) |
| `paired` / `revoked` | `take` writes those event names (`tamber-stub.ts:134`, `300`) | `send({event:'paired'…})` and `send({event:'revoked'…})` (`tamber-server.mjs:631`, `634`) |
| `hello` | `scope`, `local`, and `client` only when paired (`tamber-stub.ts:154`) | same (`tamber-server.mjs:692`) |

On `POST /__tamber`, `needs` / `allows` run after the body is parsed and before any method handler, including `hello`, `pair-code`, `clients`, and `revoke` (`tamber-stub.ts:149-154`). A method the table does not name is 400 and does not fall through. `inspect` other than `refresh: true`, and `drop` of another client's hold, are raised to `write` by the imported `needs`, and the paired client's name is written onto the request first (`tamber-stub.ts:150-151`), so a paired client cannot claim the owner's name to pass as `propose`.

`/__tamber/events` authenticates and returns 401 for a revoked token (`tamber-stub.ts:317-318`) but does not call `needs` / `allows`. That is not a hole with the scopes the stub can mint. `events` is `read`, and every token is `read`, `propose`, `write`, or the local `write` token, all of which `allows` would pass. A paired client cannot smuggle a local-only method through the long poll: the body is only `after`, `epoch`, and `client`, and a paired token's client name replaces the body's (`tamber-stub.ts:320`). The real server does check the stream path before the handler (`tamber-server.mjs:501-503`).

The stub is thinner than A2 on purpose where the commit said so: codes are the fixed series `7Q2KD-M4Xnn`, not 50 random bits; there is no two-minute expiry, no cap of 8, no burn after five misses, and no 429. Revoke removes the token, records it in `gone`, and emits `revoked`. It does not mark that client's held proposals `revoked: true` or clear pins and presence. The tool's inbox does not read that flag.

**3. The tool.** Clients are re-read when the list can have changed, not on build status. `loadClients(hello.local, changes.clients)` (`app.contract:650`) runs again when `changes.clients` changes. That string is `clientMoves` (`app.ts:1022`). It increases when a poll starts again — the first poll, a missed gap, or a new server epoch (`app.ts:1070-1071`) — and when an event is `paired` or `revoked` (`app.ts:1085`). `build-status` only updates `buildsNow`. A scrub frame bumps `scrubMoves` and `paced`, not `clientMoves`. A steady stream of build statuses keeps the same `clients` argument, so `loadClients` is not asked again.

Revoke shows its result and updates the list. The mutation is `Pinned` (`ok`, `error`, `note`) and `refreshes clients` (`app.contract:651`). Success text is the note; failure text is the error (`app.contract:1312-1313`, `pairing.ts:38-40`). The refresh runs after the server has removed the client, so the row is gone without waiting for the next poll. The `revoked` event re-reads the list as well.

A refused `pair-code` is the red `pair-error` line (`app.contract:1296-1297`, `pairing.ts:22`). A refused `clients` is `clients-error` (`app.contract:1300-1301`, `pairing.ts:34`). That branch is not the empty-list sentence, so a 403 does not also say "No client is paired."

The times are safe on the JS and wasm targets. `clock` parses an ISO string and calls `toLocaleTimeString` on `new Date(parsed)` (`pairing.ts:12-14`). It does not call `Date.now()`, `new Date()` with no argument, or `Intl.DateTimeFormat.format()` with no timestamp. Those are the calls the data-module prelude refuses. Both web targets format that Date's own time value with the engine's locale. A missing or unparseable time becomes `""`, and the row then omits "seen" (`app.contract:1308`).

**4. `pageOf`.** The move matches the old function. The regex, the port bounds, the rejection of the tool's own port, and the `127.0.0.1` / `localhost` pair are the same (`apps/design-tool/origins.ts:12-16`). The only caller passes `Number(new URL(API).port)` (`app.ts:678`), which is the value the old function computed internally from `API` (`http://127.0.0.1:7740`). Same input, same `{page, frame}`.

**5. Tests.** The two new tests drive the section rather than asserting static copy. After a restart, the propose note is checked, the picker is set to read, and the note changes (`app.test.contract:1704-1706`). Minting shows `7Q2KD-M4X01`, the certificate line for `https://192.168.1.20:7743`, and the tunnel line (`1707-1711`). `POST /__tamber/pair` with that code is from outside the tool; the client row appears with scope `read` only after the poll, which is the `paired` re-read (`1712-1718`). Revoke checks both the note and that the row is gone and the empty sentence is back (`1719-1723`). The second test checks the no-door sentence (`1726-1734`). Both restart the stub first, so the fixed code series starts at `…01`.

They do not cover a refused `pair-code` or `clients`, a paired token receiving 403 and 401, or `hello.local == false` hiding the section. Those paths are in the sources above and are untested.
