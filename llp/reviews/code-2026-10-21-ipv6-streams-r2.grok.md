# Code review: the dev loop's [::1] connections lose their requests' close under Bun, round 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `519c4d06862dd6cdb9d13ee14ae083fb2236e68c045143ed05faa9ce3b16d095`), shared by both reviewers, each blind to the other's review; HEAD `d6fc3d20cc`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks
- **Disposition:**
  Neither reviewer found anything blocking. Grok had no findings and confirmed each round 1 fix, the shutdown order under Bun 1.4.2, and the wasm loop. Astra's two test SHOULD-FIXes are taken post-round, in their own commit:
  - Astra 1 (the IPv6 probe asked the listener under test, so a missing [::1] listener would skip the localhost leg and pass): IPv6 is checked on a socket of the test's own; where it exists, the loop must answer on [::1] or the test fails.
  - Astra 2 (request errors swallowed when Bun marks the request destroyed first): errors reject unconditionally; after the client's own destroy the promise has already settled.
  Rerun: the unit test 3 of 3, the dev-loop test passing.

I'll start with the review skill and round-1 notes, then inspect the HEAD vs `3e8377ee5c` change without touching the tree.I'll load the reviewer persona and round-1 notes, then inspect the two commits without changing the tree.Reviewing the two commits and round-1 notes as read-only; I’ll only inspect, not change the tree.Round-1 notes and the fix diff are in; next I’ll read the current sources, the original change, and the wasm/dev-loop call sites.I'll trace both dev loops, upgrade listeners, and how `suiteBrowser` takes extra Chrome args.Bun’s `closeAllConnections` drops the native handle; I’ll check whether that still emits `close`, which the new test waits for.Nothing blocks. Round 1’s items are closed, and the follow-up changes hold together.

**Round 1**

- Unhandled `[::1]` upgrades are destroyed: `host/web/serve.mjs:1057`.
- The JS-loop test maps `localhost` to `[::1]` and runs that leg only when `::1` accepts: `host/web-js/dev-page-streams.test.mjs:41–44`.
- Readiness waits for the post-listen banner (`127.0.0.1:${port}/`) and `plan ready`: `host/web-js/dev-page-streams.test.mjs:39`.
- Stream-test teardown is in `finally`: `host/web/tests/ipv6-loopback.test.mjs:79–81`.
- The port probe waits for the companion’s `close`: `host/web/tests/ipv6-loopback.test.mjs:71–78`.

**Fixes**

A main-server `upgrade` listener runs inside `emit` and makes it return true, so the socket is left to that listener — the same rule Node and Bun use internally. Neither loop installs one.

Companion teardown is still `closeAllConnections` then `close`. Under pinned Bun 1.4.2, `closeAllConnections` calls `stop(true)` and `getBunServerAllClosedPromise` still emits `close`, which the unit test waits for.

The wasm loop (`host/web/dev.mjs`) uses the same helper. `/__dev` still drops the build on `req` `close`. It has no `upgrade` listener, so an unhandled `[::1]` upgrade is destroyed.

**Tests**

The unit test hits `::1` for client leave, unhandled upgrade, and shutdown. The browser test drives the JS loop (`host/web/dev.mjs` without `--wasm`), which is the design-tool canvas path.

No findings.
grok exit 0
