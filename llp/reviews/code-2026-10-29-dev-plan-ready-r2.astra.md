# Code review: dev loops say ready once listening, round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9f95444d5e`.
- **Method:** one brief (sha256 `dc3c0eecf46d8d4b3b30ac1962c115f2065625db17e2853356000e309093da1d`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/dev.mjs:519](host/web/dev.mjs:519):** `listening = !!early` bypasses the readiness gate for games. `buildingServer()` has bound only IPv4; compiler callbacks can run before the IPv6 bind at line 1485 or canvas doors at line 1498. A generation can therefore print “plan ready” before an IPv6 `EADDRINUSE` failure. Confirmed with an in-memory replay of the startup code. Hold readiness until the remaining listeners and doors succeed.

- **MINOR — [dev-plan-ready.test.mjs:110](host/web/tests/dev-plan-ready.test.mjs:110):** The negative assertion still uses merged raw chunks, bypassing the corrected per-stream parser. With stdout `"plan re"`, stderr `"diagnostic
"`, then stdout `"ady
"`, `run.ready()` is true but this assertion passes. Confirmed by replay. Assert the readiness tracker after the output streams close.

- **MINOR — [dev-plan-ready.test.mjs:23](host/web/tests/dev-plan-ready.test.mjs:23):** Cleanup removes the temporary dist but leaves the wasm generation cache. The loop keys `target/dev-generations/<hash>` by that randomly named dist, so the four wasm cases leave distinct caches that subsequent runs never reuse. Remove those owned caches after stopping the children.

Verdict: **Request changes.** The focused unit tests pass, but the game readiness exception still violates the failed-listen condition.
