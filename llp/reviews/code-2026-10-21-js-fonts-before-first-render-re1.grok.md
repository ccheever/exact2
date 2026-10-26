# Code review: the JS target's declared faces load before its first render (the clone's wasm-vs-JS widths), narrow re-check, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `e4eb83c739`.
- **Method:** one brief (sha256 `2550e8081da5a0cd579a834e2142c1d6d6b4b40215d2860db0a9d8300947c500`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (the JS target's boot, the lead's 00:00Z task), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**RIGHT**

`(a)` `clock.agent` is set synchronously in the generated entry while the font race is still pending (`build.mjs:436`), after `sounds()` and `ts()` arm their callbacks and before `go(start)` yields. `agentMode` (`rt.js:291`) is the same predicate `mount` now uses (`rt.js:1394`): `__exactRender` or `AGENT_ADMITTED` and `?agent`. `sounds.js` `load` (rAF, then a timeout) and `ts-data.js` `pageModule` (the `painted()` resolution reads `clock.agent`) both run only after this turn, so an agent page is already one for the whole wait. With no faces, `start()` still runs in that same turn and `mount` sets the flag before those callbacks. `gateAgent` (`build.mjs:521`) still sees `URLSearchParams(…).has("agent")` and `const AGENT_ADMITTED = true` together in `rt.js` and rewrites the constant to false; the entry only calls `agentMode()`.

`(b)` The fast case measures widths only when every hooked `load` finished at or before the first node (`fonts-boot.test.mjs:113`); otherwise it requires the ≥95 ms floor. The slow case holds the `.ttf` until after the render snapshot, with no ceiling, and requires `readyAt >= mounted`. A fast win without Inter fails the width checks; a timeout without the wait is under 95 ms. Holding the file means dropping the cap never reaches the selector. `done` is stamped in `finally` before the promise the boot awaits, and the observer is queued before `booted()`, so neither branch depends on a tuned race.

No findings.
