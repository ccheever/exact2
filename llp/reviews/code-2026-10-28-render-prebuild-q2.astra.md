# Code review: document-render.test's warm-up build (main red), narrow re-check, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `9b98519547`.
- **Method:** a narrow brief naming the earlier findings and their fixes; a quick review (a mechanical main-red fix, test only), then a narrow re-check. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

CLEAN.

- [document-render.test.mjs:12](host/web-js/document-render.test.mjs:12) warms the same Caltrain WASM module and `caltrain-render`, including development trust.
- The returned promise is awaited under the independent 30-minute hook timeout. [Bun 1.4.2 runner](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/runtime/test_runner/Execution.rs)
- Build failures propagate with status, stdout and stderr; `finally` removes the fixture.

Working tree unchanged. Your warm/cold results were not independently reproduced; the accidental test-discovery run was stopped after sandbox write failures.
