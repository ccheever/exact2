# Code review: the wasm client shell activates within a second without frames, once (LLP 1122 queue item (3)), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `1b12a9079a`.
- **Method:** one brief (sha256 `e35d4eea4dd2f89ff751f4732abd10c3f72a528d6bbbe0d59b0b7bbdf9888a61`), shared by both reviewers, each blind to the other's review; commit 1b12a9079a. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX, 2 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - The test waits for the dev loop's first `plan ready` before it navigates, so a first generation published later can't restart the page inside the count (1).
  - The startup fixture runs the timer first, then the frames, in both orders, and checks one activation, the same promise, and one input-glue load (2).
  - QUEUE's entry adds Canvas 2D's first-page readiness (navigation.js's nested frames) (3).

No **BLOCKER** found in the activation change.

1. **SHOULD-FIX — The new test can count a legitimate dev reload as duplicate activation.** At [activation-noframes.test.mjs:43](/tmp/rv-dt-q3a/host/web/tests/activation-noframes.test.mjs:43), it waits only for the listening URL. The resident compiler can publish its first generation afterward ([dev.mjs:569](/tmp/rv-dt-q3a/host/web/dev.mjs:569)). If the page boots the baked plan first, then receives that generation during the observation window, its legitimate restart calls `activateData` again at [glue.js:1362](/tmp/rv-dt-q3a/host/web/glue.js:1362). Both counters become two despite a correct guard. Wait for `plan ready` before navigating.

2. **NIT — Exercise timeout-first activation followed by resumed frames.** [agent.test.mjs:893](/tmp/rv-dt-q3a/host/web/agent.test.mjs:893) always fires frames first; [activation-noframes.test.mjs:33](/tmp/rv-dt-q3a/host/web/tests/activation-noframes.test.mjs:33) permanently discards them. Neither exercises scrolling the frame back into view after timeout activation. Retain callbacks in the VM fixture, fire the timeout first, then both frames, and assert promise identity and counts. The browser’s 1.5-second sleeps also don’t establish that the competing callback and its asynchronous work completed; the VM assertions are the stronger proof.

3. **NIT — Extend the queued limitation to Canvas 2D readiness.** [QUEUE.md:1570](/tmp/rv-dt-q3a/QUEUE.md:1570) records GPU loading only. A frameless page initially containing a Canvas 2D surface still waits on nested rAFs at [navigation.js:501](/tmp/rv-dt-q3a/host/web/navigation.js:501). `pieces.pending()` includes that promise, so [glue.js:1274](/tmp/rv-dt-q3a/host/web/glue.js:1274) keeps input and `exact.ready` blocked even after `exact_data_ready` runs. This is pre-existing and suitable for the deferred follow-up.

The guard itself is sound: `activate` is async, so even a throw before its first `await` produces a retained rejected promise. Neither fulfillment nor rejection permits another activation. Served pages retain immediate activation; boot failures precede scheduling; reloads wait for `moduleReady` and intentionally activate their new runner separately. `agentMode` is already initialized and its exclusion preserves the existing agent behavior.

The timeout intentionally permits activation before a frame: `loadAfterPaint` injects scripts without independently waiting for paint. `frameCallbackMs` remains truthful, and `metrics.mjs` uses agent mode. The older unconditional paint assertions at [browser.rs:1051](/tmp/rv-dt-q3a/js/web/tests/it/browser.rs:1051) describe the frame-first path, not the newly allowed timeout path.

The exports copy preserves memory, tables and other function references; I found no behavioral problem there. This dev build is unsplit, however, so it doesn’t exercise stage imports. The permanently frameless case cannot pass the parent implementation through its ordinary startup path.

Read-only review; no builds or tests run.
