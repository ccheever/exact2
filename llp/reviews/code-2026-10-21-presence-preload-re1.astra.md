# Code review: The wasm host preloads its presence module where the plan names a presence row, narrow re-check 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `937b82dcdd`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the lead's 23:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

(1) and (4) hold. `presence.landed(...)` preserves `this`, and its timeout timer is cleared on success, failure, and timeout. Remaining findings:

- **P2 — [glue.js:1194](host/web/glue.js:1194): Timeout result ignored.** `agentSettled()` continues to `agent(request)` when `landed()` returns `false`. Reproduced a successful layout reply while an exit batch remained held. Return an explicit failure before dispatching the operation.

- **P2 — [glue.js:1194](host/web/glue.js:1194): The 10-second wait does not bound the whole request.** It starts after `pieceLoad`. A 7-second piece load plus a stalled presence load reaches 17 seconds; CDP times out at 15 seconds, but the operation subsequently executes. Use the remaining time from a shared deadline.

- **P2 — [agent.test.mjs:234](host/web/agent.test.mjs:234): Existing agent tests now fail.** Their presence stub supplies only `live`. The new calls throw `TypeError: presence.landed is not a function`; focused runs reproduce this.

- **P3 — [navigation.js:554](host/web/navigation.js:554): Quoted values can still match.** `font-family: "A;--exact-layout-transition:x";` triggers a load despite declaring neither presence property. The regex treats a semicolon inside a string as a declaration boundary.

The four presence-loader tests pass.
