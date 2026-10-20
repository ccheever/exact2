# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 3 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `719f96bc55`.
- **Method:** one brief (sha256 `91a0b9fa6a38fe3ba943a93bf725f1d4aa56f66190b73a3bc3397bc320c878e5`), shared by both reviewers, blind to the other review; commit 719f96bc55, with theme2's ask: every other path a paired token is honoured on, for the same transport rule. A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 NIT, known).
- **Disposition:** nothing to take.

No **BLOCKING** or **SHOULD-FIX** findings in `719f96bc55`.

**NIT — already agreed:** [tamber-server.mjs:497](scripts/tamber-server.mjs:497) returns JSON 404 versus the loops’ empty 405 for unknown POST paths, so fingerprinting remains possible.

What checks out:

- **All paired-token routes are covered.** The sole production `pairs.lookup` is inside `principalOf`, whose only caller follows the guard. Pairing, RPC, subscribe and events are refused before body reads, authentication, limiter admission or code attempts. The preceding expiry sweep also runs for unknown paths; it introduces no submitted-token/code timing oracle. [tamber-server.mjs:472](scripts/tamber-server.mjs:472)
- **Access comes from the listener.** Both dev loops and standalone `serve` pass the development gate’s result. Only TLS/trusted-tunnel handlers supply `remote`/`tunnel`; request headers cannot set them. [tamber-remote.mjs:164](scripts/tamber-remote.mjs:164)
- **Local means the complete gate verdict:** admitted Host/Origin, an actual loopback socket peer, and no recognized forwarding headers. Spoofing Host, Origin or X-Forwarded-For cannot turn a direct LAN peer local. Mapped LAN IPv6 addresses and `0.0.0.0` fail. An externally configured proxy that strips provenance and rewrites Host remains indistinguishable from local traffic—an existing boundary. [install-page.mjs:275](scripts/install-page.mjs:275)
- **Forwarding cannot bypass this guard.** JS handles Tamber against the original peer before forwarding. Forwarding itself loses that peer, but the resident `--serve-as` loop disables Tamber; its return relay only uses build-channel endpoints. `--via` requires locality before forwarding. [web-js/dev.mjs:870](host/web-js/dev.mjs:870), [web/dev.mjs:399](host/web/dev.mjs:399), [tamber-server.mjs:1354](scripts/tamber-server.mjs:1354)
- **Excluded paths are appropriate.** `/__tamber/token` and main-listener `/__tamber/tool/*` remain local-only; remote tool files are credential-free static assets. No canvas method/grant/gateway exists here; `addPort` requires TLS. `/__dev/hello`, `/__dev/commands`, `/__dev/report` and dev streams use separate nonces, never paired tokens, preserving LAN phone builds. Local tooling and agent drivers remain unaffected. [tamber-tool-files.mjs:20](scripts/tamber-tool-files.mjs:20), [tamber-remote.mjs:274](scripts/tamber-remote.mjs:274), [tamber-builds.mjs:345](scripts/tamber-builds.mjs:345)
- **Regression test is sound.** It covers all four paths and successful same-code, same-peer pairing afterward with a fixed clock. `fromPeer` adding `remote:true` correctly models an admitted door. [tamber-pairing-server.test.mjs:205](scripts/tamber-pairing-server.test.mjs:205)

Validation: three existing gate tests and 134 in-memory assertions passed. Six listener tests could not bind in this sandbox (`EPERM`/`EADDRINUSE`). No files changed.
