# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6f51896504`.
- **Method:** one brief with a security pass (sha256 `0dcdafd714dc8e8f1bc496004732057690e2315aee93e952b5a429ef62fd4f60`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. The new `remote` field matches the agreed shape, stays on the local `pair-code` answer, and the loopback suite passed (20 tests).

## Nit

**[nit] The new local-token check does not prove a refusal — `scripts/tamber-remote.test.mjs:445`**

`expect(status).not.toBe(200)` also passes when `send` returns a socket error and `status` is missing. The claim this commit makes is that the local token is refused on the TLS door. The earlier test in the same file already locks that as 404 with the token absent from the body.

Fix: keep the response and expect `404`, and expect the body not to contain `token`.

## Checks

**(1) Shape and who can see it.** `pair-code` is local-only (`scripts/tamber-pairing.mjs:58`, `allows` at `63-64`). `dispatch` checks that before it calls `remote()` (`scripts/tamber-server.mjs:649-656`). A failed mint returns the error unchanged and does not call `remote()`. A non-empty list is copied onto `ok.remote`; an empty list leaves the key off. `list()` (`scripts/tamber-remote.mjs:267`) returns `{origin, fingerprint}` for the TLS listener and `{origin, tunnel: true}` for the tunnel, in that order. It copies the fingerprint string only, not the key or the PEM.

A paired client gets `local: false`, so `pair-code` is 403 and `remote()` never runs. A remote or tunnel request is forced `access.local = false` (`scripts/tamber-server.mjs:486`) before the principal is chosen, so the local token is not a principal there (404, same as `hello`). `hello`, `clients`, and the pairing events do not carry the field. The fingerprint is the public certificate hash already printed at startup; the tunnel origin is the loopback end. Neither is a credential, and neither leaves this answer.

**(2) Getter wiring.** All three call sites pass `() => doors?.list() ?? []` and set `let doors = null` before `tamberServer` runs (`scripts/tamber-server.mjs:1168-1171`, `host/web/dev.mjs:63` then `399`, `host/web-js/dev.mjs:227` then `860`). The closure is not called during construction, so there is no temporal dead zone. Before `openRemote` resolves, `doors` is null and `pair-code` omits `remote`.

`serveAlone` opens the doors before `listen`. If `openRemote` throws, its own `bind` closes listeners it already opened and the catch exits, so the main port never listens. Both dev loops listen first, then assign `doors`; a code minted in that gap names no door, and the failure path exits after `openRemote` has already closed a half-open bind. The `--serve-as` child never constructs the wire (`servedAs == null` at `host/web/dev.mjs:399`) and is not passed `--remote` or `--trust-tunnel` (`host/web-js/dev.mjs:987`).

**(3) CLI.** `pairCode` prints the code, the scope, the expiry, and then each door (`scripts/tamber-pairing.mjs:337-339`). It does not print the local token, the private key, or the PEM. The TLS line is `https://host:port/__tamber/tool/` plus the certificate's SHA-256. That path is served on the TLS listener. The same handler serves `/__tamber/tool/` on the tunnel (`scripts/tamber-remote.mjs:156` and `253`). The CLI does not append that path there. It prints `through the trusted tunnel at http://127.0.0.1:port`, which is the local end in the field, not a URL the other machine opens.

**(4) Docs.** The 1.13 bullet names `pair-code`'s `remote` as additive (`docs/tamber-protocol.md:89-93`). The pairing bullet (`194-201`) matches the wire: optional `remote`, `{origin, fingerprint}` and `{origin, tunnel: true}`, absent when none is open, local token only. An older sentence in that same section (`225-227`) still says off-machine there is "no tool". That predates this commit. The remote-listener bullet at `179-180` and the CLI are the accurate description.

**(5) Test.** With both doors open, `ok.remote` equals `info()`'s origin and fingerprint plus the tunnel entry. The TLS client's `getPeerCertificate().fingerprint256` on the pair response equals that same fingerprint, so the certificate the client saw is the one named. The pair body and `hello` both report `propose`. The local token's `hello` is only asserted not to be 200 (the nit above). With no door, `remote` is absent. `bun test scripts/tamber-remote.test.mjs`: 20 pass, 0 fail.

Not covered, and not a product bug: a paired token calling `pair-code` on the TLS port (the 403 is the existing local-scope gate, already tested on this wire in `scripts/tamber-pairing-server.test.mjs:75-77`), a propose token's write refusal on that socket (same `dispatch`), and the one-door shapes. `list()` is the same two branches the both-doors case already joins.
