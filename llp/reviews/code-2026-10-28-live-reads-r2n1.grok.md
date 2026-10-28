# Code review: the clone's live reads on a local server, narrow re-check 1, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `05dec78dc8`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

**CLEAN.** The three fixes do not add a hang, a stuck clock count, a cross-page delivery, or a false pass.

The release stays tied to a wrapped `s.clock` step. `watch` replaces that method once per session (`scripts/live-reads-fixture.mjs:260-264`) and `runTests` calls it from `atStep` after `tap "arm"`, which is the first input of all six tests. Each test gets a new session from `launch` (`scripts/agent-test.mjs:573`). Every test sets `before data`, so the pre-step `data()` does not clock, and `arm` sends no mutation, so nothing calls `s.clock` before the wrap. Later steps, the drag hold (`s.clock('+300')`), the mutation clock, and reload's storage wait all go through that wrap. Web reload keeps the same session object, so the wrap survives `carrier.reset`.

A clock that skips `s.clock` does not hang these tests. An input's end is `carrier.ask({ op: 'clock', land: true })` (`scripts/agent.mjs:1261`), and both hosts return that after one land without waiting on in-flight fetches (`host/web-js/agent.js:419-424`, `host/web/glue.js:1225-1226`). `waitForBoot` waits for the boot mark and `exact.ready`, not for resource fetches. Answers asked by a tap sit in `withheld` until the next `clock data` starts, then wait 300 ms inside that step.

`clocks` cannot stick or go negative on these paths. `clocked()` increments before it returns `done`, and the wrapper's `finally` decrements once (`scripts/live-reads-fixture.mjs:254-263`). A throw inside `s.clock` still runs `finally`. Nested `this.clock` (settle's follow-up) awaits the wrapper, so the counts nest and unwind. `watch` is synchronous and idempotent, and steps are not concurrent.

An answer is one closure per request. A timer that fires after `done()` still answers that request; a reload aborts it with the old page. The next page's fetches are new requests and stay withheld until its own clock step. The server's `withheld` set is drained into those same closures, not into the new page.

A fetch asked outside a step does not start its 300 ms until the clock step that must wait for it. One asked during `clock +61000` starts then, and that jump waits for the send before it returns (`host/web-js/agent.js:511-520`). A driver gap before the step no longer spends the delay. The recorded breaks still fail all six tests.

The butterfly test moved unchanged into `live.test.contract` (lines 76-85) and is gone from `app.test.contract`. Its `newPages.0 == "discover"` assert still needs Discover to return posts, which is what arms `latestOn`. `liveReadsApp` removes the temp directory and rethrows if a write fails (`scripts/live-reads-fixture.mjs:271-278`).
