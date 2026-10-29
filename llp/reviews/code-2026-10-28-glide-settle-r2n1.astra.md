# Code review: web clock settle waits for a glide, narrow re-check 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fd493229d1`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [host/web/glue.js:1252](host/web/glue.js:1252):** Round 2’s second finding remains open. A `then` can start a browser-owned scroll during `applyBatch`, with its first event queued for the next frame. When no collection owes a scroll, nothing guarantees a frame before this comparison; the unchanged counter permits `settled:true` before scrolling is observed.

- **SHOULD — [host/web/glue.js:1252](host/web/glue.js:1252), [host/web-js/agent.js:505](host/web-js/agent.js:505):** Both targets can report success after exhausting 16 non-quiet rounds. Wasm explicitly bypasses the counter check with `rounds >= 15`; JS rejects `!quiet` only for collection scroll events. Non-collection scrolling therefore falls through to `settled:true`. Exhaustion should return false.

Both paths reproduced with in-memory probes of the current functions. Browser integration tests were not run; the tree is unchanged.
