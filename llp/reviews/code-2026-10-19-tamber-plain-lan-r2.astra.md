# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 2 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `719f96bc55`.
- **Method:** one brief (sha256 `8291fddca3f066e1d0a6e040d317a5a9e5644072a0acaed7d21b5ad31a41a0c8`), shared by both reviewers, blind to the other review; commit 719f96bc55 (round 1 taken). A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 NIT, known).
- **Disposition:** nothing to take (the known fingerprinting NIT stays as round 1 says).

No new **BLOCKING / SHOULD-FIX** findings. The direct-LAN plaintext gap is closed.

- **NIT, known — fingerprinting:** [tamber-server.mjs:497](scripts/tamber-server.mjs:497) returns JSON 404, distinguishable from the loops’ empty 405 for unknown POSTs. I agree this need not block D12: it refuses transport access, reveals no credential validity, and matches the existing token refusal. It does not provide scanner indistinguishability.

What checks out:

- **Callers and flags:** Both dev loops and the standalone listener pass `developmentGate().check(req)`. Only [tamber-remote.mjs:164](scripts/tamber-remote.mjs:164) supplies remote flags: through TLS, or the explicitly enabled loopback tunnel listener. Host, Origin and forwarding headers cannot manufacture those flags. No canvas gateway is implemented at HEAD; its `addPort` hook is TLS-only. Test callers use the gate or explicit simulated verdicts.

- **Forwarding:** [web-js/dev.mjs:870](host/web-js/dev.mjs:870) handles the wire against the original socket before forwarding native requests. Forwarding **does lose the original peer**, but the [resident loop has no Tamber server](host/web/dev.mjs:399); its reverse relay uses only build-channel paths. [`--via`](scripts/tamber-server.mjs:1354) requires the original request to be local before forwarding.

- **Local classification:** [install-page.mjs:293](scripts/install-page.mjs:293) requires admission, loopback Host, actual loopback socket peer, and no recognized forwarding header. Spoofed Host/Origin, mapped LAN IPv6 and `0.0.0.0` do not qualify. Existing limitation: an external proxy that connects over loopback, supplies a loopback Host and omits forwarding headers appears local; this gate cannot recover its original peer.

- **Refusal and tests:** The guard precedes body reads, token lookup, revoked-token responses, pairing admission and code comparison. Expiry maintenance runs beforehand on *every* path; I found no new credential-dependent timing branch. The [new test](scripts/tamber-pairing-server.test.mjs:205) covers all four paths and same-peer redemption after six misses. Changing `fromPeer` to `remote:true` correctly models the authorized remote listener, though the test itself does not exercise TLS.

- **Exclusions/regressions:** `/__tamber/token` and `/__tamber/tool[/…]` retain their separate local restrictions; remote listeners serve public tool files separately. `/__dev/report`, `/__dev/hello`, `/__dev/commands`, build streams and `/__dev/design` remain available for LAN phone builds, with reports bound to stream nonces. Agent drivers are unaffected. The phone design-tool client at HEAD already targets fixed loopback and has no remote pairing flow; this commit introduces no regression there.

Read-only verification passed **100 gate cases and 57 refused wire requests**, including zero limiter/code/token-lookup calls, zero body reads, and successful same-peer redemption. Socket tests were blocked at bind (`EPERM`/`EADDRINUSE`); the file-creating pairing suite was not run. No files changed.
