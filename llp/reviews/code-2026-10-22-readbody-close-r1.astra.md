# Code review: readBody settles on its connection's close, quick review 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** a quick review brief (one round each), the author (Claude, rt1) not a reviewer.
- **Transcription:** the final message, unedited.

- **Blocking — [scripts/tamber-request.mjs:44](scripts/tamber-request.mjs:44):** The socket listener survives successful reads. Each request on a long-lived keep-alive socket retains another callback and its closure until disconnect. Remove the named socket handler in `finish` on every settlement. The `settled` guard correctly prevents a later close from changing a completed text/raw result.

- **Should-fix — [scripts/tamber-request.test.mjs:23](scripts/tamber-request.test.mjs:23):** The TLS handoff regression is meaningful, but disconnect after the server observes partial data instead of relying on 200 ms. Add consecutive successful reads on one socket and assert listener counts return to baseline.

- **Nits:** None.

`body()` and the raw caller already accommodate `undefined`. Canvas uses a separate, unchanged reader at [scripts/tamber-canvas.mjs:244](scripts/tamber-canvas.mjs:244); this fix does not cover it.

Test attempted; sandbox `EPERM` at `listen()` prevented verification.
