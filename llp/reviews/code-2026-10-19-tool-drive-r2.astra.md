# Code review: The design tool's direct drive and verify on the checkout's own port (scripts/tool-drive.mjs), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6f08d91bbf`.
- **Method:** one brief (sha256 `45e120903b3d3a32772cecd9d71e6e35c1037d63302ec1577d87df807569b96c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one P2 (the integration test's server was in the process spawnSync blocked, so it could pass for the wrong reason), one P3 (a fixed port)
- **Disposition:** both taken in the next commit: the port held by a process of its own on a free port, each refusal's text and exit asserted, the stage removed after

1. **P2 — Integration test can pass for the wrong reason** ([line 58](/tmp/rv-tooldrive-2/scripts/tool-drive.test.mjs:58)). [`spawnSync` blocks](https://bun.sh/docs/runtime/child-process) the thread hosting `Bun.serve`, so `answering()` times out. Verify then reaches the build; afterward, the stub’s bind failure can produce a diagnostic containing `127.0.0.1:17994`, satisfying both assertions. Agent can pass through the same startup failure despite `--reset-storage`. Use asynchronous children or a separate server process, and assert exit status plus the specific “already answers” refusal. Reject timeouts explicitly; the current 120-second limit does not establish an early refusal.

2. **P3 — Fixed port breaks concurrent-test isolation** ([line 54](/tmp/rv-tooldrive-2/scripts/tool-drive.test.mjs:54)). Runs in different checkouts both bind 17994 without coordination, so one fails during setup. Allocate an available port. Leaving the bounded stage under ignored `target/` is not itself blocking.

**Fix #1 before landing.** No production-routing blocker found: precedence, help handling, and rewriting relative or symlink paths to `design-tool` look correct—the absolute stage directory remains authoritative.

Read-only review; no builds or tests run.
