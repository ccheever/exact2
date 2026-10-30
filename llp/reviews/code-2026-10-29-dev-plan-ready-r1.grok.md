# Code review: dev loops say ready once listening, round 1 of 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `cb58fa60f5`.
- **Method:** one brief (sha256 `a2e0b709d9786db74a88fd7464ea8356dc3f7f8504af2a24d2712f0a5bae57c8`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

**MAJOR** — `host/web/tests/dev-plan-ready.test.mjs:28` (same hole at `:69`–`:78`). The wasm and TypeScript loops call `startCompiler()` at `host/web/dev.mjs:985`, then only later `listenDelay()` at `:1488`. The 20s wait overlaps the compile; it does not contain it. `sayReady` changes anything only when a ready line is produced while `listening` is still false. A cold `cargo run --release` (theme-fixture; `scripts/metrics.mjs` already budgets 120s for that) or a slow canvas-gallery produce finishes after the delay and after `server.listen`, so both the connect-on-cue test and the taken-port test pass with the old immediate `console.log`. Load makes the compiler slower, so a regression goes green. The JS case is sound: its old `plan ready` sat before `listenDelay(3000)`, and a refused connect does not ride out that gap.

**MINOR** — `host/web/dev.mjs:573`. A later contract plan still `console.log`s `edit → plan ready …` at once. `scripts/design-tool-inspect.test.mjs:35`, `scripts/tamber-server.test.mjs:528`, `host/web-js/dev-gallery.test.mjs:47`, and `host/web/tests/tables.test.mjs:106` treat `includes('plan ready')` as the cue. A second plan while the first `plan ready:` is still queued makes them connect before the listen. The anchored consumers (scheduler, resident watcher, `scripts/metrics.mjs`) do not match that line. Startup emits one plan; this is a save in the pre-listen window.

**MINOR** — `host/web/dev.mjs:1501`. The wasm cue is flushed inside the IPv4 listen callback. `openRemoteCanvas` runs after that (`:1502`) and on failure logs `cannot listen`, closes the server, and exits. The JS loop prints `plan ready` only after that await returns (`host/web-js/dev.mjs:1157`–`:1164`). With no `--remote`, `--trust-tunnel`, or `--relay`, doors return immediately. A wasm door bind failure still advertises readiness and then drops the server.

**MINOR** — `host/web/tests/dev-plan-ready.test.mjs:58`. The probe is only `http://127.0.0.1`. `[::1]` is bound in `alsoOnIPv6Loopback` before the cue, but the test stays green if the cue moves to after the IPv4 listen and before `[::1]`. That is the address the design tool's canvas uses (`localhost`).

**NIT** — `host/web/tests/dev-plan-ready.test.mjs:41`. `hear` appends stdout and stderr into one string and runs `^` on that blob. A stderr chunk between two pieces of one ready line hides it (the wait then runs to 600s). Buffer each stream by line, as `scripts/metrics.mjs` does.

**NIT** — `host/web/tests/dev-plan-ready.test.mjs:66`. On the 620s timeout, bun skips `finally`, so `exact-plan-ready-dist-*` (and the taken-port holder) are left behind. `afterEach(stopAll)` still reaps the child. The no-delay unit check at `:50` (`< 100` ms of wall time) can also fail under load when `listenDelay` returned immediately.

No lost or double cue on the flush: `listening = true` and `readyLines.splice(0)` run in one listen turn, and `sayReady` is either a push or a log. `sayReady` is initialized at `host/web/dev.mjs:522` and `startCompiler()` is at `:985`, so nothing calls it before it exists. Non-early EADDRINUSE on `[::1]` and on `127.0.0.1` exits without flushing. `servedAs` skips IPv6, still flushes on the IPv4 listen, and the resident watcher sees that line. `--lan` uses the same callback on `0.0.0.0`. Early mode (`listening = !!early`) prints immediately because `buildingServer` is already bound; that matches the stated rule. `dev-scheduler.test.mjs` is not in the diff. `listenDelay` returns unless `EXACT_TEST_TIMING=1` and the delay is a whole number of milliseconds from 1 to 60000, and it writes only the stderr line.

Verdict: The listen gate itself is right and the JS proof holds; the wasm/TS proof does not, so this is not clean.
