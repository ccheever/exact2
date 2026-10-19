# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `912c238c25`.
- **Method:** one brief with a security pass (sha256 `1ed865d23c5235500b15b25a33d9b0927cd947f310d96d2d7e987eb52d8de701`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Nothing blocks this change.** No blocker or should-fix findings.

- **nit — [scripts/tamber-remote.test.mjs:442](/tmp/rv-pr-3/scripts/tamber-remote.test.mjs:442):** The mismatched-fingerprint branch remains untested. Add an attempt with a deliberately wrong pin, assert that no pairing POST reaches the server, then redeem the same code using the correct pin. This would protect the refusal behavior alongside the successful pairing case.

The five requested checks look sound:

1. **Shape and access:** `remote` contains `{origin, fingerprint}` and/or `{origin, tunnel: true}`, only on successful local-token `pair-code` answers; an empty list omits the field. Paired tokens—including `write` tokens on loopback—cannot obtain it through `pair-code`. Both remote doors reject the local token. The fingerprint is public certificate metadata; knowing it grants no authority.

2. **Wiring:** All three getters capture initialized `doors` variables. Before opening completes they return `[]`. Bind failures close previously opened listeners and trigger shutdown without publishing a door object. The wasm `--serve-as` child creates neither its own wire nor remote doors.

3. **CLI:** Output contains the intended pairing code and connection information, no bearer token or private key. `/__tamber/tool/` is routed on both TLS and tunnel origins. The tunnel wording correctly distinguishes the server’s loopback target from the other machine’s forwarded address.

4. **Documentation:** The Pairing shape, static-tool exception, and additive `tamber/1.13` note agree with the implementation.

5. **Test:** The certificate check now precedes sending the code **on the same socket**. Together with the advertised-field equality assertion, it checks the intended fingerprint. Exact `403/needs: write`, `403/needs: local`, and `404` assertions establish scope enforcement and local-token refusal without accepting socket errors.

Other optional coverage remains: TLS-only/tunnel-only field shapes, the getter before opening, partial-bind cleanup, and CLI output.

`bun test scripts/tamber-remote.test.mjs`: **7 passed, 13 failed during setup** because the sandbox blocked loopback listeners or temporary-file creation. The TLS end-to-end assertions did not execute here. No files changed.
