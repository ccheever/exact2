# Code review: The wasm host preloads its presence module where the plan names a presence row, round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `b2bb3da1a4`.
- **Method:** one brief (sha256 `031ce7f88732cda87a594cd6d057b134736884eb5951a6679dd12cc444ef10e8`), blind to the other reviewer; 2 rounds (the lead's 23:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

1. **P2 — First gain can still jump while preload is pending.** [glue.js:1324](host/web/glue.js:1324) starts loading without making activation wait. `activateData()` can apply a gaining batch and enable input while `presence.live` is null; the lost geometry is never recovered. An in-memory probe reproduced this. **Fix:** complete preloading before activating data/input or accepting a presence-using reload, without holding dispatched batches.

2. **P2 — Preload is not deferred until after paint.** [glue.js:1324](host/web/glue.js:1324) runs before the boot batch reaches the DOM. Despite its name, `loadAfterPaint()` immediately appends a module script. **Fix:** schedule through `afterPaint()`, and have `pending()` include that scheduled work.

3. **P2 — A stalled download blocks every agent operation.** [glue.js:1194](host/web/glue.js:1194) awaits presence without a timeout; [glue.js:1232](host/web/glue.js:1232) likewise ignores the clock deadline. A load that neither resolves nor rejects leaves requests pending until the carrier times out. **Fix:** bound both waits and return an explicit unavailable/unsettled result.

4. **P3 — Restart retries an unavailable loader while reporting no pending work.** [navigation.js:560](host/web/navigation.js:560) exposes `start` directly. After rejection clears `loading`, another boot starts another load, but `unavailable` remains true, so `pending()` returns null and `hold()` bypasses it. **Fix:** preserve terminal failure by guarding `preload`, or explicitly reset failure state for a tracked retry.

Separately, the **pre-existing no-presence loading issue** remains at [navigation.js:566](host/web/navigation.js:566): any `--exact-` CSS text triggers loading, including press feedback and tint. Narrow this to the two presence properties and exit operations.

No issue found in `needed()`’s coverage of literals, classes, component parameters or exit-animation bindings, or in the macro/private-field access.

Six focused Bun tests passed. Additional in-memory probes reproduced the findings. Rust and browser integration tests were not run in this read-only checkout. No files changed.
