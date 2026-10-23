# Code review: long-poll streams (LLP 1016.000 D8), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c89cbfa57d`.
- **Method:** one brief (sha256 `980616989fac23f8bf2f26c95a87ff62e680603106360349d70ab313c5a9a599`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Blocking

**A heartbeat never takes the stream out of in flight, so `clock settle` waits out a heartbeat-only poll.** The plan (`.longpoll-plan.md:52`) says a long poll is in flight until its first reply of any kind, heartbeat included, and a server that only heartbeats must not hold settle for longer than one wait. Snapback idle is that server.

What the code does: in flight means `messages == 0` (`runner/src/runner.rs:345-349`). `messages` increments only for `Outcome::Message` (`runner/src/runner/commit.rs:826`, `1091-1094`). The native heartbeat path calls `beat()` and waits (`host/apple/src/executor_longpoll.rs:107-115`, `131-132`); `message()` runs only for a real event (`139`). Linux `clock settle` waits while `has_pending()` (`host/linux/src/presenter.rs:1292-1294`, `host/linux/src/agent.rs:1082-1084`, `1119`, `1303-1312`) and returns `settled: false, reason: "requests"` at 20s. On the wasm page, `opened` resolves only inside `message` (`host/web/glue.js:744-750`), and `readLongPoll` heartbeats `continue` without calling it (`host/web/http-body.js:174-185`). On the JS target, `inflight.n` drops only in `done` (`host/web-js/rt.js:401-419`), and settle treats `inflight.n > holds().length` as busy for 20s (`host/web-js/agent.js:410-413`).

Clear in-flight on the first reply without delivering an event. A `message()` would be a false event. The same hole covers a first 503: that is also a reply with no message. No test settles a heartbeat-only poll.

## Should-fix

1. **Web `pause` ignores an already-aborted signal** (`host/web/http-body.js:146-150`). `addEventListener('abort')` does not run when the signal is already aborted, then `setTimeout(ms)` waits the full delay. Native `register` fires immediately (`vendor/ibex/crates/ibex2/src/stdlib/abort.rs:63-65`, `host/apple/src/executor_longpoll.rs:383-390`). Abort during `response.body.cancel()` on a 408/429/502/503/504 (`http-body.js:168-173`) then sits in `pause(wait)` for up to 60s with `controller.retrying` set (`201-203`). The loop top then sees the abort and returns: delayed cleanup, no second request, no permanent map leak. `letGo` (`host/web/glue.js:955`) cannot shorten a wait whose listener was added after the abort. The existing test aborts an already-armed wait. Check `controller.signal.aborted` at the start of `pause` and resolve.

2. **`Retry-After: 0` (or a past HTTP-date) skips both the backoff and the 1s floor.** Native `wait == 0 || pause(...)` (`executor_longpoll.rs:148-152`, `166`) loops immediately. Web floors at 0 and `pause(0)` (`http-body.js:171-172`, `202`). Failures still increment and do not affect the wait. A granted origin that answers 408/429/502/503/504 at once, with `Retry-After: 0`, is an unbounded request loop. Floor that wait at the heartbeat floor, or use the backoff minimum when the server asks for 0.

3. **Web follows redirects before the grant check, and a long poll repeats that.** Native `admit_hop` runs before every open, including each redirect hop (`vendor/ibex/crates/ibex2/src/stdlib/fetch.rs:403-405`); `DeniedRedirect` ends the stream (`host/apple/src/executor_outcome.rs:57-59`). Web checks `url.href` before `fetch` and `response.url` after the browser has followed the chain (`http-body.js:158`, `165-166`). A `redirect: 'manual'` response is opaque (`395-396`), so intermediate hops are invisible. A chain that leaves the grants and comes back is fetched on every ask for the life of the poll. The next URL stays the original one plus the cursor (`executor_longpoll.rs:142`, `http-body.js:188`), so the poll is not retargeted; the ungranted hop is. Default `credentials` is `same-origin`, and the browser strips `Authorization` on a cross-origin redirect, so this is a repeated connection (and any query the `Location` carries), not a cookie forward. Ending the stream when `response.redirected` is set makes a redirect one exchange. That is stricter than native, which will follow a granted hop and then poll the original URL again. No redirect test on either host.

4. **Web cursors past 2^53 collapse.** `cursorOf` is `JSON.parse` then `JSON.stringify` (`http-body.js:151-154`). Native `serde_json::Number::to_string()` keeps integers through 2^64−1 (`executor_longpoll.rs:234-236`). Two distinct seqs become one cursor, the same-cursor rule turns later replies into heartbeats, and the query param sticks. The 1s floor still applies. Take the original number lexeme. Spec: "a number as JSON writes it" (`llp/1016.000-answers-that-keep-coming.rfc.md:589`).

5. **`long_poll` without `stream` is sent as an ordinary GET.** The spec says the host refuses that before anything is sent (`rfc.md:579-580`). `Request::long_poll` neither sets `stream` nor checks the names (`runner/src/request.rs:419-425`). The seam keeps the terms only when `stream` is set, and an ordered request is always tag 1 (`logic/abi/src/lib.rs:362-370`). The stream opener runs only for `request.stream` (`host/apple/src/executor_core.rs:556`). `Answer::stream(...).long_poll(...)` is safe: it sets `stream` and upgrades ordered to independent 1 MiB (`runner/src/request.rs:48-55`). Refuse at encode time, the way a non-follow redirect is refused (`logic/abi/src/lib.rs:347-353`). The worker envelope checks name length (`data/src/envelope.rs:229-243`) and still keeps `long_poll` on a non-stream request.

## Nits

- **Whitespace and BOM.** Native heartbeats are ASCII-only (`executor_longpoll.rs:113`). Web `text.trim()` is Unicode (`http-body.js:181`), so NBSP is a heartbeat on web and a terminal non-JSON `Response` on native. `trim` also strips a leading U+FEFF, so BOM-prefixed JSON parses on web and fails on native.
- **`Retry-After` dates.** Native accepts IMF-fixdate only (`executor_longpoll.rs:335-350`). Web `Date.parse` (`http-body.js:171`) also accepts obsolete RFC 850 and asctime, so those wait on web and fall through to backoff on native.
- **JS seam check is weaker.** `rust-data.js:103` accepts an empty name and treats any `same` byte other than 1 as false. `read_http` rejects both (`logic/abi/src/lib.rs:500-502`). The encoder writes 0 or 1. This is the JS target's decoder; the wasm host goes through `read_http`.
- **Name length units.** Prelude and `ts-stream` use UTF-16 `.length` (`js/src/prelude.js:33`, `host/web-js/ts-stream.js:32`). Wire, seam, and envelope use Unicode scalars (`js/src/wire.rs:55`, `logic/abi/src/lib.rs:500`, `data/src/envelope.rs:234`). A name of 200 non-BMP characters is refused on the JS paths and accepted on the Rust ones.

## What holds

Native admission runs again on every re-issue and every redirect hop, before the socket. Web re-checks the URL it is about to ask and the final response URL. The cursor is percent-encoded into one query parameter (`executor_longpoll.rs:252-318`, `URL.searchParams.set`); it cannot change origin or path, and CR, LF, and NUL are encoded. An off-origin final redirect ends the stream and does not become the next URL.

One request is in flight. The reader stops at the stream ceiling (`executor_longpoll.rs:212-214`; web `boundedHttpBody`). `Accept: application/json` is set unless the app named one. Non-GET, a body, `ws:`/`wss:`, a missing `exactStream`, and `exactTimeout` are refused before send on the prelude, `ts-fetch`/`ts-stream`, and the host loop. Seam and envelope reject empty or over-long names.

204, 304, empty body, and same-cursor are heartbeats on both hosts; 408/429/502/503/504 retry; anything else ends. A heartbeat is not shown as `retrying`. Any 2xx resets the failure count. The native `retrying` map is removed after the wait, on abort, and after a panic (`executor_longpoll.rs:167-176`). Web `request()` deletes `controller.retrying` in `finally` (`http-body.js:354`). `forget` / `letGo` abort the controller the runner no longer holds.

## (a) Leave `RUST_ABI` at 3

An older host's `read_result` returns `"invalid result tag"` for tag 10 (`logic/abi/src/lib.rs:454`). `call_reply` finishes decoding, including the result, before it applies writes (`535-576`), so the call fails as `DataError::Unavailable` with no HTTP and no writes. Tag 10 is not decoded as tag 9 or as ordered HTTP. That is the same rule as tag 6 (`358-359`): bump the compat id for silent misbehavior, and leave the ABI when the old path already fails loudly.

`TYPESCRIPT_RUNTIME` 2 → 3 is the silent case and it did move (`bake/src/compat.rs:35-43`). An older prelude drops `exactLongPoll` and opens an event stream (`docs/reference.md:1144-1146`). `typescriptRuntime` is a compat input only for a Hermes app (`compat.rs:473-475`).

A Rust-only app's compat id does not move, because `rustAbi` stays 3 (`compat.rs:34`, `390`). An OTA bundle that starts emitting tag 10 still loads on an old shell, and that source fails loudly until the binary is updated. Sources that never emit tag 10 keep working. Bumping `RUST_ABI` would force every Rust app onto a new binary. An old `rust-data.js` throws `a Rust answer of kind 10 (surface work)` (`host/web-js/rust-data.js:109`); loud, and the message is wrong, and that file ships in the same bundle as the module.

## (b) The 1s floor is the right number

Measure it from ask time, which this does (`executor_longpoll.rs:92-95`, `http-body.js:159-160`). A poll the server holds for 25s is not delayed. Events re-ask at once (`rfc.md:597`); raising the floor would add latency to real news. One hertz is the right cap for a no-news reply, including up to the existing stream bound. It does not cover `Retry-After: 0` (should-fix 2) or a cursor that changes on every reply. The test overrides (40ms under `cfg(test)`, `longPollTiming.heartbeatMs`) are fine while production stays at 1000.
