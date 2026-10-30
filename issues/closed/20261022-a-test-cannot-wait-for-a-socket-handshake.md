# A test cannot wait for a socket's handshake: `clock data` stops at its first message

**Status:** Closed
**Resolution:** Built the ask's second shape as ruled (Charlie, 2026-10-26): `clock until state <path> == <literal> within <ms>`, a form of `clock` in the driver (scripts/agent-held.mjs) and in authored tests, waits on real time with the clock frozen and fails naming the last value; Excord's first test uses it after `clock settle`, which runs the `after(1, …)` task that sends IDENTIFY.
**Systems:** the agent's `clock data` and an authored test's launch wait (`scripts/agent-test.mjs`, the runner's stream settlement, LLP 1016.000 D5), every host
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's wiring lane (`apps/excord/llp/0001-excord.rfc.md` §4.2)
**Date:** 2026-10-22

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): an ask

## Symptom

An authored test waits for the app's data before its first step and after a
`reload`, as `clock data` does. A stream counts as landed at its first message
(LLP 1016.000 D5). A WebSocket protocol whose useful state comes several frames
later is then still opening when the test's first step runs: Discord's gateway
sends HELLO, the client answers IDENTIFY, and READY (the account's servers and
channels) arrives on real time after it. On the web a first launch's step usually
finds READY there, because the driver's own overhead covers it; after a `reload`
it does not:

```
bun scripts/agent.mjs web --app apps/excord --storage d1 --epoch 2026-10-22T18:00:00Z \
  "clock data" "state gateway.state" "reload" "state gateway.state" "state channels" \
  "clock +300 real" "state gateway.state"
→ "ready" · reloaded · "identifying" · [] · real=314 · "ready"
```

(The fixture server must be running: `bun apps/excord/fixture/server.ts --world
transcript --reset-on-identify`.) Excord's tests therefore begin with `clock +500
real` after each launch, which moves the virtual clock too and is a guess at a
bound, not a condition.

## Ask

A way for a test to wait for the stream's protocol rather than its first frame,
without real time. Two shapes would do:

- `clock data` (and the launch wait) also lands each open stream's messages that
  arrive while anything the stream's own frames caused is in flight, until the
  stream has been quiet for a short real-time window; or
- a test step that waits, on real time and with a bound, until a state holds:
  `wait state gateway.state == "ready" within 2000`.

The second is general (any backend that pushes), and fails with what it saw.
