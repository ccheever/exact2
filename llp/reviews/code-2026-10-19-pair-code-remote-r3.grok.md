# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `912c238c25`.
- **Method:** one brief with a security pass (sha256 `1ed865d23c5235500b15b25a33d9b0927cd947f310d96d2d7e987eb52d8de701`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. No should-fix. The `remote` field stays on a successful local `pair-code` answer, the fingerprint it names is the certificate the TLS client sees, and the loopback suite passed (20 tests, 0 failures).

## Nit

**[nit] Stage 10 is still listed as future work — `docs/tamber-protocol.md:1328`**

"Not yet in the protocol" still ends with "pairing and remote clients (stage 10)." Pairing, the remote doors, and `pair-code`'s `remote` are specified under `tamber/1.13` in this same file. Remove that bullet.

## Checks

**(1) Shape and who can see it.** `pair-code` is `local` (`scripts/tamber-pairing.mjs:58`). `dispatch` refuses before `mint` when that check fails (`scripts/tamber-server.mjs:650`). `remote()` runs only after a successful mint (`655`). A non-empty list is copied onto `ok.remote`; an empty list leaves the key off. A failed mint is returned unchanged.

`list()` (`scripts/tamber-remote.mjs:267`) returns `{origin, fingerprint}` for the TLS listener and `{origin, tunnel: true}` for the tunnel, TLS first. `fingerprint` is `X509Certificate.fingerprint256` (the same string printed at startup). The key and the PEM stay on the certificate object. The new test's `toEqual` locks those two objects and no extra keys.

A paired client, including `write` on loopback, has `local: false`, so `pair-code` is 403 and `remote()` never runs. A remote or tunnel request is forced `access.local = false` (`scripts/tamber-server.mjs:486`) before the principal is chosen, so the local token is no one there (404). `hello`, `clients`, `pair`, and the pairing events do not carry the field. The fingerprint is the public certificate hash. The tunnel origin is that machine's loopback end. The secret on this response is still the code, and it stays on this local answer.

**(2) Getters.** All three pass `() => doors?.list() ?? []` with `let doors = null` before `tamberServer` (`scripts/tamber-server.mjs:1168`, `host/web/dev.mjs:63` then `399`, `host/web-js/dev.mjs:227` then `860`). The closure is not called during construction.

`serveAlone` opens the doors before `listen`. A failed `bind` closes listeners already opened and throws; the catch closes the wire and exits, so the main port never listens. Both dev loops listen first, then assign `doors`. A code minted in that gap names no door (it does not name a door without a fingerprint). The failure path exits after `openRemote` has closed a half-open bind, while `doors` is still null. The `--serve-as` child does not construct the wire (`servedAs == null` at `host/web/dev.mjs:399`) and is not passed `--remote` or `--trust-tunnel` (`host/web-js/dev.mjs:987`).

**(3) CLI.** `pairCode` prints the code, the scope, the expiry, and then each door (`scripts/tamber-pairing.mjs:337-342`). The local token, the private key, and the PEM are not printed. The TLS line is `https://host:port/__tamber/tool/` plus the certificate's SHA-256, and it says to check that fingerprint before entering the code. That path is served on the TLS listener (`scripts/tamber-remote.mjs:156`, `serveToolFiles`). The same handler serves `/__tamber/tool/` on the tunnel. The tunnel line names `http://127.0.0.1:<port>` as this machine's end and tells the user to open `/__tamber/tool/` at the other machine's end. `or` is prefixed only when a non-tunnel door is in the list, so a tunnel-only run does not start with "or".

**(4) Docs.** The 1.13 bullet names `pair-code`'s `remote` as additive (`docs/tamber-protocol.md:89-93`). The pairing bullet (`194-201`) matches the wire: optional `remote`, `{origin, fingerprint}` and `{origin, tunnel: true}`, absent when none is open, local token only. The pair sentence, the transport row, "Off this machine", and "The design tool's files" all say the static tool is served on the remote doors and that `pair` is the unauthenticated protocol request. The leftover is the nit above.

**(5) Test.** With both doors open, `ok.remote` equals `info()`'s origin and fingerprint plus the tunnel entry. On the connection that sends the code, `getPeerCertificate().fingerprint256` is compared to that fingerprint and the POST is written only when they match (`scripts/tamber-remote.test.mjs:441-443`). `hello` reports `propose`. On that socket a `transaction` is 403 `needs: write`, `pair-code` is 403 `needs: local`, and the local token's `hello` is 404 with a body that does not contain the token. Those assertions require a status, so a socket error fails them. With no door, `remote` is absent. `bun test scripts/tamber-remote.test.mjs`: 20 pass, 0 fail.

The mismatch path is not executed. The server does not pin; withholding the code is the other machine's job, and the test does that only in source order on the success path. Also untested, and already how the code behaves: the one-door list shapes, and `pair-code` with the local token on the TLS port (that request is 404 before `dispatch`, same as `hello`).
