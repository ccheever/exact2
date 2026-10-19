# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 2 of 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, a throwaway worktree at `719f96bc55`.
- **Method:** one brief (sha256 `8291fddca3f066e1d0a6e040d317a5a9e5644072a0acaed7d21b5ad31a41a0c8`), shared by both reviewers, blind to the other review; commit 719f96bc55 (round 1 taken). A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve.
- **Disposition:** nothing to take (the known fingerprinting NIT stays as round 1 says).

I'll review HEAD as a read-only security pass: the commit itself, every caller that reaches the Tamber wire, and whether a non-loopback peer can still get through on plaintext.The review stays read-only on this commit. I'll read the diff and then trace every path that can still reach the wire.The gate is a four-path check in `handle`. Next I’ll trace every caller and how `access.local`, `access.remote`, and `access.tunnel` are set.No BLOCKING, SHOULD-FIX, or NIT findings. The plaintext gate holds, and the round-1 404 decision is the right one for this gate.

**1. Plaintext peers.** Every production `handle` gets an `access` the socket owner builds. The dev loops (`host/web/dev.mjs:1336`, `host/web-js/dev.mjs:866`) and `exact tamber serve` / the tool listener (`scripts/tamber-server.mjs:1148`) pass `developmentGate().check`, which is only `{allowed, local}`. `remoteHandler` (`scripts/tamber-remote.mjs:164`) sets `remote: true` and `tunnel` from which listener accepted the socket, with `peer` from `remoteAddress`. The TLS door is TLS-only. `--trust-tunnel` binds `127.0.0.1` and `::1`. `addPort` (canvas) does the same on a TLS socket; nothing in the tree calls it outside tests. The canvas gateway uses `pairing.lookup` / `onEnd`, not a plaintext `handle`. No caller copies `remote` or `tunnel` from a header.

The wire is judged on the loop that accepted the client. Both dev loops call `tamber.handle` before any forward, so a LAN peer's `/__tamber`, `/pair`, `/subscribe`, and `/events` are refused on that socket. The JS loop forwards only `/exact.json`, `/__dev`, `/__dev/open`, `/__dev/generation/…`, and an envelope `GET /` (`host/web-js/dev.mjs:951`), raw headers included. Under `--serve-as` the resident listens on `127.0.0.1` even when `--lan` is passed (`host/web/dev.mjs:59`) and does not mount Tamber (`:399`). Its relay back is its own loopback fetch of `/__dev/hello` and `/__dev/commands` (`scripts/tamber-builds.mjs:649`), not the client request. `--via` (`scripts/tamber-server.mjs:1351`) drops anything that is not already `access.local`, then fetches the loopback target with only `content-type` and the token header.

**2. `access.local`.** It is the whole loopback test (`scripts/install-page.mjs:293`): admitted, no `Forwarded` / `X-Forwarded-For` / `X-Forwarded-Host`, `Host` equal to `127.0.0.1:<port>` or `localhost:<port>`, and `remoteAddress` in `{127.0.0.1, ::1, ::ffff:127.0.0.1}`. A LAN peer who names the loopback host is admitted and not local (`scripts/install-page.test.mjs:513`). `Origin` can only fail the cross-site check. A mapped LAN address is not in that peer set. The LAN socket is IPv4 `0.0.0.0`; the extra listener is `::1` only. `0.0.0.0` is neither a loopback `Host` nor a TCP peer. A forwarding header on a loopback connection forces `local` false. A proxy that itself connects from loopback, rewrites `Host` to loopback, and sends none of those headers still looks local. That is the gate's existing definition of local, and a LAN peer's own socket cannot produce it.

**3. Leak.** The refusal is `notFound` (`scripts/tamber-server.mjs:117`, `:497`) before `admit`, `pair`, `principalOf`, and `wasRevoked`. Same status and body for a live code, six wrong codes, a live token, and a revoked token. `pairs.expire()` runs first and does not read the request. Keeping this 404 is right for the gate: it is the answer a token-less `POST /__tamber` already returned, and the answer `GET /__tamber/token` already gives a LAN peer. On `exact tamber serve`, `listen()`'s own miss is that same `notFound`. The dev loop's empty 405 is only for a path `handle` does not claim. Matching it would make the wire paths look unlike the token page.

**4. Paths left open.** Right.

| Path | Who reaches it on plaintext `--lan` |
|---|---|
| `/__tamber/token` | Loopback `GET` only (`:488`) |
| `/__tamber/tool`, `/__tamber/tool/…` | Loopback only (`scripts/tamber-tool-files.mjs:21`). The door serves those static files itself |
| Any other `/__tamber/…` | `handle` returns false. `POST` is an empty 405 |
| `/__dev/report`, `/__dev/hello`, `/__dev/commands` | A phone or Linux build. A report needs a stream nonce; it does not mint a token or run a wire method |
| `/__dev/page`, `/__dev/design`, the app | The page on `--lan` |
| `/__dev`, `/__dev/open`, `/exact.json`, `/__dev/generation/…` | Native client, forwarded to resident producers |

`--via` stays loopback-to-loopback. `tamber op` and `tamber pair` still take the token from `/__tamber/token`. A phone design tool uses the TLS door.

**5. Test.** `scripts/tamber-pairing-server.test.mjs:205` proves the gap at `handle`: one live code and six wrong ones, then `hello`, `events`, and `subscribe` with a live paired token, all 404, then that same code pairs through the door from `192.0.2.40`. Six misses is past both the per-peer five and the burn's five, so a 200 on the redeem means neither the limiter nor the code was touched. `fromPeer` as `{local:false, remote:true}` (`:52`) is right. Those tests are a paired client, which now exists only after the door, and `pair()` keys off `access.peer`, not `access.remote`. The "local token is no one" case still sets `remote` or `tunnel` on its own (`:173`).
