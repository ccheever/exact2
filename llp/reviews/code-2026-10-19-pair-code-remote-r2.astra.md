# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aa68d0a996`.
- **Method:** one brief with a security pass (sha256 `bd580d2201194edcf5fa015190e03b1ff64c0dd51f625548ba4e136375a9aa59`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Nothing blocks this field change.** I found no new authorization bypass or credential leak.

- **should-fix — [scripts/tamber-remote.test.mjs:437](/tmp/rv-pr-2/scripts/tamber-remote.test.mjs:437):** The certificate check closes its TLS connection; `send()` then sends the code on another connection with `rejectUnauthorized: false`, checking that connection’s fingerprint only after the response. This proves fingerprint equality on the successful path, but not pinning before disclosing the code. **Fix:** send the pairing POST on the verified socket, and add a mismatched-pin case asserting that no pairing request reaches the server and the code remains usable.

- **nit — [docs/tamber-protocol.md:202](/tmp/rv-pr-2/docs/tamber-protocol.md:202):** “The one path a remote request reaches without [a token]” still contradicts the static-tool exception corrected elsewhere in round 1. **Fix:** say “the only unauthenticated protocol operation,” with static tool GET/HEAD requests explicitly excepted.

The remaining checks look sound:

- **Shape and access:** `{origin, fingerprint}` and `{origin, tunnel: true}` match the agreement. `remote` appears only on successful local-token `pair-code` answers and is absent for an empty list. Paired clients, including `write` clients on loopback, cannot mint codes. Remote doors reject the local token. The fingerprint is public certificate metadata; learning it grants no authority.
- **Wiring:** all three getters capture initialized `doors` variables. Failed opening closes previously opened listeners without assigning a door object. The `--serve-as` child creates neither its own wire nor remote doors.
- **CLI and docs:** output contains the intended pairing code and public connection information, no bearer token or private key. `/__tamber/tool/` is routed on both TLS and tunnel origins. The revised tunnel wording distinguishes the server’s target from the other machine’s forwarded address. The Pairing shape and additive 1.13 note match the implementation.
- **Test assertions:** fingerprint equality is established transitively through the advertised field. The added `403/needs: write`, `403/needs: local`, and exact `404` assertions establish scope and local-token refusal without accepting socket errors.

Other useful missing cases are TLS-only/tunnel-only responses, the configured getter before opening, cleanup after a partial bind failure, and CLI output.

`bun test scripts/tamber-remote.test.mjs`: **7 passed, 13 failed during setup** because loopback binds or temporary-file creation failed in this sandbox. The TLS end-to-end test did not execute. No files changed.
