# Code review: feedback nudges and one-line notes, round 5 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `9294d18f70`.
- **Method:** one brief (sha256 `bfcf205620d3276b47ef29c757a17925dd11eab0b4400da6f01446e60890d06e`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** both findings (shared by both reviewers) are addressed in `247ad6a041` and `ef7c888a18`.

- **MAJOR — Relay drops output when the caller reads slowly.** [scripts/app-cli.mjs:209](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:209). Forwarding does not wait for downstream writes to finish. Once child stderr ends, `spawned()` resolves and the generated launcher calls `process.exit()`, discarding pending output. **Reproduction:** a read-only probe executed the unchanged `spawned` helper with a child synchronously writing 16 MiB to stderr and exiting 37; the caller paused reading for 500 ms. Only **655,360 bytes** arrived. The inherited-stderr control delivered all **16,777,216 bytes**. Both returned 37. The new test reads promptly, so it misses this regression. **Fix:** honor downstream backpressure and finish pending stderr writes before returning to the launcher. Add a slow-consumer regression asserting complete, ordered output.

- **MAJOR — Truncated single-quoted values escape masking.** [scripts/feedback-note.mjs:82](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.mjs:82). Closed single quotes are recognized, but the unmatched-quote fallback handles only double quotes and backticks. The new capture limit at [scripts/app-cli.mjs:164](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:164) can remove a closing single quote before sanitization. **Reproduction:** evaluate `errorTail(("test: expected '" + "private payload ".repeat(30) + "'").slice(0, 240))`, then pass its result through `redact()`. The private payload remains verbatim; the equivalent double-quoted input becomes `test: expected …`. Such captured values enter notes and are sent under `always`. **Fix:** mask unmatched single-quoted values using the same opener rule as closed single quotes, and test truncation through the capture path.

Confirmed: the captured tail is bounded; exit codes and signal-code mapping are preserved. Read-only relay probes also confirmed that a child running beyond 300 ms completes normally, while a grandchild retaining stderr triggers the bounded wait only after child exit. Consent gates remain intact by inspection.

The permitted test command produced **2 passes**; **11 tests were blocked at temporary-directory creation by sandbox `EPERM`**, before their assertions.

VERDICT: CHANGES
