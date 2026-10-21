# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 1 of 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, a throwaway worktree at `e4b3a0c265`.
- **Method:** one brief (sha256 `3dfc85ac5fc405195f37d891bcfd34456d8ca3d68b4686d7547ba90f199e13a9`), shared by both reviewers, blind to the other review; commit e4b3a0c265 (the fix as first committed). A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 NIT).
- **Disposition:** taken in `719f96bc55`: the test sends six wrong codes from the plain peer and redeems the live code through the door from the same peer, so a spent limiter slot or code attempt would fail it (both reviewers). Not taken: the JSON 404 differing from the loops' empty 405 (Astra SHOULD-FIX, fingerprinting) — it is notFound's, the answer /__tamber/token already gives a LAN peer; the loops' refusal convention is outside this fix (graded NIT in rounds 2 and 3).

I'll review HEAD as a read-only security pass: the commit itself, every caller of the Tamber wire, and whether a non-loopback peer can still reach it over plaintext.The commit is a small gate in `handle()`. Next I’ll trace every caller and how `access.local`, `access.remote`, and `access.tunnel` are set.No blocking or should-fix issues. One test gap.

**NIT** — The new test never checks that a refused plain attempt leaves that peer's pairing limiter untouched. `scripts/tamber-pairing-server.test.mjs:208`

The 404s use peer `192.0.2.40`, then the code is redeemed from a different peer inside `fromPeer` (`203.0.113.N`). `admit()` runs only inside `pairEndpoint`, which this refusal returns before, so a regression that counted the plain POST and then 404'd would still pass. Five such probes would 429 that address on the real door for a minute. The code itself does not spend the limiter or the code.

What checks out:

1. **Plaintext from off this machine is closed.** `handle` refuses `/__tamber`, `/__tamber/subscribe`, `/__tamber/events`, and `/__tamber/pair` unless `access.local`, `access.remote`, or `access.tunnel` (`scripts/tamber-server.mjs:493`). Every production caller:
   - Dev loops (`host/web/dev.mjs:1336`, `host/web-js/dev.mjs:866`) and `exact tamber serve` / the tool listener (`scripts/tamber-server.mjs:1149`) pass `developmentGate().check`, which sets only `{allowed, local}`. No header is copied into `access`.
   - The TLS door and `--trust-tunnel` (`scripts/tamber-remote.mjs:164`) set `remote: true` on their own sockets. The tunnel binds `127.0.0.1` and `::1` only.
   - There is no canvas gateway caller. `addPort` only stamps `remote: true` on that TLS socket.
   - `--via` forwards only after its own loopback listener has `access.local`, and the dev loop then sees that loopback fetch.
   - Under `--serve-as` the resident loop has no Tamber server and binds loopback even with `--lan`. The JS loop handles `/__tamber*` with the original socket before `forward()`, and `forward()` only carries `/exact.json`, `/__dev`, `/__dev/open`, `/__dev/generation/*`, and an envelope `GET /`. The resident's relay back to the parent is its own `fetch` of `/__dev/hello` and `/__dev/commands`, not the client's path. A forwarded native request is re-judged as loopback on the resident; that does not reach the wire.

2. **`access.local` is the gate's whole loopback test** (`scripts/install-page.mjs:293`): admitted, no `Forwarded` / `X-Forwarded-For` / `X-Forwarded-Host`, Host is `127.0.0.1:port` or `localhost:port`, and the socket peer is `127.0.0.1`, `::1`, or `::ffff:127.0.0.1`. A LAN peer cannot make that true with Host, Origin, a mapped LAN address, or `0.0.0.0`. Those headers can only turn `local` off. The LAN listener is IPv4 `0.0.0.0` plus `::1`. A proxy makes it true only when its TCP connection is already loopback, Host is a loopback name, and it adds none of those forwarding headers — the case `--trust-tunnel` is for.

3. **The refusal spends nothing and does not distinguish a live code or token.** It runs before `pairEndpoint`, so no `admit()`, no parse, no fail counter, no burn. It also runs before `principalOf` / `wasRevoked`, so a live, unknown, or revoked token is the same 404 with no body read. The body is `{message:"Not found."}`, the same helper as `/__tamber/token`. On `exact tamber serve` an unknown path is that same response. On a dev loop, `POST /__tamber` is that 404 while `POST /no-such` is an empty 405; `/__tamber/token` already answered LAN peers that way.

4. **No legitimate plain, non-local wire client is cut off.** Loopback (the design tool, `--via`, `tamber op`) still has `access.local`. The phone's build channel is not these four paths: `/__dev`, `/__dev/page`, `/__dev/hello`, `/__dev/commands`, and `/__dev/report` stay open to a `--lan` peer, gated by the stream nonce, which is what a phone build uses. `/__tamber/token` and `/__tamber/tool/` were already local-only on the plain listener; the TLS door still serves the tool files itself and calls `handle` with `remote: true`. Agent drivers do not speak `/__tamber`.

5. **`fromPeer` setting `remote: true` is right.** Every older use is "a remote client who may use the wire" (pair, or a malformed code that must still get 400 from `pairEndpoint`). The paired token is then used from loopback via `gate.check`. The plain-LAN case is the new test, which does show a live code and a live token getting 404 on all four paths and the same code still pairing afterward.
