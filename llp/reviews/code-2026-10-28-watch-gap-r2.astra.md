# Code review: the design server's watcher: a read after a new watch, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e41a89fff0`.
- **Method:** one brief (sha256 `9f0ab4cc6be203a354299c2008c0a929c02a8f9093a14f4c70e8c26b2a0a7e85`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/apple/devices.mjs:162](host/apple/devices.mjs:162) — HEAD removes `EXACT_AGENT_PADS` from `forwardedEnv()`, breaking injected gamepads for iOS simulator and device drives. Both launch paths use this function, while native `ExactExtensions` still reads that variable; those drives now receive an empty pad list. Restore its forwarding alongside `EXACT_STORE`.

**Non-blocking**

none.

The round-1 blocker is resolved: the ordered `hello` drains earlier reads, and the test waits for injection and the round’s digest without requiring an empty snapshot. No remaining correctness issue found in the Tamber fix or test.

Static review only; no tests run.
