# Data modules cannot send on a WebSocket (receive-only on every host)

**Status:** Open
**Systems:** runner streams, data modules, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/126

## Current scope

Preserve Charlie's selected next core investment: runner-owned sending under net.websocket grants. Select stream-handle/mutation-send API, distinguishing ordered protocol frames from coalescible snapshots. Verify ack/ping/backpressure/close/disposal; ambient globals and #227 refusal consistency are separate.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
A TypeScript data module can open a WebSocket only as a receive-only stream (`fetch("ws://…", { exactStream })`). On macOS no frame can be sent: the stream mapper gets no socket, and `WebSocket` is undefined in the module. On the web JS target the browser's own `WebSocket` is reachable in the module and does send, so the hosts disagree. Ask: give data modules the web `WebSocket` API (`send`, `close`, `onmessage`, `onclose`, `readyState`, `bufferedAmount`) on every host, under the `net.websocket` grant.

## Why it matters
A coding-agent desktop app talks to its server over one WebSocket that carries an RPC protocol. The client sends Request, Ack, Interrupt, Ping and Pong frames. A terminal stream holds back output once 8 chunks are unacknowledged or 64 KiB are pending, so a client that never sends Ack stalls the terminal. A device viewer sends keys and touches over its own socket. Without send, apps write a native transport (Swift `URLSessionWebSocketTask`, about 2,200 lines in one app) and poll it from TypeScript through a native module. That transport has hard limits (an inbox that resets under load, a cap on open streams), and each new stream needs native code. Every other app that talks to a WebSocket server must build the same thing.

## Current behavior (exact2 4c893fef6)
- `docs/reference.md:510-512`: "a `ws:`/`wss:` URL is a receive-only WebSocket under `net.websocket` alone (no frame is ever sent, so a feed that waits for a subscribe frame cannot be read)". Messages that arrive faster than they commit coalesce to the newest.
- Receiving works on both hosts: `Text#2 [text] "message: hello from server"`.
- macOS: `Text#3 [can-send] "mapper socket: undefined; global WebSocket: undefined"` and `Text#4 [direct] "threw: TypeError: undefined cannot be used as a constructor."`. The server log shows `open` and no received frame.
- Web JS target: `"mapper socket: undefined; global WebSocket: function"`, and `new WebSocket(url)` in the module sends: `server: received ping from module`, `Text#4 [direct] "echo ping from module"`.
- Hypothesis: the web build does not guard `WebSocket` in the module realm as it guards timers and `Date` (LLP 1027.000 D3). Whether that path checks the `net.websocket` grant was not tested.

## Expected behavior
Web standard (Chrome): `new WebSocket(url)`, `send(text | ArrayBuffer)`, `onopen`/`onmessage`/`onerror`/`onclose` (code, reason, `wasClean`), `close(code, reason)`, `readyState`, `bufferedAmount`, `binaryType`. **Proposal** for exact2:
- The same API in data modules on macOS, iOS, Linux and both web targets, checked against `net.websocket` grants.
- Either a socket object passed to the `exactStream` mapper (`(event, socket) => …`) that a mutation can keep and send on, or the global `WebSocket` constructor. One documented form, the same on every host.
- `send` before open throws, as on the web. Frames go out in order. A socket that sends delivers every message (bounded and documented), not only the newest.
- Close codes and reasons reach the app; reconnect is the app's job. Sockets close with the resource or the window.

## Reproduction
Minimal app (`app.ts` and the echo server are in `evidence/X21/app/`):
```contract
shape Feed
  text: string
  canSend: string

shape Direct
  text: string

component X21WebsocketSend
  resource feed = feed("ws://127.0.0.1:8791/ws") as shape Feed else empty(text="waiting", canSend="?")
  resource direct = direct("ws://127.0.0.1:8791/direct") as shape Direct else empty(text="waiting")
  view
    main testId="root" padding=24
      text feed.text testId="text"
      text feed.canSend testId="can-send"
      text direct.text testId="direct"
```
`app.ts` (grant `net.websocket ws://127.0.0.1:8791`): `feed` returns `fetch(url, { exactStream: (event, socket) => … })` and tries `socket?.send(…)`. `direct` runs `new WebSocket(url)`, sends `ping from module` on open, and answers with the echo.

| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x21-websocket-send`; files as above; `contract build --json` | compiler | `[]` | `[]` |
| 2 | `bun .exact/echo-server.ts &`; `agent web "clock data" "clock +500 real" tree logs` | web (Chrome) | receives; mapper socket `undefined`; `new WebSocket` sends and gets `echo ping from module` | a documented send path, grant-checked |
| 3 | `bun exact.mjs mac`; same drive with `agent macos` | macOS 26 | receives; no socket; `WebSocket` undefined; server receives nothing | the server receives `ping from module`; `direct` shows the echo |

### Evidence
The files are attached at the end of this issue, in folded sections.
Quoted from `repro.log`:
```
macOS  Text#3 [can-send] "mapper socket: undefined; global WebSocket: undefined"
macOS  Text#4 [direct] "threw: TypeError: undefined cannot be used as a constructor."
macOS  server: open / server: close 1006          (no "received")
web    Text#4 [direct] "echo ping from module"
web    server: received ping from module
```
- `evidence/X21/repro.log`: commands, `tree`, `logs` and the echo server's log for both hosts; the quoted reference text.
- `evidence/X21/app/`: the minimal app and `echo-server.ts`.

## Acceptance criteria
- The minimal app on macOS, iOS, Linux and both web targets: the server logs `received ping from module`, and the view shows the echo.
- A conformance case against Chrome with the echo server: text and binary frames both ways, order, close code and reason, `bufferedAmount` back to 0, `send` before open throws.
- A socket to an origin outside the grants is refused on every host, the web JS target included.
- `state.streams` lists an open two-way socket; `logs` name its sends and close.

## Notes
- Workaround: a native WebSocket transport in a native module, polled from TypeScript. It costs native code per protocol and has inbox limits.
- Related: a `net.websocket` grant names one exact origin, so a server address the user types at run time needs another grant form (separate gap).
- Not tested: iOS, Linux, the wasm web target; binary frames; whether the web JS target's `WebSocket` honors grants.
- Related LLPs: 1016.000 (streams), 1027.000 (guarded globals in the web build).

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef65f1bf8d6e4be6df9265e2ae49790637 (2026-10-05), checkout exact2-repro, macOS 26.6.2, bun 1.4.2

# app/echo-server.ts: Bun echo server on 127.0.0.1:8791 (greets on open, logs and echoes each client frame)
$ bun exact.mjs contract build app.contract --json
[]

## Web (JS target, agent's Chrome)
$ bun .exact/echo-server.ts &
$ bun exact.mjs agent web "clock data" "clock +500 real" tree logs
{"clock":0,"settled":true}
{"clock":500,"real":519}
epoch 3 · incarnation 1 · clock 500 ms · 4 nodes
View#1 [root]
  Text#2 [text] "message: hello from server"
  Text#3 [can-send] "mapper socket: undefined; global WebSocket: function"
  Text#4 [direct] "echo ping from module"
t=0 boot: 4 nodes, epoch 1
t=0 reply 2; wall 103 ms
t=0 message 1; wall 115 ms
-- server log:
server: listening http://127.0.0.1:8791/
server: open
server: received ping from module
server: open
server: close 1006
server: close 1006

## macOS (Hermes data module)
$ bun exact.mjs mac          # cargo 62.2 s
$ bun .exact/echo-server.ts &
$ bun exact.mjs agent macos "clock data" "clock +500 real" tree logs
{"epoch":2,"settled":true,"incarnation":1,"clock":0}
{"incarnation":1,"clock":500,"epoch":2,"real":503}
epoch 2 · incarnation 1 · clock 500 ms · 4 nodes
View#1 [root]
  Text#2 [text] "message: hello from server"
  Text#3 [can-send] "mapper socket: undefined; global WebSocket: undefined"
  Text#4 [direct] "threw: TypeError: undefined cannot be used as a constructor."
t=0 boot: 4 nodes, epoch 1
t=0 direct shows its build-time answer until its source answers
t=0 query feed: feed
t=0 query direct: direct
t=0 direct answered: equal to its build-time answer
t=0 request 1 (feed): GET ws://127.0.0.1:8791/ws
t=0 data_ready (2 asked again) → epoch 1 (+0 −0 ~0)
t=0 fulfil 1 (feed) [message, 17 bytes; wall 3 ms] → epoch 2 (+0 −0 ~2)
-- server log:
server: listening http://127.0.0.1:8791/
server: open
server: close 1006

# docs/reference.md:506-512 at this revision:
An answer that keeps coming (LLP 1016.000) is a `fetch` with `exactStream`,
returned as the answer: `return fetch(url, { exactStream: (event) => value })`.
The promise never settles; each message, and the end, is mapped now (the
mapper cannot await) and commits as the resource's answer. An `http:`/`https:`
URL is read as server-sent events under `net.fetch`; a `ws:`/`wss:` URL is a
receive-only WebSocket under `net.websocket` alone (no frame is ever sent, so a
feed that waits for a subscribe frame cannot be read). An event is
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
// X21: a data module opens a WebSocket, hears the server, and tries to send.
shape Feed
  text: string
  canSend: string

shape Direct
  text: string

component X21WebsocketSend
  resource feed = feed("ws://127.0.0.1:8791/ws") as shape Feed else empty(text="waiting", canSend="?")
  resource direct = direct("ws://127.0.0.1:8791/direct") as shape Direct else empty(text="waiting")
  view
    main testId="root" padding=24
      text feed.text testId="text"
      text feed.canSend testId="can-send"
      text direct.text testId="direct"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X21 Websocket Send",
  "short_name": "X21 Websocket Send",
  "id": "com.example.x21-websocket-send",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x21-websocket-send",
    "name": "X21 Websocket Send"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.test.contract`

