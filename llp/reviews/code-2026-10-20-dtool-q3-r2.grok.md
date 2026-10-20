# Code review: the wasm client shell activates within a second without frames, once (LLP 1122 queue item (3)), round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `99e5216089`.
- **Method:** one brief (sha256 `a6bfdf850bba789237544d8cc2f402a4dba3b0e125a9b660ec6476e0e71a105a`), shared by both reviewers, each blind to the other's review; commits 1b12a9079a (round 1's) and 99e5216089 (its fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX, 2 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - (1) as Astra's 1: each milestone latched as it is heard, before the tail is cut.
  - (2) a first compile's `error:` line ends the wait at once; a renamed marker still fails closed, said so in the record.
  - (3) the header names LLP 1122's queue item (3) and its QUEUE entry as filed.

Nothing blocks. The three fixes that were taken match their records, and Grok's second finding was rightly left as-is. The once-guard in `glue.js` is unchanged and still holds.

Grok's second finding is the one the record declines. That decline matches the DOM standard. `dataset.moduleReady = 'true'` goes through the HTML `DOMStringMap` setter, which calls "set an attribute value". When the attribute already exists, that calls "change an attribute", which always does three things: save `oldValue`, store the new value, and "handle attribute changes". "Handle attribute changes" queues an `attributes` mutation record as its first step. Neither algorithm compares the old and new values. A second `'true'` set queues a second record, so `calls` counts sets. Chromium, which this test drives, follows that.

The other round-1 items are in place:

- The dev-loop wait is real. `plan ready:` is printed once, on the first plan the compiler actually publishes (`host/web/dev.mjs:569`, `announced` sticks). The page is opened only after that, so the HTML carries the generation, `dev.js` adopts it (`host/web/dev.js:279`), and the event-stream fetch waits for `exact.ready` (`dev.js:102`) and then stops because `committed` is already that generation (`dev.js:181`). A later plan is a second activation on purpose, and the count no longer includes it.
- The startup fixture runs the timer before the frames when asked (`host/web/agent.test.mjs:891-893`). The optional-modules test loops both orders (`:898`). Before either callback, `loads` is still empty (`:890`). After the timer has started activation, that between-frames check is skipped because the loads have correctly begun; promise identity, a single `activate` event, and one `./input-glue.js` load are still checked for both orders. The frame-first order still asserts `loads` is empty between the two frames.
- The queue entry matches the code. A first-page Canvas 2D op waits on nested frames (`host/web/navigation.js:501`), `pieces.pending()` returns that promise (`:509-511`), and `activateData` waits on it before `inputReady` (`host/web/glue.js:1274-1275`). `exact.ready` waits on that same settlement.

1. **SHOULD-FIX** — `host/web/tests/activation-noframes.test.mjs:41-44`. The wait requires both `127.0.0.1:${port}/` and `plan ready:` to sit in one 8000-character tail, and it only looks every 100ms. `plan ready:` is printed once. On a cold tree the sequence is: this test's fresh dist misses `currentWebBuild()`, so `build.mjs --wasm` runs to completion under the `logic-dev` profile (`host/web/build.mjs:291`); `startCompiler` then spawns `cargo run -q --release` of `theme-fixture-dev` (`dev.mjs:535`, stderr inherited); the listen banner prints while that release build is still going; rustc diagnostics after the banner push the URL out of the tail; the one `plan ready:` line arrives with the URL already gone. The loop then sits until 600s and throws `the dev loop never served`, after a plan that did publish. The old URL-only wait exited on the banner, so later compiler stderr could not erase it. Latch each marker on its own (as `lines()` does in `scripts/design-tool.test.mjs:1118`).

2. **NIT** — same wait, `activation-noframes.test.mjs:44`. A resident compiler that prints `error` and keeps running (`host/web/src/dev.rs:723`, logged at `dev.mjs:576`) never prints `plan ready:`. The loop does not open the page, so the count cannot pass by mistake, but it burns the full 600s and reports that the loop never served. A changed log line does the same: the colon is what distinguishes this line from `edit → plan ready` (`dev.mjs:570`), and a rename fails closed rather than double-counting.

3. **NIT** — `host/web/tests/activation-noframes.test.mjs:1-2`. The file header still quotes the old queue title, "The wasm client shell activates only after two animation frames". That entry is now the offscreen canvas surface.
