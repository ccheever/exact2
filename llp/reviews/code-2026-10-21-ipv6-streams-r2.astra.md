# Code review: the dev loop's [::1] connections lose their requests' close under Bun, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `519c4d06862dd6cdb9d13ee14ae083fb2236e68c045143ed05faa9ce3b16d095`), shared by both reviewers, each blind to the other's review; HEAD `d6fc3d20cc`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 2 SHOULD-FIX
- **Disposition:**
  Neither reviewer found anything blocking. Grok had no findings and confirmed each round 1 fix, the shutdown order under Bun 1.4.2, and the wasm loop. Astra's two test SHOULD-FIXes are taken post-round, in their own commit:
  - Astra 1 (the IPv6 probe asked the listener under test, so a missing [::1] listener would skip the localhost leg and pass): IPv6 is checked on a socket of the test's own; where it exists, the loop must answer on [::1] or the test fails.
  - Astra 2 (request errors swallowed when Bun marks the request destroyed first): errors reject unconditionally; after the client's own destroy the promise has already settled.
  Rerun: the unit test 3 of 3, the dev-loop test passing.

Nothing blocks. Two test issues remain:

1. **SHOULD-FIX — [host/web-js/dev-page-streams.test.mjs:42](/tmp/rv-dt-six2-astra/host/web-js/dev-page-streams.test.mjs:42): IPv6 coverage can silently disappear.** The availability probe connects to the listener being tested. A missing IPv6 listener produces `ECONNREFUSED`, skips localhost, and passes on an IPv6-capable machine. Check IPv6 support independently, then require the dev listener to answer.

2. **SHOULD-FIX — [host/web/tests/ipv6-loopback.test.mjs:50](/tmp/rv-dt-six2-astra/host/web/tests/ipv6-loopback.test.mjs:50): Request errors can still be swallowed.** Bun sets `destroyed` before emitting some request errors, so this guard can leave `stream()` pending until timeout. Reject errors unconditionally; rejection after the intentional destroy’s resolved promise is harmless. [Bun implementation](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/js/node/_http_client.ts#L992-L1005)

The upgrade destroy, listener readiness, assertion-failure teardown, and explicit `[::1]` close wait otherwise address round 1. Handled upgrades remain safe: `emit()` returns true when a listener exists in [Bun](https://github.com/oven-sh/bun/blob/bun-v1.4.2/src/js/node/events.ts#L164-L217) and [Node](https://nodejs.org/api/events.html#emitteremiteventname-args). No wasm-loop regression found.

Read-only review; no builds or tests run.
astra exit 0
