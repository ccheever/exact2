# LLP 1110: Live Snapback reads in an Exact app

**Type:** RFC
**Status:** Draft r2, 2026-10-08. r1's direction approved by Charlie the same day (SSE, the Snapback endpoint by its lead session); r2 keeps that endpoint as the later transport and adds an Exact-side mechanism that needs no server change. Reviewed twice: round 1 folded; round 2 GPT-6 Astra NOT READY, Grok 4.7 READY WITH CHANGES. Not accepted, and not built on main (the lane stops at two rounds). Round 2's design points are folded into the text below and marked *owed* where the lane's implementation does not yet meet them (Dispositions).
**Systems:** Runner (an answer that keeps coming whose messages a TypeScript iterator produces: `Answer::message`, the stream's count carried across its rounds); the TypeScript executors (`js/src/prelude.js` natively and in the wasm target's module realm; `exact-js`; `js/web`; the JS target's `host/web-js/ts-data.js` and `rt.js`); the web glue's `clock settle`; the Snapback4 client (`snapback4/ts`: `Snapback.changes()`); Snapback 4's HTTP host (`/changes` as an event stream, later; Snapback repository, its lead session); Contract (no new form)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-08
**Revised:** 2026-10-08, r2: a TypeScript source may answer with an async iterator (D1–D7); `Snapback.changes()` over the existing long poll is its first consumer (D8); r1's server-sent events become the later transport behind the same call (D9). r1's D2–D4 are folded into D8. 2026-10-08, r2 round 1 folded: native lowering of async generators, `Request::iterator` from the first round, the refused-commit and forwarding rules, settle by what a stream waits on, `cursor`, heartbeats and `ephTables=` (Dispositions). 2026-10-08, r2 round 2 folded as design (marked *owed* where lane C's implementation falls short): rounds keyed by logical call, every failure retires the call, topics wait for a round in flight, `pending(r)` apart from settle, lowering by parsing the bundle, iterators deferred at bake, reservations at open with one disposal, worker refusal before the pump.
**Implementer:** unassigned for round 3. Lane C (Claude (Opus 5.5), 2026-10-08) built round 1's design on branch `lane/iterator-streams` (not landed): the runner half, every executor, `Snapback.changes()`, tests and drives; round 2 found the defects listed under Dispositions in it.
**Related:** LLP 1016.000 (answers that keep coming: D1 a stream is a resource, D2 interest ends it, D4 coalesce, D5 an open stream does not hold `clock settle`, D6 the long poll as the interim; "Not built: … worker-placed TypeScript streams"), LLP 1069.004 (SSE and receive-only WebSocket as built; Charlie: no trade is needed for live data), LLP 1027 D1a (an answer may be a promise; each awaited `fetch` is one more round under its own ticket), LLP 1027.000 (no clock or timers in a data module), LLP 1027.002 (worker placement), LLP 1041 D2 and §8.4 (bounds; independent HTTP), LLP 1097 (storage that finishes after the answer), LLP 1012 §2 (`clock settle`), `snapback4/README.md` ("Live updates"), Snapback 4 `crates/snapback4/src/serve_changes.rs` (`GET /changes?since&wait`), the app farm's round-02 synthesis (`~/appfarm/synthesis/round-02.md`)

## Summary

An Exact app on Snapback 4 sees other people's writes only when it asks
again, and today it asks on a timer. Snapback's change long poll (`GET
/changes?wait`) is what makes Snapback's own clients live, and it cannot
be held inside an Exact answer: an answer is bounded, and an answer in
flight holds `clock settle`.

r1 proposed that Snapback serve its changes as server-sent events, which
Exact's `fetch(url, { exactStream })` already reads. That needs a change to
Snapback's concurrency model first (D9), so it is slow. r2 adds the
Exact-side half that needs nothing from the server: **a TypeScript source
may answer with an async iterator.** Each `yield` is a stream message, a
`return` ends the stream, a throw is a failure, and the runner's forget
path calls `return()` when interest goes. The Snapback client then offers
`Snapback.changes()`, an async generator over the long poll that yields
`{ seq, online }` each time the server's head moves. The reading resources
take `seq` as an argument, so a change re-asks them. That gives live reads
today on Snapback's existing long poll, and the event stream stays the
later transport behind the same call.

## Motivation

The app farm built small apps from scratch on Exact2 + Snapback 4 with two
builder models. In its second arm (Exact2's first-party Snapback client
required), 67 of the arm's 76 apps refresh on a Contract `task` every 2 to
15 seconds, and none push. Every diary that made updates appear without a
tap did it with a `task` calling an action that `sync()`s and reads again;
none held `poll(20)`. The builders said why, in their words:

- "A long-held poll would make clock-data test settlement wait, so periodic
  sync fits this slice." (quiet-tent-capacity-0966)
- "The app does not use a hanging long poll, since each Exact data answer
  needs a bounded lifetime." (pto-coverage-planner-0432)
- "`poll()` is unused: Exact sources cannot hold timers, and a 20 s long-poll
  inside a resource would stall answers the test driver waits on; freshness
  comes from the 15 s contract task re-syncing instead." (studio-door-0755)

In the first arm (hand-written HTTP), 61 of 109 apps polled on a timer.

A timer costs every client a request per period whether or not anything
changed. It shows a change up to a period late, and it leaves Snapback's
own change signal unused. LLP 1016.000 D6 offered the long poll with a
cursor as the interim, and in practice it collides with D5's reason to
exist: a held answer is "in flight", so a test that settles waits for it.

What is missing is narrow. Exact already has answers that keep coming
(LLP 1016.000), and they already stop holding `clock settle` after their
first message. A TypeScript source, though, can only make one by handing
the host a single streamed request (`exactStream`): it cannot say "wait for
news by long poll, then tell me". An async generator says exactly that, in
the most common shape the language has for it.

## Design

### D1 — An async iterator is an answer that keeps coming

A TypeScript source's `answer` may return an **async iterator**: an object
with `next()` and `[Symbol.asyncIterator]`. That is what calling an
`async function*` returns, and what a hand-written iterator is.

```ts
const sources: Sources = {
  live: ([persona]) => Snapback.changes({ origin, headers: () => ({ 'x-snapback-persona': persona }) }),
  ticker: async function* ([from]) {
    for (let n = from; ; n++) {
      const r = await fetch(`${origin}/next?after=${n}`, { exactIndependentHttp: { maxResponseBytes: 4096 } });
      yield { n: (await r.json()).n };
    }
  },
};
```

It is LLP 1016.000's stream resource, and nothing about the resource
changes. `pending(r)` is true until the first message and false from then
on. Each message is parsed and shape-checked like a single answer and
commits as its own settlement: derives recompute, and resources whose
arguments changed ask again (LLP 1016 D5). The resource's value is the
last message's.

- **A turn** is what runs between two host operations: from `next()`, or
  from the reply that resumes the iterator, until it awaits host work
  (a `fetch`, a storage step, `native.later`), returns, or throws. The
  executor pumps the iterator eagerly. After a yield it calls `next()`
  again at once, so a turn runs on to the next host operation.
- **One message per turn.** The newest value yielded in a turn is the
  turn's message. Earlier ones in the same turn are coalesced (D4). A turn
  that yields nothing commits nothing: it is one more round, as an awaited
  `fetch` is in a promised answer (LLP 1027 D1a as built).
- **`return` ends the stream.** The resource keeps its last value, and the
  return value is ignored (an iterator answer is typed
  `AsyncIterator<T, void>`). If it ends before its first message, that is a
  failure: `an answer that keeps coming ended before its first message`.
- **A throw is a failure.** LLP 1016 D4 applies: the resource keeps its
  last value, `failure(r)` reads it, and the stream is over. A value
  yielded in the same turn as the throw is not delivered.
- **The iterator is the answer, returned as is.** `answer` must return the
  iterator itself, not a promise of one. A promise that resolves to an
  iterator fails, saying so: `return the iterator itself, not a promise of
  it: make the source an async function*`. The JS target needs this
  (D6), and one rule on every executor is simpler than two.
- **Resources only.** A mutation answers once. An iterator answer to a
  `send` fails when it is asked: `a mutation answers once; an answer that
  keeps coming is a resource`. It is let go (`return()`) at once.
- **Not a sync iterator.** Only `Symbol.asyncIterator` marks a stream. A
  list is a value, and arrays are sync iterables.
- **No `exactStream` inside one.** An iterator answer that calls
  `fetch(…, { exactStream })` fails: the iterator is already the stream.
  An answer is one or the other.

**Hermes has no async generators.** The pinned `hermesc` refuses `async
function*` ("async generators are unsupported"). So a native bake whose
bundle contains one lowers the whole bundle to ES2017 with rolldown's
`transform.target`, which turns each async generator into the lowering's
generator-and-promise helper. Whether it contains one is read from the
bundled output, dependencies included, by parsing it, never by matching
source text (*owed*: the lane's `lowering` in `js/bake/src/typescript.mjs`
matches captured sources with a pattern, which misses `async/* c
*/function*` and a dependency's generator, and lowers on a comment). A
bundle with none is left as written. The web runs async generators as written. A
hand-written iterator (an object with `next`, `return` and
`[Symbol.asyncIterator]`) needs no lowering anywhere, which is why
`Snapback.changes()` is written that way (D8): every Snapback app would
otherwise be lowered. The executor tests run lowered generators through
the pinned VM: `await`, yields, a throw, `return()` and `finally`.

**At bake, an iterator is deferred whatever it does,** a finite one that
never awaits host work included: `__exact_call` refuses it with the bake
code, as a storage-reading answer is refused, so it has no compiled value
and the device asks after first pixel (*owed*: the lane's prelude lets a
finite iterator's last message become a compiled value).

**Types.** The generated declarations add `Messages<S> =
AsyncIterator<Result<S>, void>` to what a source may return (`Sources`,
`Answer`), beside `Result<S> | Promise<Result<S>>`.

*Rejected:* a Contract form (`stream resource …`). The source decides
whether it streams, as LLP 1016.000 D1 already ruled for `exactStream`.

### D2 — Each awaited host operation is its own request

Inside an iterator, `fetch` is the same binding as in any answer. On the
executors where the runner sees a TypeScript source's requests (native
Hermes, and the wasm target's module realm), every `fetch` the iterator
awaits is a **request under its own ticket**. The host admits each one by
its grants when it runs it (`net.fetch`, LLP 1016 D6), aborts it when the
runner lets it go, and counts it on its lane, exactly as it does each
`fetch` of a promised answer (LLP 1027 D1a: "one more round … under a new
ticket"). Storage steps and `native.later` are requests too, as they are
for any answer. No host work runs outside a ticket, and no grant is checked
once for the whole stream.

On the JS target, a TypeScript source's `fetch` is the page's grant-checked
fetch (`ts-fetch.js` `fetchWith`) and never a runner ticket. That is true
of every answer there, so an iterator's fetches are admitted the same way
and nothing new is introduced.

**The runner half.** Two fields on `Request`, no new `Answer` variant, on
the precedent of `Answer::stream` (LLP 1069.004 slice 1):

- **`Request::iterator`** marks every request an iterator answer awaits,
  **from its first round**. The executor knows the answer is an iterator
  when `answer` returns it, so the runner knows before any message. Such a
  request is never kept for newer arguments (LLP 1054.000.000 D3), a send
  refuses it, and its pending entry is a stream from the start: listed in
  `state.streams` with zero messages and in flight, as an SSE stream is
  before its first event.
- **`Request::yielded`** carries a message: `Answer::message(value,
  coalesced, then)` returns `Answer::Later(then)` with the value (in its
  canonical bytes, since a request crosses threads) and the mark set. The
  runner takes the value off before the host sees the request.

A round's reply is ordinary (`Outcome::Response`, a storage result), so it
takes `fulfill`'s ordinary path, and the pending entry's `StreamCount`
(messages, coalesced) carries from round to round. What the source answers
decides what happens:

- **a message and a next round:** the value commits as the resource's,
  exactly as an answer now does, and the next round goes out **with the
  same commit, before it settles**. A settlement that asks the resource
  again in that commit (a watched topic, LLP 1016.002 D4, or a forced
  refresh) supersedes the round as it supersedes any request in flight,
  whether the newer answer comes now or later.
- **a round with no message:** one more round of the same stream.
- **`Answer::Now(value)`:** the final turn, which commits and ends it. The
  value is the newest yield, or the last one delivered when the final turn
  yielded nothing.
- **an error:** the stream ends as a failure (`release_failed`).

A message from an `answer` (a first turn that yields before any host work)
is the same, taken at settlement time: the value shows in the asking
commit and its round goes out with it.

**When a commit is refused.** The iterator has consumed the reply, and no
other reply will come for that ticket. So a round's reply whose commit is
refused, for any reason and not only a data or shape error, releases the
ticket as failed (as a host stream's message already does), and the
round the refused commit enqueued is taken back out of the handed-out
requests. The source learns that its call is no longer wanted through
`forgotten` (the runner sets `forgot`), which is how an HTTP round's call
is cleaned up; `discard` stays for continuation tokens.

**A refused commit must not lose the iterator it superseded.** An
equal-argument refresh asks a new iterator while the old round is in
flight. The executor lets the old call go only once the commit that
replaced it stands; if that commit is refused, the runner restores the old
round and its reply must resume the old iterator, never the new one. So an
executor keys an iterator's rounds by its logical call (a token on every
round, as a continuation carries one), not by source and arguments, and
defers supersession cleanup to `forgotten` after the commit (*owed*: the
lane's `exact-js` forgets the replaced call at once and keys HTTP rounds by
arguments).

**Every failure retires the call.** A round whose message fails to decode
or fit its shape, or whose turn runs over its budget, ends the stream as a
failure, and the executor lets the call go there and then: `return()`, its
reservation released, nothing left parked (*owed*: the lane's `exact-js`
and `js/web` drop the parked call before decoding and never forget it).

**A watched topic waits for a round in flight** (LLP 1016.002 D4). While an
iterator round is in flight (before the first message, or a storage step
or continuation after it), a changed topic marks the round and asks again
when it lands; it asks now only when the round is open (a network wait,
which has no device reply to wait for). A round with no message carries
the mark on, and a marked round's landing asks again in that commit, so a
quiet stream cannot hold the mark (*owed*: the lane's `changed` asks every
stream now, on the runner and the JS target).

**Forwarding sources.** `Mixed` (outside an ordered set) and placement on
`main` pass the request through as they pass any. `Storage<D>` rewrites a
portable storage request into a continuation, and it carries `yielded` and
`iterator` across the rewrite. A turn envelope cannot carry a value and a
request together, and a stream would outlive the turn's reservation, so
every composition that answers through one refuses (D6).

### D3 — `clock settle`: in flight until the first message, then on the device only

An iterator answer is in flight until its first message lands, and
`pending(r)` is false from then on, whatever the stream awaits. What holds
the agent's `clock settle` is a separate question, answered by what the
stream waits for (*owed*: the lane's runner uses one predicate for both,
so a storage step after a message makes `pending(r)` true natively but not
on the JS target). After the first message, **its HTTP requests and long
native calls do not hold `clock settle`**: they wait on the far side, as an SSE stream's socket does, or
`settle` would wait out every long poll. **Its storage steps and
continuations still do**: they wait only on the device, and every web
target already counts storage in flight. That is LLP 1016.000 D5 applied
to what the stream waits for, not to its transport.

What this means for a drive:

- **The first message is deterministic.** Everything before it, the
  iterator's first `fetch` included, holds `settle`. A resource reading
  `live.cursor` has its first answer when `clock data` returns.
- **A message the far side triggers lands when its reply is delivered.**
  Every native agent operation delivers one completion the host already
  holds as it begins, and `clock settle` goes on pumping only while
  something is in flight; the web delivers a reply as it arrives. The iterator's local work after the
  reply (the JSON parse, storage, the yield) commits with it. `settle` does
  not wait for a reply that has not arrived, and a second open-stream reply
  already queued natively waits for the next operation. Nothing makes the
  far side's reply arrive before a given operation, so a drive that
  expects another session's change asks `clock settle` again, a bounded
  number of times, until it shows. (The drives in the lane needed one.)
- **What the message starts holds `settle` again.** The resources it
  re-asks, which `sync()` and read, are ordinary answers.

Executor tests have no network: they script the replies and are exact.

*Rejected:* counting every request of an iterator unless the app marks a
wait (`fetch(url, { exactWait })`). Every message would be deterministic,
but every author would have to mark every wait, and an unmarked one would
hold `settle` for its whole wait. If drives show that the bounded re-ask
is not enough, it can come back as a refinement.

### D4 — Coalescing and bounds

- **Coalescing is per turn, and pulling makes it the only coalescing.** An
  iterator is pull-based: across turns it cannot run ahead of the runner,
  because it resumes only when its reply is delivered. Within one turn,
  yields that the turn overtakes are coalesced to the newest, and their
  count is the message's `coalesced`. That is LLP 1016.000 D4's "keep only
  the newest undelivered message", with no host queue at all. Backpressure
  is native to iterators, which is what D4 rejected for SSE because SSE
  has none. On the JS target a turn is what runs before the page's next
  task (D6).
- **A turn yields at most 1,000 times.** The eager pump would otherwise
  loop forever on a synchronous `while (true) yield x`. At the 1,000th
  yield without host work the stream fails: `yielded 1000 messages in one
  turn without awaiting host work`. A turn also keeps the existing 100 ms
  per-call budget natively (LLP 1027 D4). Each turn is a call.
- **A message is an answer.** Each value takes every limit an answer takes:
  the shape check, the executor's reply decoding and its byte limits, and
  the plan's value vocabulary. Nothing new is admitted.
- **At most 16 iterator streams open per module,** reserved when the
  stream opens (natively when `answer` returns the iterator; on the JS
  target when its opener runs) and released exactly once, by one idempotent
  disposal, when it ends, fails, is let go, or its answer is discarded by a
  re-read or a refused commit; a disposed stream pulls and delivers no more
  (*owed* on the JS target: the lane reserves at `answer` and leaks a
  reservation on a discarded answer, and keeps pulling after a failure). That is the
  same number as the native executor's open SSE and socket streams (LLP
  1069.004 As built). The 17th fails when it is asked: `16 answers that
  keep coming are open in this module`. An open iterator stream costs an
  engine object and whatever request it has in flight. Idle, it holds no
  thread.
- **What its requests cost is their own.** A long poll is a request held
  by the server. Natively it must be independent HTTP
  (`exactIndependentHttp`, LLP 1041 §8.4): on the ordered lane it would
  hold every later ordered request of the app behind it for up to the wait.
  So it occupies one of the native executor's two independent workers for
  its wait. Two live streams fill them, and further independent work waits
  behind them (it is not refused; the lane admits 128). That cost is
  stated here, not hidden. A host stream path for an iterator's waits (its
  own thread, the 16-stream bound) is the remedy if it is measured to
  matter. It is not built.

### D5 — Interest ends it: the forget path calls `return()`

When the resource's arguments change, `refresh` runs (with equal
arguments too: a new iterator), it answers now, or it leaves the plan, the
runner forgets the ticket (LLP 1016.000 D2), and:

- **natively and in the module realm**, the host aborts the request in
  flight. The executor lets the call go: its pending `fetch` rejects with
  `FetchError` `Aborted`, `return()` is called on the iterator, later
  `fetch`es in it reject at once, later yields are dropped, and its
  stream's reservation is released. A generator suspended at an `await`
  cannot be returned until it reaches a `yield`. So the language runs the
  queued `return()` at its next `yield`, or the rejected `fetch` ends it if
  the iterator does not catch it. Either way its `finally` runs, and
  nothing it does afterward reaches the runner.
- **on the JS target**, the ticket closes once the commit that let it go
  stands (`rt.js` `Open`, as for `exactStream`), and the pump calls
  `return()`. A `fetch` made synchronously as the iterator resumes (inside
  the pump's `next()`, before the turn's first `await`) carries the
  stream's `AbortSignal` and is aborted. A generator's `fetch` made after
  an `await` in the same turn cannot be tied to the stream, since the page
  has no turn to tie it to (the reason `exactStream` must start before the
  first `await`). It runs to its end and its result is dropped.

An iterator that must abort its own waits, on every executor, owns an
`AbortController` and aborts it in its own `return()`. A hand-written
iterator's `return()` runs at once, even while its `next()` is pending.
`Snapback.changes()` does this, so its poll is aborted the moment interest
goes (D8).

What the server sees is its own matter. Snapback's held poll notices a
closed connection only when it writes, so `Snapback.changes()` asks with
`heartbeat=1`. The server then beats every 2 s (only on HTTP/1.1, which its
`tiny_http` host speaks), and a poll that was let go frees its long-wait
worker within one beat. Without it the worker would
wait for a change or the deadline.

A late message on a forgotten ticket is dropped with the usual journal
line.

### D6 — Every executor

| executor | where it runs | how |
|---|---|---|
| Native Hermes, `main` placement | Apple (macOS, iOS, tvOS), Linux, Windows (`exact-js`) | The bake lowers async generators (D1). The prelude's `__exact_call` sees an async iterator (its tag-3 reply says `iterator`), reserves one of the 16, keeps it on the call, and pumps it. `__exact_settle` answers a new tag 4 (the turn's newest yield, its `coalesced`, and the ticket it now awaits) as well as today's tags 0/1/2, and marks every round `iterator`. `exact-js` maps tag 4 to `Answer::message` and refuses an iterator for a mutation at once. `__exact_forget` calls `return()` (D5). |
| Native Hermes, `worker` placement, and any module in an ordered set | the same hosts: `typescript.placement: "worker"`, or a main module that shares a `secret.keep` name with a worker module (LLP 1027.002 D3) | **Refused, visibly, when `answer` returns the iterator, before it is pumped.** Their turns cross an envelope, which carries a value or a request but not both, and a stream would outlive a turn's reservation. That is LLP 1016.000's "worker-placed TypeScript streams, not built". The composer tells the module at construction that it is placed or in an ordered set, and `__exact_call` fails the answer: `an answer that keeps coming is not built for worker placement or an ordered set's turns (LLP 1016.000); place this module on main`. (*Owed*: the lane refuses only when a round leaves the turn envelope, so an iterator that finishes inside one owner turn commits as a one-shot.) |
| Web, JS target (the default web build) | the page realm (`host/web-js`) | `ts-data.js` hands `rt.js` the iterator as a stream (`{ stream: opener }`, the shape `exactStream` already uses). The opener pumps it: a turn is the burst of yields before the page's next task, and its newest yield is delivered as a message. Its end re-delivers the last message's value, which commits nothing new, and closes the ticket. A throw is the failure. `rt.js` counts it in flight until its first message, lists it in `state.streams`, refuses it for a send, and closes it when its ticket is let go: the opener calls `return()` (D5). |
| Web, wasm target (games, `--wasm`, conformance's oracle) | the iframe module realm (`module-glue.js`, the same `prelude.js`) | The prelude as natively. `module-glue.js` treats tag 4 as it treats tag 1, and a message from a turn that goes on to storage rides to the turn's end: the turn's fetch carries it, or a newer message overtakes it. `js/web` maps tag 4 to `Answer::message`. The glue's `clock settle` counts a ticket only while the runner says it is in flight: `exact_request_active` answers 2 for an open stream's network round, which `letGo` keeps and `settle` does not wait on. |
| Web, wasm target, `worker` placement | the dedicated module Worker | Refused, as native `worker` (same words), by the prelude when the realm says `placement` is `worker`. (*Owed*: the lane's Worker realm has no refusal; a tag 4 there fails as "fetch has no URL".) |
| Bake (`query`) | build time | Deferred, as a fetching answer is (LLP 1027 D4): no compiled value, and the device asks after first pixel. |

No host's transport changes. An iterator's requests are ordinary requests
on every host.

### D7 — What the agent sees

`state.streams` lists an iterator stream from its first round, with
`messages` and `coalesced`, as it lists an SSE stream, and `state.pending`
lists it until its first message. Then, by executor:

- **Natively and in the wasm target's module realm** (the runner's
  tickets), its `ticket` is the round in flight, so it changes each round,
  and `state.pending` lists it again while it awaits a storage step or a
  continuation (D3). The journal says `fulfil {ticket} ({name}) […]` for
  each reply, `{name}: the reply asks for one more request` for a round
  with no message, and `forget request {ticket} ({name})` when interest
  goes.
- **On the JS target** its `ticket` is the opener's, one for the
  iterator's life. A storage step after the first message holds `clock
  settle` (the page's storage queue counts in flight) without listing the
  stream in `state.pending`. The journal says `message {ticket}` per
  message and `close stream {ticket}` when its ticket is let go.

No new operation.

### D8 — The first consumer: `Snapback.changes()`

```ts
// app.ts
live: ([persona]) => Snapback.changes({ origin, headers: () => ({ 'x-snapback-persona': persona }) }),
```

```
shape Live
  cursor: string
  seq: number
  online: bool

  resource live = live(persona) as shape Live
  resource board = notes(persona, live.cursor, time.epochAtZero + performanceNow()) as shape Board
```

`Snapback.changes({ origin, headers, wait = 20 })` returns a hand-written
async iterator (no lowering, D1) that yields `{ cursor, seq, online }`.
The server holds a poll at most 25 s; `wait` is clamped to that.

1. Its first turn asks `GET /changes?since=0&wait=0&heartbeat=1`, which
   answers the server's head at once, and yields it. That is the first
   message, in flight until it lands (D3).
2. Every later turn asks `GET /changes?since=<the last seq>&wait=<wait>&heartbeat=1&ephTables=&store_id=<the last store>`
   as independent HTTP (D4). The server answers when its head moves past
   the cursor, when its store is replaced, when its backend generation
   changes, or when the wait runs out. The iterator yields when any of
   the three changed, and otherwise asks again. `ephTables=` names no
   ephemeral table: without it the server also ends a poll every 100 ms
   while any ephemeral row (presence, typing) moves, and the stream would
   re-poll at that pace. This stream carries durable changes only (open
   question 2). A reset of the server's ephemeral incarnation still ends a
   held poll within 100 ms; the cursor is unchanged, so the stream asks
   again without a message.
3. On a transport failure, a refusal, or a non-2xx reply, it yields
   `{ cursor: <last>, seq: <last>, online: false }` and returns. It does not
   set `failure(live)`: the app reads `online`. Before any head that is
   `{ cursor: '', seq: 0, online: false }`, and it is the first message on
   purpose: an app that starts offline asks its readers once, and they
   answer from what the device keeps. The app resumes by asking `live`
   again: `refresh live` from an action, or from a `task` whose period is
   its retry interval.

`cursor` is `<store_id>:<generation>:<seq>`. It changes whenever anything a
reader must ask again for changes, so readers take `live.cursor` as an
argument, not `seq`, which a replaced store or a new generation can leave
unchanged.

The stream's cursor is the last head it announced, not the partition's
watermark, which `db.poll()` uses. The watermark moves only when the
device syncs, and the readers sync only after the message commits, so a
poll from the watermark would be answered at once, again and again, until
they had. The stream needs no partition, no device call and no storage.
Between a poll's reply and its yield there is only the JSON parse.

Its `return()` aborts the poll in flight at once on every executor, with
its own `AbortController` (D5), and `heartbeat=1` lets the server free the
worker within 2 s.

Each reader takes `live.cursor` as an argument. A change moves it, so the
reader asks again (LLP 1016 D5) and its answer starts with `db.sync()`
(one round at a time per partition, as the client runs them) and reads
the device. Mutations keep `refreshes`. Snapback pays one long-wait worker
per live client, as Snapback's own clients already do (32 per host,
`request_workers.rs`). That bound is the server's, and it is why D9's
evented endpoint is still wanted.

README's "Live updates" is rewritten around it. The timer pattern stays
documented as the fallback.

### D9 — Later: Snapback's changes as an event stream, behind the same call

r1's D1 stands as the later transport. With it, `GET /changes?stream`
holds the response and sends one event each time the viewer's head moves
past the cursor, with the new cursor as the event's `id` and heartbeats
between. Its admission, credentials and interest are `/changes?wait`'s,
and it coalesces.

It is the Snapback lead session's to design, because Snapback 4's HTTP
host is `tiny_http` 0.12, which is blocking and thread-per-request behind
fixed pools (32 request workers, 32 long-wait workers, 128 connections).
Serving many mostly idle streams needs idle streams that cost no thread.
That is a change in the host's concurrency model. When it exists,
`Snapback.changes()` returns `fetch(url, { exactStream })` instead of a
generator. An answer may be either (LLP 1016.000 as built), and the app's
call, its `Live` shape and its readers do not change. Until then the long
poll is the transport, and its cost is D8's.

## Cost and what it touches

- **Runner:** `Answer::message`, `Request::yielded` and `Request::iterator`;
  the pending entry's iterator mark, its `StreamCount` carried across
  rounds, and `local` (D3); the round enqueued before the commit settles,
  and taken back out when it is refused; release on any refused commit; a
  send's refusal; `Runner::is_open`. Tests (`iterator_tests.rs`): rounds
  under their own tickets, a round with no message, an end, a throw, new
  arguments, an equal-argument refresh, a first turn that yields, a refused
  commit, a send, the first round as a stream in flight and never kept.
- **Prelude (native and the module realm):** the pump, tags 3/1 marked and
  tag 4, the turn's coalescing and its 1,000-yield bound, the 16-stream
  bound, `return()` on forget, the refusals (a promise of an iterator,
  `exactStream` inside). **Bake:** `lowering` for async generators.
- **`exact-js`, `js/web`, `data`:** tag 4 to `Answer::message`, the mark on
  every round, a send refused at the call; the envelope's refusal;
  `Storage<D>` carrying both fields. Tests (`js/tests/it/iterate.rs`, a
  lowered fixture through the pinned VM): each turn's message and request,
  coalescing, a round with no message, the end, a throw, a first turn that
  yields, the bounds and refusals, a hand-written iterator, through the
  runner, forget running `finally`, a send.
- **Glue:** `exact_request_active`'s 2. **JS target:** the opener in
  `ts-data.js`, the tied signal in `ts-fetch.js`, the send's refusal in
  `rt.js`. **Types:** `Messages<S>`.
- **Snapback client:** `Snapback.changes()`, its tests against a real
  `snapback4 dev` (a second client's write arrives as a message with no
  timer; `return()` aborts the poll at once; unreached, it says offline and
  ends), README.
- **Snapback server:** nothing now (D9 later).
- **Contract:** nothing.

## Alternatives considered

- **Keep the timer** (today). Correct and simple, at the cost described in
  Motivation.
- **A held long poll inside an answer** (LLP 1016.000 D6). It holds `clock
  settle`, and an answer has one value, so it re-asks per change anyway.
- **SSE first** (r1). The right end state (D9), blocked on Snapback's
  concurrency model.
- **An iterator consuming `exactStream`** (`for await (const e of
  exactEvents(url))`). It is more general, but it gives a host stream to a
  realm that has none on the web (the JS target cannot tie a later turn's
  stream to its answer), and nothing needs it yet.
- **Push into the device without Contract.** A data module cannot start a
  commit between answers (LLP 1027.000), and the resource re-ask is
  already the mechanism.

## Does anything refuse this?

- **`rules/DEFERRED.md`:** no. Its only "push" refusal is update delivery's
  service half. LLP 1069.004 records Charlie's ruling that "nothing there
  refuses live data for apps" and that no trade is needed. "Server-side
  anything" there is the update economy's.
- **LLP 1027 D1, "Synchronous, on purpose":** superseded by D1a (an
  answer may be a promise). An iterator is the same continuation, resumed
  more than once, with I/O still a request the host runs.
- **LLP 1027.000 (no timers):** kept. The iterator waits only on host
  work, never on time. A retry interval stays a Contract `task`.
- **LLP 1016.000 "Not built: worker-placed TypeScript streams":** kept,
  and D6 refuses it by name.
- **`rules/RULES.md` "agents add no apparatus":** no check, script,
  registry or config is added. This is a revision of an approved document.
- **Charlie's 2026-10-08 approval** was of r1's direction (SSE, by
  Snapback's lead session). r2 does not withdraw it (D9). It puts a client
  mechanism first that is needed anyway, so that live reads do not wait on
  the server. That ordering is his to confirm.

## Open questions

1. Should an event say which tables moved, so only resources that read
   them re-ask? `/changes` already answers `tables`, but a resource does
   not yet declare which tables it reads. `Snapback.changes()` could yield
   them for an app that filters by hand.
2. Ephemeral state (presence, typing) rides the same poll in Snapback's
   own client (`eph`, `ephi`, `ephTables`). This stream opts out
   (`ephTables=`, D8). Whether a second stream should carry it waits for a
   consumer.
3. Where the cursor lives across a reload: the first turn starts from the
   server's head, which is right for a reader that syncs anyway. An app
   that keeps its own cursor would pass `since`.

## Dispositions

Review findings and what became of them, by round. Each review is under
`llp/reviews/` (`rfc-2026-10-08-1110-r2*.astra.md`, `…grok.md`).

### Round 1 (r2 as first written)

**GPT-6 Astra — NOT READY.**

1. *Blocker: async generators do not compile on native.* Accepted. The
   pinned `hermesc` refuses them. D1 now specifies the bake's lowering (the
   whole bundle to ES2017 when captured sources declare one) and why
   `Snapback.changes()` is a hand-written iterator. The executor tests run
   lowered generators through the pinned VM. Types: `Messages<S>`.
2. *Blocker: iterator identity arrives too late.* Accepted. D2 adds
   `Request::iterator` on every round from the first: never kept, refused
   for a send at its first round (and natively at the call), listed in
   `state.streams` from its first round. The 16 are reserved when `answer`
   returns the iterator (D4).
3. *Blocker: the message-plus-next-round transaction.* Accepted. D2 now
   says: the next round is enqueued with the message's commit, before it
   settles, so settlement's own supersession (an answer now or a newer
   request) applies; a refused commit takes the round back out and
   releases the ticket for any refusal, not only data or shape;
   `forgotten`, not `discard`, cleans up an HTTP round's call. Runner tests
   cover a refused commit, new arguments, an equal-argument refresh and a
   first turn that yields.
4. *Blocker: forwarding sources.* Accepted. `Storage<D>` carries both
   fields across its rewrite. Ordered-set turns refuse as worker placement
   does, with words that name both (D6).
5. *JS-target cancellation for Snapback's repeated polls.* Accepted.
   `Snapback.changes()` owns an `AbortController`; its `return()` aborts the
   poll in flight at once on every executor and stops the loop (D5, D8).
6. *`seq` alone misses store and generation changes.* Accepted. The stream
   yields `cursor` (`store_id:generation:seq`), sends `store_id`, and
   readers take `cursor` (D8).
7. *Settle classification for storage.* Accepted. D3: after the first
   message, HTTP and long native calls are open and storage steps and
   continuations stay in flight, on every executor; the module realm
   carries a message across a turn's storage steps (D6).
8. *The server's worker is not freed without heartbeats.* Accepted.
   `heartbeat=1`; the cancellation latency is the server's 2 s beat (D5).
9. *Loopback speed is not a delivery barrier.* Accepted. D3 drops the
   argument and asks a drive to re-ask `clock settle`, bounded.

**Grok 4.7 — READY WITH CHANGES.** (It read r2 while round 1's folds were
being written, so it reviewed most of them too.)

1. *Ephemeral wakes turn the long poll into a 100 ms re-poll.* Accepted.
   `Snapback.changes()` asks with `ephTables=`, naming none, and D8 and
   open question 2 say so.
2. *The first failure has no last cursor; a stream that never had a head
   should fail.* Not taken. An app that starts offline must still read the
   device, and its readers ask only once `live` has a value; a failure
   would leave them unasked. The first message `{ cursor: '', online:
   false }` asks them once, not again. D8 says so, and that `failure(live)`
   is not set on this path.
3. *D7 describes the runner's tickets; the JS target differs.* Accepted.
   D7 is split by executor.
4. *Nits:* the journal's words are the runner's and the JS target's,
   quoted per executor (D7); the server's 25 s ceiling is stated (D8); D3
   says a native operation delivers one held completion as it begins and
   pumps on only while something is in flight.

### Round 2 (r2 with round 1 folded; reviewers read a frozen tree with the lane's implementation)

**GPT-6 Astra — NOT READY.** By the lane's rule (two rounds), the lane
stopped here: this revision and the four reviews land; the implementation
does not. Each finding's design point is now in the text, marked *owed*
where the lane's code falls short.

1. *Blocker: a refused refresh can restore a ticket whose iterator was
   already let go, and the old reply then resumes the new iterator.*
   Accepted (D2: rounds keyed by logical call; cleanup deferred to
   `forgotten` after the commit). Owed, with a test: an equal-argument
   refresh, a downstream refusal, then the original reply.
2. *Blocker: decode, shape and budget failures strand native and wasm
   iterator calls.* Accepted (D2: every failure retires the call). Owed,
   with a test of repeated malformed yields that checks `finally`,
   outstanding work and the reservation.
3. *Blocker: two JS-target lifetime leaks* (pulling after a failure; a
   reservation taken at `answer` and never released when the answer is
   discarded). Accepted (D4: reserve at open, one idempotent disposal).
   Owed.
4. *The browser Worker refusal is missing.* Accepted (D6). Owed.
5. *Storage settlement changes `pending(resource)`.* Accepted (D3:
   `pending(r)` and settle are separate predicates). Owed.
6. *Native lowering misses valid syntax and dependencies.* Accepted (D1:
   detect by parsing the bundle). Owed.
7. *Nit: a finite iterator becomes a compiled value at bake.* Accepted
   (D1: deferred whatever it does). Owed.

**Grok 4.7 — READY WITH CHANGES.**

1. *A watched topic asks an iterator again at once, even mid-round, and a
   round with no message drops the mark.* Accepted (D2: mark while in
   flight, ask now only when open, carry the mark through a quiet round).
   Owed, with runner tests during the first fetch, a storage round and a
   no-message round. (The lane carried the mark through a no-message round
   after the frozen tree; `changed` is still owed.)
2. *Nits:* D3 now says every native operation delivers one held
   completion as it begins and `clock settle` pumps on; D6 refuses worker
   placement and ordered sets before the pump (also Astra 4); D5 states
   the HTTP/1.1 gate on heartbeats; D1's lowering is decided by parsing,
   so a comment cannot trigger it; D8 says an ephemeral incarnation reset
   ends a poll and the stream asks again without a message.

### What the lane verified (round 1's design, on `lane/iterator-streams`)

For round 3's implementer, what already ran: runner tests
(`iterator_tests.rs`, 10); `exact-js` tests through the pinned Hermes VM
(`js/tests/it/iterate.rs`, 9, a lowered fixture); `bun test snapback4/ts`
against a real `snapback4 dev` (17, two new: another client's write
arrives as a message with no timer, within the test's 5 s bound;
`return()` aborts a held poll at once; unreached, it ends offline); the
README guestbook as written, `test web`; JS-target cross-browser
conformance (`conformance/iterate.contract` on RealWorld's module, Chrome
against Firefox and WebKit, 6 of 6 steps each); and drives with no task
timer, a write in one session seen by another after one `clock settle`:
two JS-target pages, two wasm-target pages (the module realm, a ticket per
round), and a web page writing to the Linux host (native Hermes and the
native executor). The five checks and `contract-difftest -- quick` passed
on the lane's tree. The wasm target's synthetic conformance cannot swap a
plan into a TypeScript module's dist ("module reload requires a paired
generation"), so the cross-browser run stands in for it.
