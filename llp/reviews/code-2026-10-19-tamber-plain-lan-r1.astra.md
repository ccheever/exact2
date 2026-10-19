# Code review: plain HTTP from off this machine reaches nothing of the wire (stage 10 E's finding), round 1 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e4b3a0c265`.
- **Method:** one brief (sha256 `3dfc85ac5fc405195f37d891bcfd34456d8ca3d68b4686d7547ba90f199e13a9`), shared by both reviewers, blind to the other review; commit e4b3a0c265 (the fix as first committed). A security pass (wire). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in `719f96bc55`: the test sends six wrong codes from the plain peer and redeems the live code through the door from the same peer, so a spent limiter slot or code attempt would fail it (both reviewers). Not taken: the JSON 404 differing from the loops' empty 405 (Astra SHOULD-FIX, fingerprinting) — it is notFound's, the answer /__tamber/token already gives a LAN peer; the loops' refusal convention is outside this fix (graded NIT in rounds 2 and 3).

No **BLOCKING** bypass found in the repository’s callers.

- **SHOULD-FIX — refusal remains fingerprintable.** [scripts/tamber-server.mjs:497](scripts/tamber-server.mjs:497) returns JSON 404, while an unknown POST gets empty 405 in both dev loops ([JS:946](host/web-js/dev.mjs:946), [wasm:1356](host/web/dev.mjs:1356)). GET responses also differ in body/content type. A scanner can identify Tamber’s presence. This inherits the existing refusal convention; it leaks no credential, but does not match “no such path.”

- **NIT — “nothing spent” is only partly tested.** [scripts/tamber-pairing-server.test.mjs:218](scripts/tamber-pairing-server.test.mjs:218) retries through `fromPeer`, which changes the peer address. It proves the code remains redeemable and the token survives, but cannot detect a consumed limiter slot or failed-code attempt. Send at least five wrong-code plaintext requests, then redeem through the door using **the same peer**. Changing `fromPeer` to `remote: true` is correct.

What checks out:

- Both dev loops and standalone `tamber serve` pass the development gate’s verdict. Runtime `remote`/`tunnel` flags originate only in the TLS/trusted-tunnel listeners, never headers. No canvas gateway caller exists at HEAD; `addPort` requires TLS.
- The JS loop handles Tamber **before** native forwarding ([dev.mjs:870](host/web-js/dev.mjs:870)). Forwarding loses the original socket peer, but the resident loop disables Tamber under `--serve-as`; its return relay uses build-channel routes only. `--via` checks locality before forwarding.
- `access.local` uses the [whole gate test](scripts/install-page.mjs:275): admitted Host/Origin, exact loopback socket address, and no forwarding headers. Spoofed Host, Origin, X-Forwarded-For, mapped LAN IPv6, or `0.0.0.0` cannot establish locality. A headerless proxy connecting through loopback remains indistinguishable from a local client—a pre-existing trust limitation.
- Refusal precedes body reading, pairing admission, code comparison and token lookup. Valid, invalid and revoked credentials therefore get the same refusal. The preceding expiry sweep also runs for unknown paths.
- `/__tamber/token` and main-listener `/__tamber/tool[/…]` remain local-only; remote doors separately serve public tool assets. `/__dev/report`, `/__dev/hello`, `/__dev/commands`, `/__dev`, `/__dev/page` and `/__dev/design` remain available for LAN builds. Phone build reporting, local tools, `--via` and agent drivers retain their paths; plaintext phone **wire** access is intentionally removed.

Verified with 69 in-memory assertions and both request handlers with stubbed build services. The file-writing fixture suite was not run. No files changed.
