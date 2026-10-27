# Code review: tamber-server tests' dev loop tool port, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8261cf2321`.
- **Method:** one brief (sha256 `156604b522f20159d98ef4cdd34f924de7d15e3ced8bebfb4880c24a5d6cee50`); a quick review, one reviewer. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The fix is correct and complete for the reported mismatch. [tamber-server.test.mjs:439](scripts/tamber-server.test.mjs:439) overrides the inherited environment with the same cached `TOOL_PORT` used to construct `FRAME_ANCESTORS`. Both JS and wasm loops receive it; descendants inherit it.

I found no remaining path in this test file, its spawned children, or the in-process `tamberServer` that re-reads `EXACT_TOOL_PORT` and compares it against the cached framing constants. The in-process framing helper uses `FRAME_ANCESTORS` directly.

Standalone behavior is preserved: unset means `7740`; a valid user setting supplies both the expected header and child configuration. This does not bind the tool port—the dev listener still uses its separate `--port`. The harness’s environment remains intact for later design-tool files.

Static review only; no edits, network access, or tests run.
