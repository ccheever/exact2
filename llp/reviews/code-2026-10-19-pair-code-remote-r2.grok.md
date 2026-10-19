# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `aa68d0a996`.
- **Method:** one brief with a security pass (sha256 `bd580d2201194edcf5fa015190e03b1ff64c0dd51f625548ba4e136375a9aa59`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. The `remote` field is only on a successful local `pair-code` answer, the fingerprint it names is the certificate the TLS client sees, and the loopback suite passed (20 tests, 0 failures).

## Nit

**[nit] Tunnel-only `tamber pair` still starts with "or" — `scripts/tamber-pairing.mjs:339`**

`list()` emits the TLS door first and the tunnel second, so both-doors output reads fine. `--trust-tunnel` alone is a supported door, and that line is then the only line under the code:

`or through the trusted tunnel to http://127.0.0.1:<port>: open /__tamber/tool/ at the other machine's end of it`

Prefix `or` only after a TLS line has been printed. Keep the rest: `door.origin` is the server's loopback end, and `/__tamber/tool/` is what to open on the other machine's forwarded end.

**[nit] Two protocol sentences still deny the tool on the remote doors — `docs/tamber-protocol.md:202` and `docs/tamber-protocol.md:1313`**

The transport row and the "Off this machine" paragraph now say the static tool is served there. The pair bullet still says `POST /__tamber/pair` is the one path a remote request reaches with no token, and "The design tool's files" still says `GET /__tamber/tool/…` is local requests only. `remoteHandler` serves that path on both doors (`scripts/tamber-remote.mjs:156`) via `serveToolFiles`. Qualify the pair sentence as the one unauthenticated protocol request, and say the same static files are served on the remote doors.

**[nit] Startup still says the TLS port serves nothing else — `scripts/tamber-remote.mjs:248`**

The line printed with the fingerprint says a remote client pairs and checks it, and that nothing else is served there. The same listener serves the wire and `/__tamber/tool/`, which is the URL `tamber pair` now tells the other machine to open. Say the tool's files are served there, and that the app and the token page are not.

## Checks

**(1) Shape and who can see it.** `pair-code` is `local` in the scope table (`scripts/tamber-pairing.mjs:58`). `dispatch` refuses before `mint` when that check fails (`scripts/tamber-server.mjs:650`). `remote()` runs only after a successful mint (`655`). A non-empty list is copied onto `ok.remote`; an empty list leaves the key off. `list()` (`scripts/tamber-remote.mjs:267`) returns `{origin, fingerprint}` for the TLS listener and `{origin, tunnel: true}` for the tunnel, TLS first. The fingerprint is the SHA-256 string from `X509Certificate.fingerprint256`. The key and the PEM stay on the certificate object.

A paired client, including `write`, gets `local: false`, so `pair-code` is 403 and `remote()` never runs. A remote or tunnel request is forced `access.local = false` (`scripts/tamber-server.mjs:486`) before the principal is chosen, so the local token is no one there (404). `hello`, `clients`, `pair`, and the pairing events do not carry the field. The fingerprint is the public certificate hash already printed at startup. Learning it by connecting grants nothing. The tunnel origin is that machine's loopback end. The secret on this response is still the code, and it stays on this local answer.

**(2) Getters.** All three pass `() => doors?.list() ?? []` with `let doors = null` before `tamberServer` (`scripts/tamber-server.mjs:1168`, `host/web/dev.mjs:63` then `399`, `host/web-js/dev.mjs:227` then `860`). The closure is not called during construction. `serveAlone` opens the doors before `listen`. A failed `bind` closes listeners already opened and throws; the catch exits, so the main port never listens. Both dev loops listen first, then assign `doors`. A code minted in that gap names no door; the failure path exits after `openRemote` has closed a half-open bind, and `stop` runs while `doors` is still null. The `--serve-as` child does not construct the wire (`servedAs == null` at `host/web/dev.mjs:399`) and is not passed `--remote` or `--trust-tunnel` (`host/web-js/dev.mjs:987`).

**(3) CLI.** `pairCode` prints the code, the scope, the expiry, and then each door. The local token, the private key, and the PEM are not printed. The TLS line is `https://host:port/__tamber/tool/` plus the certificate's SHA-256, and it says to check that fingerprint before entering the code. That path is served on the TLS listener. The same handler serves `/__tamber/tool/` on the tunnel. The tunnel line does not append that path to `http://127.0.0.1:<port>`. It names that address as the server's end and tells the user to open `/__tamber/tool/` at the other machine's end. The dangling "or" is the nit above.

**(4) Docs.** The 1.13 bullet names `pair-code`'s `remote` as additive (`docs/tamber-protocol.md:89-93`). The pairing bullet (`194-201`) matches the wire: optional `remote`, `{origin, fingerprint}` and `{origin, tunnel: true}`, absent when none is open, local token only. The transport row and the off-machine paragraph match the doors. The two leftovers are the nit above.

**(5) Test.** With both doors open, `ok.remote` equals `info()`'s origin and fingerprint plus the tunnel entry. A TLS handshake reads `getPeerCertificate().fingerprint256` and the test expects that to equal the named fingerprint before `POST /__tamber/pair` is sent. The pair response's peer certificate is the same value, so the certificate the client saw is the one named. `hello` reports `propose`. On that socket a `transaction` is 403 `needs: write`, `pair-code` is 403 `needs: local`, and the local token's `hello` is 404 with a body that does not contain the token. Those assertions require a status, so a socket error fails them. With no door, `remote` is absent. `bun test scripts/tamber-remote.test.mjs`: 20 pass, 0 fail.

The pin-before-send check and the pair request are two connections; the server does not pin. Refusing a mismatched fingerprint is the other machine's job. Also untested, and not a product bug: the one-door list shapes, and that the 403 body itself has no `ok.code` or `ok.remote` (the refusal returns before `mint`).
