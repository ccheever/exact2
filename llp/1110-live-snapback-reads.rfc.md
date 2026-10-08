# LLP 1110: Live Snapback reads in an Exact app

**Type:** RFC
**Status:** Draft, 2026-10-08. Not reviewed.
**Systems:** the Snapback4 client (`snapback4/ts`: a stream source beside the device; `snapback4/client`: the change cursor it resumes from); Snapback 4's HTTP host (`/changes` as an event stream; Snapback repository, its lead session); Contract (no new form: a stream resource and resource arguments, LLP 1016.000)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-08
**Implementer:** unassigned
**Related:** LLP 1016.000 (answers that keep coming: D1 a stream is a resource, D5 an open stream does not hold `clock settle`, D6 the long poll as the interim), LLP 1069.004 (SSE and receive-only WebSocket as built), LLP 1027.000 (no clock or timers in a data module), `snapback4/README.md` ("Live updates"), Snapback 4 `CLIENT-AND-OPERATIONS.md` (`GET /changes?wait`, the change cursor), the app farm's diaries (`~/appfarm`, 2026-10-07/08; summarized below)

## Summary

An Exact app on Snapback 4 sees other people's writes only when it asks
again, and today it asks on a timer. Snapback's change long poll (`GET
/changes?wait`), the one thing that makes its clients live, cannot be held
inside an Exact data answer: an answer is bounded, and an answer in flight
holds `clock settle`. This RFC makes Snapback's change notifications a
stream (server-sent events), exposes them through the client as a stream
source, and lets Contract do the rest with what it already has: the reading
resources take the stream's sequence as an argument, so a change re-asks
them.

## Motivation

The app farm built small apps from scratch on Exact2 + Snapback 4 with two
builder models. In its second arm (Exact2's first-party Snapback client
required), every diary that made updates appear without a tap did it with a
Contract `task` calling an action every 2 to 15 seconds, which `sync()`s and
reads again; none held `poll(20)`. The builders said why, in their words:

- "A long-held poll would make clock-data test settlement wait, so periodic
  sync fits this slice." (quiet-tent-capacity-0966)
- "The app does not use a hanging long poll, since each Exact data answer
  needs a bounded lifetime." (pto-coverage-planner-0432)
- "`poll()` is unused: Exact sources cannot hold timers, and a 20 s long-poll
  inside a resource would stall answers the test driver waits on; freshness
  comes from the 15 s contract task re-syncing instead." (studio-door-0755)

In the first arm (hand-written HTTP), 61 of 109 apps polled on a timer.

A timer costs every client a request per period whether or not anything
changed, shows a change up to a period late, and leaves Snapback's own change
signal unused. LLP 1016.000 D6 offered the long poll with a cursor as the
interim; in practice it collides with D5's reason to exist: a held answer is
"in flight", so a test that settles waits for it.

## Design

### D1 — Snapback serves its change cursor as an event stream

`GET /changes?stream` (or `Accept: text/event-stream`) holds the response and
sends one event each time the viewer's head moves past the cursor, with the
new cursor as the event's `id` (so `Last-Event-ID` resumes) and heartbeats
between. Its admission, credentials and interest are `/changes?wait`'s; it
coalesces (only the newest head matters to a reader), as LLP 1016.000 D4
does on the Exact side.

This is the Snapback side and its lead session's to design. What the Exact
side needs from it: the cursor in `id`, nothing per event that a reader must
not miss, and a stated bound on open streams per principal and per host,
with a refusal the client can show. The HTTP host today reserves 32
long-wait workers; a stream that pins a worker for its whole life would
exhaust them at 32 viewers, so the stream must not cost a worker while idle,
or the bound must say how few there are.

*Rejected:* a WebSocket. Exact's sockets only listen (LLP 1016.000, as built),
the stream carries nothing outbound, and SSE resumes by `Last-Event-ID` for
free.

### D2 — The client offers the stream as a source

```ts
// app.ts
live: (args, store, storage, native) => Snapback.changes({ origin, headers, name: `inbox-${persona}` }),
```

`Snapback.changes` makes the `fetch(url, { exactStream })` call at once,
before any `await` (a TypeScript stream must start while it is asked: LLP
1016.000, as built), starting from the cursor the partition last synced
(kept by the client, read synchronously from what the page holds, or from
the start when it has none), and maps each event to `{ seq }`. Failure is
data: `{ seq, offline: true }` with the last seq kept.

### D3 — Contract re-asks what reads, by argument

```
resource live = live() as shape Live
resource inbox = inbox(live.seq, time.epochAtZero + now()) as list<Message>
```

A change moves `live.seq`; the reading resources' arguments change, so they
ask again (LLP 1016 D5), and each answer starts with `db.sync()` (one round
at a time per partition, as the client now runs them) and reads the device.
Mutations keep `refreshes`. Nothing new in Contract or the runner.

In a test or an agent drive the stream behaves as LLP 1016.000 D5 says: once
its first event lands it no longer holds `clock settle`; a write by another
principal (the operator's, or a second session's) arrives as an event the
next `clock settle` delivers.

### D4 — Offline and reconnect

The stream ends on a network failure (a message, then the end, LLP
1016.000). The app re-asks `live` when it wants to resume, from a `task`
whose period is its retry interval, or on the next action; the cursor
resumes it without a full resync. Writes made offline are the device's
outbox, unchanged.

## Cost and what it touches

- Snapback 4: the event-stream form of `/changes` in its HTTP host, its
  bound and refusal, tests, and a line in `CLIENT-AND-OPERATIONS.md`.
  Tracked in the Snapback repository's issue for it.
- Exact2: `Snapback.changes` in `snapback4/ts` (the stream fetch, the cursor
  it starts from, failure as data), a test against a real `snapback4 dev`, the
  README's "Live updates" rewritten around it. No runner, host or Contract
  change.

## Alternatives considered

- **Keep the timer** (today). Correct, simple, and the cost described in
  Motivation.
- **A held long poll inside an answer** (LLP 1016.000 D6). Holds `clock
  settle`, and a re-asked or forgotten answer leaves a request outstanding
  until the server's wait ends.
- **Push into the device without Contract.** A data module cannot start a
  commit between answers (LLP 1027.000), and the resource re-ask is already
  the mechanism.

## Open questions

1. Should an event say which tables moved, so only resources that read them
   re-ask? The change poll knows its interest set; a resource does not yet
   declare which tables it reads.
2. Ephemeral state (presence, typing) rides the same poll in Snapback's own
   client; does it ride this stream, or wait?
3. Where the cursor lives across a page reload: the partition already keeps
   its watermark; the stream should start there rather than at "now".
