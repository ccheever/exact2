# Code review: long-poll streams (LLP 1016.000 D8), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `5634a624cd`.
- **Method:** one brief (sha256 `06cc06e33749731b416a4d2433f18e8ea0ac5ecab51f19d1b432d1fab526cdce`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Blocking

None.

## Should-fix

1. **A lone surrogate ends the poll on one host and can continue it on the other.** `serde_json` rejects an unpaired surrogate anywhere in the body, so `cursor_of` returns no cursor and the native poll stops as `Response` (`host/apple/src/executor_longpoll.rs:279`, `:266`). The web parser accepts that document. A surrogate in the cursor is `Refused` (`host/web/http-body.js:186`). A surrogate in any other field, with a normal cursor, is delivered and the poll continues (`host/web/http-body.js:183`). Same bytes: native stops, web keeps asking. Neither puts the surrogate in the next URL.

2. **A long poll that dies before `readLongPoll` never raises the web JS held count.** The grant check returns at `host/web/http-body.js:369`, and a failed `longPollRefusal` returns at `:387`, both without `controller.replied`. Re-ask and redirect denials inside the loop do count (`:193`, `:202`). The runner counts every terminal long-poll outcome (`runner/src/runner/commit.rs:1098`). `clock held` does not hang: the request was never held, and `clock data` still settles. After the fact, `heldReplies` is 0 on web JS and 1 on the runner.

3. **A hand-written tag 10 with bad terms is a decode error natively and a successful result on JS.** `read_http` rejects `same > 1` and `long_poll_refusal` before `call_reply` applies writes (`logic/abi/src/lib.rs:521`, `:559`). `host/web-js/rust-data.js:103` turns any other byte into `undefined` and does not refuse; `callWith` then applies the writes (`:135`) and `http-body.js:387` refuses before a socket opens. A module built with `encode_result` never emits this (bad terms become a tag-4 refusal). No request is sent. Writes are still grant-checked.

## Nits

1. **`bake/src/receipt.rs:558` still writes `rustAbi: 2`, and the test at `:1045` locks it.** Not in this diff. Live publish sets 4 (`scripts/rust.mjs:421`, `bake/src/compat.rs:37`, `logic/abi/src/lib.rs:16`, `host/web-js/rust-data.js:10`), and `classifyArtifacts` is exact equality, so a leftover 2 fails closed. It does not offer tag 10 to an ABI-3 binary.

2. **IMF-fixdate still disagrees off the canonical form.** Native takes any three letters as the weekday (`host/apple/src/executor_longpoll.rs:422`) and day 1–31 with no month length (`:441`). The web requires `Sun`-style case (`host/web/http-body.js:153`) and lets `Date.UTC` overflow (31 Feb becomes March). A date before the epoch is backoff natively (`executor_longpoll.rs:451`) and the 1s floor on the web (`http-body.js:155`). `Sun, 06 Nov 1994 08:49:37 GMT` matches. Waits stay within 60s.

3. **The web scheme check is `/^https?:/i` (`host/web/http-body.js:162`); native requires an `http://` or `https://` prefix (`runner/src/request.rs:460`).** The grant check still runs on the parsed URL. This is refusal parity, not a bypass.

## Round 2, checked

The marker is a native `Completed::reply` flag (`host/apple/src/executor_stream.rs:190`). An event replaces a queued marker; a marker does not replace an event. Web drops `exact:reply` only when the request is a long poll (`host/web-js/ts-stream.js:147`, `host/web-js/rt.js:419`). The runner maps it only when `pending.long_poll` (`runner/src/runner/commit.rs:1088`). Folded replies count once, which is what `clock held` reads.

Refusal happens before send on the async executor (`host/apple/src/executor_async.rs:648`), the plain executor (`host/apple/src/executor_core.rs:1332`), `long_poll::run`, `http-body.js:385`, the seam encoder ahead of storage and ordered (`logic/abi/src/lib.rs:285`), the wasm batch, the prelude, `ts-fetch.js`, `js/src/wire.rs`, and the worker envelope. Linux and the other native hosts share this executor.

Grants are checked on every native hop (`vendor/ibex/.../fetch.rs` `admit_hop`) and on the web URL about to be asked and on `response.url`. The cursor is one encoded query parameter; it cannot change origin or path. Abort is checked at the top of both loops, between body chunks (`Body::read` checks the signal), and during `pause`. Let-go does not count. One request is in flight. The body cap is the stream ceiling (native `max_body` is `usize::MAX` only so the 16KiB read loop enforces `limit`). Heartbeats, 503s, and transport errors take the 1s floor; events re-ask immediately, as D8 says. ABI is 4 on the seam, `RUST_ABI`, `scripts/rust.mjs`, and `rust-data.js`. `typescriptRuntime` stays 3.
