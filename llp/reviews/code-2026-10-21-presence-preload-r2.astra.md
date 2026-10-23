# Code review: The wasm host preloads its presence module where the plan names a presence row, round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `775da7467b`.
- **Method:** one brief (sha256 `9b5562c92c58b995b5e45107f08e1341e7d07d3a84b16f5b12d6e6eb1720cf60`), blind to the other reviewer; 2 rounds (the lead's 23:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **P2 — Timeout can report false settlement.** [glue.js:1232](host/web/glue.js:1232) ignores which promise wins. With a stalled presence load and a held exit batch, `clock settle` can return `settled: true` before any held batch applies. Reproduced. **Fix:** distinguish timeout from completion and return `settled: false` when presence remains pending.

- **P2 — The wait exceeds ordinary agent calls’ timeout.** [glue.js:1194](host/web/glue.js:1194) waits 20 seconds, but ordinary Chrome agent requests have a 15-second CDP deadline. A stalled preload therefore makes `tree`, `state`, etc. fail before this fallback runs; the page continues processing afterward. **Fix:** align the page’s wait budget and carrier timeout, leaving headroom.

- **P3 — The regex still matches unrelated property values.** [navigation.js:554](host/web/navigation.js:554) fetches presence for `font-family:"--exact-layout-transition";`, which the wasm font serializer can emit. `\b` also accepts hyphenated suffixes. **Fix:** match complete declaration names followed by a colon, outside values. Canonical emitted presence declarations have no spacing or uppercase mismatch.

- **P3 — The new test has a timing race.** [presence-loader.test.mjs:57](host/web/tests/presence-loader.test.mjs:57) assumes two nested timers finish within 5 ms. A 20 ms event-loop stall reproduced `calls === 0` at that assertion. **Fix:** explicitly drive queued rAF callbacks and await load completion instead of fixed sleeps.

The 7 focused tests pass normally. Controlled checks found no permanently scheduled promise or duplicate load/retry during the scheduled `hold()` window. No files edited.
