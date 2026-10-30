# Code review: the clone's live reads on a local server, round 1, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `73487b9eed`.
- **Method:** one brief (sha256 `5f7efb681dc1e02c972b16da13d3791ba60dda86f6f0fe74894fb7b9ab4568b5`), blind to the other reviewer; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** The brief also covered the clone's split (live-split 32377a3). Grok's narration before its verdict is left out.

CLEAN

The six authored tests prove their claims by the clock waiting on the delayed fetch, the fixture matches the reads theme2 asked to stand in for, and nothing in the new files leaks a server, a temp dir, or port 47712. The clone gate still runs only `app.test.contract`.

## What each test proves

Every answer waits 300 ms (`scripts/live-reads-fixture.mjs:205`). `clock data` and a jump both wait while a counted fetch is in flight, bound at 20 s (`host/web-js/agent.js:511`, `host/web/glue.js:1076`). 300 ms sits inside that bound, so a late answer fails the expect and a healthy one cannot trip the bound. With the wait removed (`held.js` `counted` forced false, or wasm `waitForInflight` returning at once), each expect below is still the placeholder.

Discover (`live-reads-fixture.mjs:118`). `posts == 30/31/32` is the `discover(reread)` fetch; the else is `empty()`, so the count is 0 until it lands. `fresh == true` is the poll mutation's `count > 0`. `loadLatest` bumps `reread`, which is the `every(60000)` key, so the timer is due 60000 ms after that commit. `clock settle` only seeks the scroll (`scrollIntoView` has no `behavior=smooth`), then `clock + 61000` lands at least 1000 ms past due. The jump itself waits for the fetch that timer starts (`agent.js:511`), so the following `clock data` is the same wait. The drag is local: 30 rows at height 60, `down` is `y > 200`, the same `0 -400` gesture as the clone. The failure half's `posts == 0` is also the placeholder; `pollFailed == true` (`failed(latest)`, `live-reads-fixture.mjs:176`) is true only after the 500 has thrown.

Explore (`:143`). Top's else is `{unavailable: false}`, so `search-unavailable` appears only after the 403 body is shaped (`:247`). People and Feeds render when the arrays land; the else is `[]`. The failure half's `people-failed` / `feeds-failed` are `failed()`, which is false while the fetch is pending.

Content (`:156`). The switches render on `topicsEnabled`. The else is `offConfig()` (`answered: false`), so `show_trending_topics` and the post-reload `clock data` both fail until `getConfig` lands. `videosOff` is `store.get('trending')`. The write is synchronous and the JS store persists in the commit tail before the in-flight count drops; `reload` keeps origin storage (`scripts/agent-test.mjs:608`). The failure half's `configAnswered == true` (`:192`) is the landed HTTP 500: `getConfig` resolves `{topicsEnabled: false, answered: true}` (`:255`) and does not take the else. The switches stay hidden for that same resolved value.

A repeated wasm ask is the same `n=reread` (`:213`). Same `n` returns the same feed, and each success step does `clock data` before the next bump, so `n=0` and `n=1` do not overlap.

## Fixture against the clone

People (`searchActors`), Feeds (`getPopularFeedGenerators`), and `unspecced.getConfig` are the clone's endpoints. A config failure hides the switches, as `topicsEnabled` does (`data/interstitials.ts:172`, false on rejection). The fixture also sets `answered`, which is how the failure half tells a landed 500 from the pending else. The English content-language conjunct (`interstitials.ts:175`) is outside what this test claims.

Two stand-ins are the ones theme2 specified so the clock has a fetch to wait on. Signed-out Discover's poll returns `{count: 1}` with no request (`data/latest.ts:42`); a thrown poll there is `{count: 0}`, still a success. The fixture's `getFeed.poll` (`live-reads-fixture.mjs:218`, `:245`) is the timer-started mutation whose `failed(latest)` the negative half asserts. Signed-out Top returns locally with no request (`data/explore.ts:259`); the fixture fetches `searchPosts` and maps 403 to `unavailable`, which is the shape the test is there to pin.

## Leaks, caps, gates

Both tests stop the page and the server and remove the temp dir in `finally` (`host/web-js/live-reads.test.mjs:41`, `host/web/live-reads.test.mjs:34`). The JS server binds port 0. The wasm server binds 47712 because `native` bakes that origin into the dist (`live-reads-fixture.mjs:269`); the test is `skipIf`'d until `EXACT_LIVE_READS_WASM_DIST` is set, and `stop(true)` releases the port. Same arrangement as `host/web/held.test.mjs` on 47711. The new files are 281, 46, and 39 lines; the clone files are 205 and 73. The 1500-line cap is clear, and the wasm test follows `held.test.mjs` (skipIf, fixed port, page on port 0, `finally`).

`host/web-js/conform.mjs:581` passes only `apps/<app>/app.test.contract`. Stage 12's clone conform runs that script (`scripts/stage12-gate.mjs:406`) and does not name `live.test.contract`.

## The butterfly

`app.test.contract:36` ("the butterfly scrolls to the top and reads the feed again, its new posts cleared") is the one remaining test whose assertion is coupled to a live Discover read. It has no `before data`, so the harness's opening `clock data` (`scripts/agent-test.mjs:585`) waits on Discover's `getFeed`. `newPages.0 == "discover"` (`app.test.contract:40`) needs `latestOn` (`app.contract:991`): a non-loading row and `pending(discover)` clear. The poll that fills `newPages` is the local `{count: 1}` (`data/latest.ts:42`). A settled getFeed error still leaves a non-loading row (`data/feeds.ts:164`), so that expect holds. The xrpc timeout is 15 s (`data/atproto.ts:14`), inside the 20 s clock bound, so a hung feed settles as that error. The logo's `softReset` (`app.contract:2899`) clears `newPages` and the button in the same turn it starts the reread; `quietPage` and the missing button are that local clear. An unsettled `clock data` returns `settled: false` and the harness keeps going (`scripts/agent.mjs:1098`, `scripts/agent-test.mjs:730`).

The other seven asserts are local chrome, on-device search history, notification defaults (`notifsettings.ts:113` returns immediately when signed out), or sign-in validation that refuses before a request. Search history (`app.test.contract:47`) also has no `before data`, so its opening `clock data` waits on the same Discover feed; its expects are the parsed query, the filters, and the device history. Signed-out Top never requests (`data/explore.ts:261`).