````contract
test "the greeting loads"
  expect tree has "root"
  expect text "greeting" == "Hello from X21 Websocket Send."
````

`app.ts`

````ts
import type { Answer, Result } from './app.contract.d.ts';

export const appId = 'com.example.x21-websocket-send';
export const grants = 'net.websocket ws://127.0.0.1:8791';

// The documented form: a receive-only stream. Is there a way to send a subscribe frame?
const feed = (url: string) =>
  fetch(url, {
    exactStream: (event: any, socket?: any): Result<'feed'> => {
      let canSend = `mapper socket: ${typeof socket}; global WebSocket: ${typeof (globalThis as any).WebSocket}`;
      try { socket?.send?.('{"_tag":"Request","id":"1"}'); } catch (e) { canSend += `; send threw ${e}`; }
      return { text: `${event.type}: ${event.data ?? event.message}`, canSend };
    },
  } as any) as any;

// The web's own API: open, send one frame, answer with the echo.
const direct = (url: string): Promise<Result<'direct'>> =>
  new Promise((done) => {
    try {
      const ws = new (globalThis as any).WebSocket(url);
      ws.onopen = () => ws.send('ping from module');
      ws.onmessage = (m: any) => { if (String(m.data).startsWith('echo')) done({ text: String(m.data) }); };
      ws.onerror = () => done({ text: 'error event' });
    } catch (e) { done({ text: `threw: ${e}` }); }
  });

export const answer: Answer = (source, [url]) =>
  (source === 'feed' ? feed(url as string) : direct(url as string)) as any;
````

`echo-server.ts`

````ts
// Echo server: greets on open, logs and echoes every client frame.
const server = Bun.serve({
  port: 8791, hostname: '127.0.0.1',
  fetch(req, srv) { return srv.upgrade(req) ? undefined : new Response('ws only', { status: 400 }); },
  websocket: {
    open(ws) { console.log('server: open'); ws.send('hello from server'); },
    message(ws, msg) { console.log(`server: received ${String(msg)}`); ws.send(`echo ${msg}`); },
    close(ws, code, reason) { console.log(`server: close ${code} ${reason}`); },
  },
});
console.log(`server: listening ${server.url}`);
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T08:15:27Z

## Decision needed
**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation. Re-checked on main `78286adc1` (2026-10-07). Since #202 the web JS target also refuses a module's own `WebSocket`, so every host is receive-only now. Receiving through `fetch("ws://…", { exactStream })` works.

**Blocked by:** no DEFERRED line, but the accepted stream design has no outbound frame: LLP 1016.000 implements "a receive-only WebSocket (slice 3)", and `llp/1069.004-building-answers-that-keep-coming.plan.md:178` says "1016.000 has no outbound frame, and this slice doesn't add one". `docs/reference.md:525` documents receive-only.

**Options:**
- **A.** A socket handle given to the `exactStream` mapper that a mutation can `send` on.
- **B.** The global `WebSocket` constructor in data modules on every host.

Either way: one documented form, checked against `net.websocket` grants, ordered and bounded delivery, and close code and reason reaching the app.

**Recommendation:** admit A, as an LLP 1016.000 amendment. It keeps sockets owned by the runner (`state.streams`, the agent, grants). The T3 clone carries about 2,200 lines of native `URLSessionWebSocketTask` transport only because a module can't send an Ack or Ping.

**Cost:** large: the runner, the stream mapper, four hosts, and a conformance case against Chrome with an echo server.

**In review:** #227 makes every host refuse a module's own `WebSocket`/`XMLHttpRequest`/`EventSource` the same way. Before it, `typeof WebSocket` was a refusing stub on the web and `undefined` on macOS. A direct use now also fails the build, by file and line. That build-time refusal is a new rule, so it is held as a draft for a ruling along with sending.

### ccheever — 2026-10-08T08:07:34Z

**Decision: Choose runner-owned bidirectional streams, not ambient WebSocket globals.**

Selected as the next core investment by Charlie on 2026-10-08; keep open for design and implementation.

T3 Code and the already queued Supabase/Convex consumers make sending a reusable capability. Preserve net.websocket grants, lifetime ownership, inspection and bounded transport.

Charlie selected this priority on 2026-10-08. Select the stream-handle/mutation-send API and distinguish protocol frames from coalescible state snapshots. Verify subscribe/ack/ping, ordering, backpressure, close reason and disposal. Review #227 independently; it fixes refusal consistency, not sending.
