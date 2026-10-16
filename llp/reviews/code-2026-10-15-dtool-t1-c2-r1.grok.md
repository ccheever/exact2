# Code review: LLP 1122 T1 commit 2 (the design tool's skeleton and its connection), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `0beef42fd`.
- **Method:** one brief (sha256 `93951f84cabd7661cf1682a6a9aa29ea578b2089aad0a506205fa5c61fed0b2d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** do not land (1 high, 4 medium, 3 low).
- **Disposition:** taken: 503 while down; the first poll asks `{after: 0, wait: false}` (a new wire field, answered at once) so no backlog is dropped; `missed` clears the list; `--via` loopback-only, no query passed on, the body capped at 1 MiB, upstream aborted; a restart test on the stub; the glue test writes after the third reply; Try Again bumps the poll only when none is held; `redirect: 'error'` on the module's fetches; the stub's own endpoints take only the driver's bearer. Kept: the stub's CORS token endpoint, a canned token that exists only while `test` or `verify` runs and reaches no design. Not taken: holding the stub's poll, since `clock data` would wait on it; the real server's hold is covered by scripts/design-tool.test.mjs.

---

The follower tight-loops after the design server dies, and the first poll always takes the miss path and drops events the server still has. The client's own token handling is sound. The `--via` proxy is the weak point.

## High

**A dead design server turns the long poll into a tight loop.** `scripts/tamber-server.mjs:327`, `apps/design-tool/app.contract:48`, `apps/design-tool/app.ts:86`.

`POST /__tamber/events` answers immediately whenever `down` is set, with HTTP 200, even after the client is already caught up (`after == seq`, epoch matches, no newer event). The view treats any `changes.ok` as success and runs `advance` again `after(1)` (1 ms). `waitChanges` bumps `answer` on every call, so the task key changes every time.

The design server exits (lock lost, `cargo` crash) after the tool is open. `fail` pushes one `down` event and wakes the in-flight poll. The client lists it once (`" · the server"`), sets `since` to that seq, and polls again. Every later poll is an instant 200 with an empty batch. Localhost then does thousands of `POST /__tamber/events` per second, and the screen stays on "Following changes" instead of the no-server state. A real HTTP failure backs off 3 s. This path never fails.

In `answer()`, if `down` is set and no event has `seq > after`, return 503 with `down`'s message. Leave the immediate 200 only for the poll that still has that `down` event queued. The existing retry task then waits 3 s and shows `changes.error`.

## Medium

**The first poll always misses, so events still in the buffer are dropped.** `apps/design-tool/app.ts:86`, `scripts/tamber-server.mjs:323`, `apps/design-tool/app.contract:55`.

With `epoch == ""` the client sends `{ after: 0, epoch: '' }`. `after: 0` asks for the backlog. The server treats any present epoch that is not this run's as `missed` and returns `events: []` plus the current seq. `advance` then sets `since` to that seq.

Open the tool after an agent write, or have the write land before this first answer is applied. The server still holds the event (it keeps 256). The client throws it away, refreshes `hello` (digests only), and follows from the tip. Anything between "page opened" and that first answer is gone. Omitting `epoch` would have replayed from 0. The empty string does the opposite.

Send `{ after: 0 }` with no `epoch` field until a real epoch has been seen. Send `{ after: since, epoch }` only when `epoch` is non-empty.

**`missed` does not clear the change list, and sequence numbers restart.** `apps/design-tool/app.ts:90`, `apps/design-tool/app.contract:102`.

On `missed`, `recent` is left as-is (`fresh` is empty, then prepended onto the old list). The view keys rows by `` `${c.seq}` ``. A new server process starts `seq` at 0 with a new epoch.

The tool is open, the design server restarts, the next poll is `missed`. `hello` refreshes, but the list still shows the previous run. New edits reuse seq 1, 2, …. Duplicate keys, so later rows do not update, and a hand edit from the previous process stays attributed on screen.

When `r.missed` is true, set `recent` to `[]` before returning, and do that before applying `seq`.

**`--via` forwards the token to whatever origin it is given.** `scripts/tamber-server.mjs:703`, `scripts/tamber-server.mjs:740`.

`target` is `new URL(via).origin` with no loopback check. `forward` copies `x-exact-tamber-token` onto that request and appends `url.search`. It also buffers the body with no size cap, and it does not abort the upstream fetch when the browser disconnects. `redirect: 'error'` only stops a second hop.

`bun exact.mjs tamber tool . --via http://127.0.0.1:7740` (or the tool's own URL) binds the proxy to itself. `GET /__tamber/token` is forwarded forever until sockets run out. `bun exact.mjs tamber tool . --via http://100.64.0.5:8080` or a tunnel URL sends the header and the raw query to that host. A correct remote Tamber server will refuse to issue a token to a non-loopback peer, so the design tool itself stops. Any other local client that already has the header, or a token placed in the query, is forwarded off-machine. A multi-megabyte POST is buffered here before the real 1 MiB limit applies. A dropped long poll or `subscribe` leaves the upstream connection open.

Allow only `http://127.0.0.1:<port>` and `http://localhost:<port>`, and reject the tool's own origin. Do not append `url.search`. Cap the body at the same 1 MiB limit. Abort the upstream fetch on `req` `close`.

**The tests do not prove the long poll, `missed`, or the dead-server path.** `apps/design-tool/stub/tamber-stub.ts:44`, `apps/design-tool/app.test.contract:31`, `scripts/design-tool.test.mjs:53`.

The stub answers `/__tamber/events` in the same turn, and its `missed` flag is only `epoch !== EPOCH`. The contract test's "long poll" is two `clock +1` / `clock data` cycles against instant JSON. It never holds a poll, never gaps `seq`, and never backs off. The first stub reply is always `missed` because of the empty epoch above, and nothing asserts that `hello` was re-read or that the list was replaced. `scripts/design-tool.test.mjs` does drive one real attributed write, but only after any stdout line containing `"settled"`. `landed` sets `settled: false` on a `type` or `tap` while data is still in flight (`scripts/agent.mjs:1352`). An early match writes before the follower's cursor is fixed. That write is then skipped by the forced miss, and the next `clock data` waits on a 25 s poll with a 20 s bound.

Hold the stub's poll until `/__stub/event`, and add a restart (`missed`, new epoch, seq back to 1) that expects the old row gone and `hello` refreshed. In the glue test, wait for the `clock data` reply that already shows `hello.ok`, then write, and assert the row from the following `state`. Neither test fails if a dead server spins.

## Low

**Try Again starts a second poll while the first is still in flight.** `apps/design-tool/app.contract:59`, `apps/design-tool/app.ts:63`.

`hello` can fail (compiler error, HTTP 500) while `/__tamber/events` is held up to 25 s. The no-server screen is up, and `reconnect` does `asks = asks + 1` without `not pending(changes)`. Both calls share `recent`, `answers`, and `place`. The older call finishing last rewinds `place.seq`, and the next poll replays events into duplicate keys.

Ignore `reconnect` while `pending(changes)`, or ignore a `waitChanges` result whose `answer` is older than the latest one applied.

**The data-module fetch follows redirects.** `apps/design-tool/app.ts:43`.

`tamber op` and `forward` use `redirect: 'error'`. `call` and `tokenOf` do not. Grants reject an off-origin final URL, and a cross-origin redirect drops `x-exact-tamber-token`, so this does not currently leak. A same-origin 302 would resend the header to whatever path on port 7740 answered. Pass `redirect: 'error'` on both fetches.

**The stub is an open CORS token endpoint on the tool's port.** `apps/design-tool/stub/tamber-stub.ts:19`.

`access-control-allow-origin: *` and `/__stub/event` have no token check. While `test` or `verify` is running, any page the browser loads can `fetch('http://127.0.0.1:7740/__tamber/token')`, read `stub-token`, and push events. Drop CORS on the stub, or allow only the test driver's origin, and require the stub token on `/__stub/event`.

The fixed port matches the grant rule (`host/web/navigation.js` `admitsNetwork` pins scheme, host, and port). `serveTool`'s dot-segment and realpath checks match the tests. `listen()` preserves `tamber serve`. Those parts are fine.

Verdict: Do not land until a dead server stops the 1 ms re-poll and the first poll stops sending `epoch: ''`.
