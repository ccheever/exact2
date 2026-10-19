# Review round 2, LLP 1121 stage 10 A1, Grok (xhigh), security pass

Reviewed: c3ba38c43c (each later finding fixed in the next sha; the landed A1 is the last re-checked one).

## BLOCKING

**A below-write hold can still drop another client's proposal** — `scripts/tamber-server.mjs:852`

`room()` (`786`) runs before `hold()` awaits the planner (`825`). Two overlapping `propose`, `import`, or `revert` calls can both see `proposals.size < 64` and both continue. After the await, each `hold()` inserts, then the loop forgets `proposals.keys().next()` — the oldest proposal, any owner — once the size passes 64. A propose client that owns nothing, sending two holds while 63 are held, evicts someone else. `scripts/tamber-pairing.test.mjs:120` posts one at a time, so it stays green. Evict in the same turn as the insert, and below write forget only that caller's own proposal (or refuse).

## SHOULD-FIX

**`room()` forgets the caller's proposal before a hold exists** — `scripts/tamber-server.mjs:790`

The drop is committed, then `importHeld` / `revert` / `hold` can still fail. `import` checks `kind` only after `room()` (`614`, then `1001`), a planner refusal returns at `825`, and an import that proposes nothing returns 200 at `1007` with `proposal: null`. At the cap, any of those leaves the caller's oldest proposal gone. Forget only when the new proposal is inserted.

## NIT

**`dispatch` defaults `local` to true** — `scripts/tamber-server.mjs:568`

`handle` calls `dispatch(request, scope)` (`554`) and does not pass `local`. That matches the only token that exists today. The default is fail-open: once a method needs `local`, a caller that omits the flag is allowed through, and `hello` reports `local: true`. Default `false`, and have the local-token path pass `true`.

## What checks out

Round 1:

- **refresh.** `needs()` is `write` unless `refresh === true` (`scripts/tamber-pairing.mjs:72`). Below write, `inspect` returns `lastSelection` and does not call `channel.inspect` (`scripts/tamber-server.mjs:649`). `on` without `refresh` is 403. `refresh: true` together with `on` still takes the cache path, so inspect mode is not toggled and no build is asked.
- **Cap, sequential.** Below write, at 64, the caller's own oldest goes; with none of their own, 409. A write client still trims the oldest. The sequential test covers that.
- **Stream, envelope, docs, `hasOwn`.** `/__tamber/subscribe` and `/__tamber/events` go through `needs` / `allows` before the body is acted on; a refusal is `{error: {message, scope, needs}}`. The status table lists 403 and the below-write 409. `hello` adds `scope` and `local`. `needs()` uses `Object.hasOwn` (`tamber-pairing.mjs:70`). A method the table does not name is 400; an unknown scope allows nothing.

**Cache.** `lastSelection` is the object spread into the `selection` event (`239–240`). `send` broadcasts that event with no `only`, and both stream paths are `read`, so a read client already receives it (and the build channel's `selected` event). The cache is that same payload, limited to builds still in `channel.list()`. It does not carry `only` hold-refusals. A later joiner can read a selection after it has left the 256-event buffer; that is the same public payload refresh is specified to return.

**A2.** Nothing in these checks has to move. The client string is read only after `parse`: subscribe (`500`), events (`517`), and `needs` / `room` / `drop` inside `dispatch`. Overwriting `request.client` from the token before those branches covers drop ownership, whose proposal `room()` treats as own, presence, and `only` delivery. Pass `local: false` into `dispatch` for a paired token (the NIT above). The design server's hello still stores a fallback writer (`contract/cli/src/tamber_serve.rs:657`); this wire never uses it. Writes go through `needsClient`, and `propose` plans with `write: false` and returns before commit.

**Tests.** The table matches `METHODS`, scopes are ordered, `local` is the flag, every method above read and propose is 403 with both scopes named, drop of own versus another, the stream envelope, inherited names, and the sequential cap. Not covered: refresh returning a selection a build actually published (the test only checks `{selections: []}` and that no frame was sent), overlapping holds at 63–64, and a failed or empty import leaving the caller's proposal in place.
