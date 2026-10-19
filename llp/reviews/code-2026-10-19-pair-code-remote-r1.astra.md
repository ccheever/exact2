# Code review: pair-code names the remote doors open, with the TLS certificate's fingerprint (tamber/1.13), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6f51896504`.
- **Method:** one brief with a security pass (sha256 `0dcdafd714dc8e8f1bc496004732057690e2315aee93e952b5a429ef62fd4f60`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Nothing blocks this commit.** No new authorization bypass or credential leak found.

- **should-fix — [scripts/tamber-pairing.mjs:339](/tmp/rv-pr-1/scripts/tamber-pairing.mjs:339):** The tunnel branch prints a bare origin whose `/` returns 404. `/__tamber/tool/` exists on both doors. Also, this `127.0.0.1` address names the server’s tunnel target, not necessarily the remote machine’s forwarded address. Print the tool path, label it as the server-side target, and tell the user to open that path through their forwarded origin. For TLS, explicitly say to verify the fingerprint before submitting the code.

- **should-fix — [scripts/tamber-remote.test.mjs:445](/tmp/rv-pr-1/scripts/tamber-remote.test.mjs:445):** `status !== 200` also passes when `send()` returns a connection error with no status. Assert **404** and the expected refusal body. The preceding `hello.scope` assertion proves the scope is reported, but does not exercise enforcement: add a write request expecting `403/needs: write` and paired `pair-code` expecting `403/needs: local`. Existing A2 tests cover those restrictions separately; this would establish them over the actual TLS door.

- **nit — [docs/tamber-protocol.md:225](/tmp/rv-pr-1/docs/tamber-protocol.md:225):** The existing “no tool” and “only … pair … without a paired token” statements contradict the unauthenticated static tool route and the new CLI link. Qualify this as the only unauthenticated **protocol** operation and name the static-file exception; reconcile the transport table at line 128 too. The new Pairing shape and additive 1.13 note otherwise match the implementation.

The field is added only to a successful, locally authenticated `pair-code` response and omitted for an empty door list. Paired clients—including `write` clients—cannot obtain it through that operation, even on loopback. Remote doors reject the local token. The fingerprint itself is public certificate metadata, so learning it independently through TLS grants no privilege. The CLI prints no bearer token or private key.

The three getters capture initialized `doors` variables; there is no TDZ. Failed opening closes previously opened listeners and triggers shutdown without publishing a door object. The wasm `--serve-as` child creates neither its own wire nor remote doors.

The new test **does prove certificate-fingerprint equality**: it compares the socket’s peer certificate with the value asserted in the response. It does **not prove pinning before pairing**: `send()` disables certificate verification and checks the fingerprint after sending the code. To test that guarantee, compare the advertised pin before sending HTTP bytes and add a mismatched-pin refusal case. Other useful missing cases are TLS-only/tunnel-only response shapes, getter initialization, failed-open cleanup, and CLI output.

`bun test scripts/tamber-remote.test.mjs`: **7 passed, 13 failed during setup**—loopback binds and temporary-file creation were blocked by the sandbox. Network behavior remains unverified here. No files changed.
