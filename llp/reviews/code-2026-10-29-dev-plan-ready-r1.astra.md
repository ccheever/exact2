# Code review: dev loops say ready once listening, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `cb58fa60f5`.
- **Method:** one brief (sha256 `a2e0b709d9786db74a88fd7464ea8356dc3f7f8504af2a24d2712f0a5bae57c8`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/dev.mjs:521](host/web/dev.mjs:521):** The additions push this file from 1,500 to **1,504 lines by `caps.mjs`’s counting**. The mandatory caps check will fail.

- **MAJOR — [dev-plan-ready.test.mjs:28](host/web/tests/dev-plan-ready.test.mjs:28):** The theme-fixture cases share `apps/theme-fixture/.exact/tamber.lock` with other tests in the parallel glue lane. A private dist does not isolate that lock. If this test acquires it first, the design-tool inspect test’s Tamber server fails to start. Take the existing fixture turn used by `scripts/design-tool-inspect.test.mjs:16`.

- **MINOR — [dev-plan-ready.test.mjs:28](host/web/tests/dev-plan-ready.test.mjs:28):** The wasm regression cases can pass without the fix when compilation exceeds the 20-second delay. The successful case then emits its old readiness line after listening; the taken-port case kills the compiler before it emits anything. Checking for the delay diagnostic does not establish that a generation completed during the delay.

- **MINOR — [dev-plan-ready.test.mjs:75](host/web/tests/dev-plan-ready.test.mjs:75):** The timeout returns `'still running'`, which passes `expect(code).not.toBe(0)`. A loop that logs EADDRINUSE but hangs during shutdown can therefore pass. I confirmed that these assertions accept the timeout sentinel. Reject on timeout and require a numeric nonzero exit code.

- **MINOR — [dev-plan-ready.test.mjs:20](host/web/tests/dev-plan-ready.test.mjs:20):** Timeout teardown stops children but does not remove temporary directories. `run.done()` appears only in `finally`, which Bun can skip on test timeout, as `test-children.mjs` documents. Register directory cleanup in teardown too.

Verdict: **Request changes.** Read-only review completed; isolated checks passed, but full integration tests were not run.
