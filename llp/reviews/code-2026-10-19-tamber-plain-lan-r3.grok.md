# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 3 of 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, a throwaway worktree at `719f96bc55`.
- **Method:** one brief (sha256 `91a0b9fa6a38fe3ba943a93bf725f1d4aa56f66190b73a3bc3397bc320c878e5`), shared by both reviewers, blind to the other review; commit 719f96bc55, with theme2's ask: every other path a paired token is honoured on, for the same transport rule. A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve.
- **Disposition:** nothing to take.

I'll review HEAD as a read-only security pass: how the wire is reached, what `access` each caller passes, and whether a paired token is honored anywhere else on plaintext.The commit only gates four paths inside `handle`. Next I’ll trace every caller and every place a paired token is still accepted.No findings. Plain HTTP from a non-loopback peer still cannot pair or present a paired token, and no other path honors that token.

The gate in `handle` (`scripts/tamber-server.mjs:493-497`) runs before `pairEndpoint`, `principalOf`, and `wasRevoked`. A refusal is `notFound` (404 `{message:'Not found.'}`), with no limiter hit, no code compare, no `lookup` (so `lastSeen` does not move), and no 401 for a revoked token.

**Callers and `access`.** Every production caller passes either `developmentGate().check(req)` or an object the door builds itself.

- `host/web/dev.mjs:1336` and `host/web-js/dev.mjs:866` pass `gate.check(req)` on the socket that arrived. `gate.check` returns only `{allowed, local}` (`scripts/install-page.mjs:293`).
- `exact tamber serve` does the same (`scripts/tamber-server.mjs:1149`).
- `remoteHandler` sets `{local:false, remote:true, tunnel}` from which listener accepted the socket (`scripts/tamber-remote.mjs:164`), not from a header. The tunnel listener is `127.0.0.1` and `::1` only.
- `addPort` is TLS-only and stamps `remote:true` (`scripts/tamber-remote.mjs:274-286`). Nothing in this tree calls it except its test. There is no `canvas` method in `METHOD_SCOPE`.

`X-Forwarded-For`, `Forwarded`, and `X-Forwarded-Host` force `local` false. They never set `remote` or `tunnel`.

**Forwarding.** `/__tamber`, `/__tamber/pair`, `/__tamber/subscribe`, and `/__tamber/events` are handled on the JS loop with the original peer and return before `forward()`. The resident loop is started with `--serve-as`, so it mounts no `tamberServer` and binds `127.0.0.1` even when the parent passed `--lan` (`host/web/dev.mjs:59`, `399`). What it does forward (`/exact.json`, `/__dev`, generations) is not the wire. `--via` (`scripts/tamber-server.mjs:1351-1354`) is `127.0.0.1:7740` only and drops the request unless `access.local` before it copies the token upstream.

**`access.local`.** It is the gate's whole loopback test: admitted, no forwarding headers, `Host` exactly `127.0.0.1:port` or `localhost:port`, and peer in `{127.0.0.1, ::1, ::ffff:127.0.0.1}`. A LAN address, `::ffff:` of a LAN address, `0.0.0.0`, a spoofed `Host` or `Origin`, and `--allow-host` all leave `local` false (already covered in `scripts/install-page.test.mjs`). IPv6 listen is `::1` only (`host/web/serve.mjs:1055`).

**Other paired-token readers.** `pairs.lookup` in production is only `principalOf` (`scripts/tamber-server.mjs:477`), and that runs only for `POST` on the three wire paths after the gate. Header readers elsewhere are clients (`app.ts`, the CLIs) or `--via` copying the header after the local check.

| Path | Paired token? | Plaintext off-box |
|---|---|---|
| `/__tamber`, `/subscribe`, `/events`, `/pair` | yes, after the gate | refused |
| `/__tamber/token` | no; local token, `access.local` only | refused (already) |
| `/__tamber/tool` | no; `serveTool` requires `access.local` | refused. TLS door and trusted tunnel serve the static files with no token (`tamber-remote.mjs:156`) |
| `/__dev`, `/__dev/page`, `/__dev/report`, `/__dev/hello`, `/__dev/commands` | no; stream nonce (`tamber-builds.mjs:345`) | still open on `--lan`, which is the phone build channel |

A phone build over `--lan`, the design tool on the TLS door, `--via` from this machine, and `tamber op` / `tamber pair` (they take the local token from loopback) are unchanged.

**Test** (`scripts/tamber-pairing-server.test.mjs:205-222`). Same peer, live code, six wrong codes, `hello`, `events`, and `subscribe` all 404, then that same code pairs once `remote:true` is set. That is past both the five-attempt limiter and the five-fail burn, so nothing was spent. `fromPeer` setting `remote:true` matches the other tests: they are a client coming through the door. Posts that run after `fromPeer` clears `flags` are real loopback (`gate.check`), which is allowed to hold a paired token. The `{local:false}` cases that remain (`clients`, the new test) expect 404.

The JSON 404 versus the dev loop's empty 405 is the same `notFound` the token page already returns to a LAN peer. Left as the agreed NIT.
