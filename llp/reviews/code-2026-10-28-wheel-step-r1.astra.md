# Code review: the test grammar's wheel step and Excord's macOS twins, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4db143ac74`.
- **Method:** one brief (sha256 `4639f9d68b32e5e443aae215c1b2b8813c16c44a8c1cd722b8dbf71abdca8dab`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **P2 should fix — source-file cap exceeded** — [apps/excord/app.test.contract:1221](apps/excord/app.test.contract:1221). The twins increase the file to 1,544 lines by the cap checker’s count, exceeding 1,500. `EXACT_CAPS_WASM=0 bun scripts/caps.mjs` fails with `cap:lines`. Split a test group into another `.test.contract` file.

2. **P2 should fix — `mac` alias loses both scenarios** — [apps/excord/app.test.contract:991](apps/excord/app.test.contract:991). The supported `mac` invocation now skips both originals and both twins: [agent-test.mjs:635](scripts/agent-test.mjs:635) leaves its scope as `mac`, which matches neither host list. Previously, the unscoped originals ran. Normalize `mac` to `macos` before filtering.

No other findings. Rust/native app tests were not run; no files changed.
