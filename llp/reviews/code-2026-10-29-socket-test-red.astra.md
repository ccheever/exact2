# Code review: web-js socket test's missing-file 404 (main red), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `057cfd1808`.
- **Method:** one brief (sha256 `b402276f52e6b2b5a0b6116afc5f3d7f696200700c9a0625dc18e0c66d92b0e4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The fix is correct and test-only: [socket.test.mjs:244](host/web-js/socket.test.mjs:244) changes only the fixture server’s missing-file response. Socket behavior and assertions remain unchanged.

`exact.json` is legitimately absent from this non-production fixture. The [JS builder](host/web-js/build.mjs:677) copies it from a bake for production builds. The [agent’s probe](scripts/agent-inspect.mjs:66) discovers optional source maps and handles HTTP failures. Returning 404 therefore does not hide the suspected product bug here; missing essential runtime files would still prevent the socket assertions from passing.

No files edited, network calls, tests, or builds run.
