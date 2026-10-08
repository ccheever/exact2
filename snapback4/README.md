# Snapback4 for Exact apps

One Snapback4 client for an Exact app's Rust and TypeScript, on every host:
the device (its partition in SQLite, its outbox, local queries and
predictions) and the client protocol over it (opening, sync rounds, store
identity and generation adoption, at-most-once sends with receipts, the
change poll), written once in Rust and performing no I/O. Each HTTP exchange
is handed to whoever drives it. Pinned to Snapback4 **0.4.13**
(`67b2ce28a3`); the npm CLI in the root `package.json` matches.

| | |
|---|---|
| `client/` (`exact-snapback4-client`) | the protocol: `Client` (`read`, `write`, `sync` → `Step::Fetch`/`deliver` → `Step::Done`, `changes`/`changed`, `refresh`/`refreshed`, `outcome`, `status`), the JSON `dispatch`, partition binding |
| `src/` (`exact-snapback4`) | the native device under the app's grants and directories: `Module` (an Exact `NativeModule` or `DataSource`), and `host_request`/`host_reply` for a Rust source's `Answer::Later` |
| `web/` (`exact-snapback4-web`) | the same client as wasm over the device's memory store; `web/build.mjs` builds it with its wasm-bindgen glue |
| `ts/` | the TypeScript driver an app's `app.ts` imports: `Snapback.open`, `read`/`readAll`, `write`, `sync`, `poll`, `refreshSession`, `outcome`, `status` |

## From TypeScript

### Mount it and build the wasm

Mount the driver in the app's `app.json` (the path is relative to the app,
or absolute; an app made by `exact new` outside this checkout names this
checkout's `snapback4/ts`), and build the wasm and its glue, which every
host's bake type-checks, before a build:

```json
"typescript": { "sources": { "snapback4": "../exact2/snapback4/ts" } }
```

```sh
node ../exact2/snapback4/web/build.mjs assets/snapback4.wasm   # the web; no argument writes only the glue
```

The first build compiles the client for a few minutes; later ones take
seconds. The web host serves `assets/snapback4.wasm` at
`/assets/snapback4.wasm`, the driver's default. It is pinned to Snapback4
0.4.13: install that version of `snapback4` for the server.

### Time

A data module has no clock. Declare the host's clock as a resource and pass
milliseconds since the epoch into every source that reads or writes:

```
shape Clock
  epochAtZero: number

  resource time = exactTime() as shape Clock
  resource inbox = inbox(time.epochAtZero + performanceNow()) as list<Message>
```

`exactTime()` is a reserved source, not a function to call inside an
expression; `performanceNow()` is milliseconds since the app started, never
the date. Under the agent and in tests the date is the drive's epoch
(`2026-01-01T00:00:00Z` unless `--epoch` or a test's `epoch` line says
otherwise), not the real time, while the Snapback server runs on real time.
So compute deadlines and expiries on the server (`now` in a mutation) and
compare times the server returned with each other; a deadline the app
computes from its own clock disagrees with the server in every test.

### A complete app

A guestbook: two personas, a list that syncs and works offline, a post whose
fate the app reports. `snapback/schema.q`:

```quarry
use personas alice, bob

table notes:
  author: principal
  body: text 1..200
  at: time
  by byTime: at, id
  public 'notes are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 100 by byTime

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200):
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
```

`app.contract`:

```
// Guestbook: the view. app.ts answers it through Snapback4.
shape Clock
  epochAtZero: number
shape Note
  id: string
  author: string
  body: string
  pending: bool
shape Board
  online: bool
  message: string
  notes: list<Note>
shape Ack
  ok: bool
  message: string

component Guestbook
  state persona = "alice"
  state draft = ""
  state notice = ""
  resource time = exactTime() as shape Clock
  resource board = notes(persona, time.epochAtZero + performanceNow()) as shape Board
  mutation posted as shape Ack queue refreshes board then afterPost
  task live mount
    every(5000, refreshBoard)
  action refreshBoard
    refresh board
  action choosePersona(value: string)
    persona = value
  action editDraft(value: string)
    draft = value
  action postNote
    send posted = post(persona, trim(draft), time.epochAtZero + performanceNow())
  action afterPost
    match posted
      case some(result)
        notice = result.message
        draft = ""
      case none
        notice = ""
  view
    main testId="root" padding=24
      column gap=12 max-width=600
        text "Guestbook" role="heading" aria-level=1 font-size=24
        select value=persona change=choosePersona appearance="auto" testId="persona" aria-label="Signed in as"
          option value="alice"
            text "Alice"
          option value="bob"
            text "Bob"
        text (board.online ? "Synced" : board.message) testId="status"
        row gap=8
          input value=draft input=editDraft testId="draft" placeholder="Say something" aria-label="Note" flex=1
          button appearance="auto" press=postNote disabled=(trim(draft) == "") testId="post"
            text "Post"
        text notice testId="notice"
        each note in board.notes key=note.id
          text `${note.author}: ${note.body}${note.pending ? " (sending)" : ""}` testId=`note-${note.id}`
```

`app.ts`:

```ts
import type { Answer, NativeModule, Result, Sources, Storage } from './app.contract.d.ts';
import { Snapback } from './snapback4/snapback.ts';

export const appId = 'com.example.guestbook';
const origin = 'http://127.0.0.1:4400';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

// One partition per viewer, kept open across answers. Opening one this page
// already has open shares it, so this cache only saves the reopen.
const devices = new Map<string, Promise<Snapback>>();
function device(persona: string, storage: Storage, native: NativeModule | null | undefined): Promise<Snapback> {
  let opening = devices.get(persona);
  if (!opening) {
    opening = Snapback.open({ app: appId, name: `guestbook-${persona}`, origin, viewer: `dev:${persona}`,
      headers: () => ({ 'x-snapback-persona': persona }), storage, native });
    devices.set(persona, opening);
    opening.catch(() => devices.delete(persona));
  }
  return opening;
}

type Row = { id: string; author: string; body: string; pending?: boolean };

const sources: Sources = {
  notes: async ([persona, now], _store, storage, native): Promise<Result<'notes'>> => {
    try {
      const db = await device(persona, storage, native);
      const round = await db.sync();                    // offline is fine once synced
      const read = await db.read<Row[]>('recent', {}, now);
      if (read.denied) return { online: false, message: read.denied.message, notes: [] };
      // Project each row onto the shape: an answer's extra fields are refused.
      const notes = (read.data ?? []).map(row => ({ id: row.id, author: row.author, body: row.body, pending: row.pending === true }));
      return { online: round.ok, message: round.ok ? '' : 'Offline: showing what this device keeps', notes };
    } catch (error) {
      // At build (bake) time there is no storage; the app asks again when it runs.
      if ((error as { code?: string }).code === 'bake') return { online: false, message: 'Connecting…', notes: [] };
      return { online: false, message: (error as Error).message, notes: [] };
    }
  },
  post: async ([persona, body, now], _store, storage, native): Promise<Result<'post'>> => {
    const db = await device(persona, storage, native);
    const written = await db.write('post', { body }, now);
    if (written.state === 'failed') return { ok: false, message: `Refused: ${written.why.code}` };
    await db.sync();
    // A round that ends ok delivered the outbox; the write's own fate is here.
    const fate = await db.outcome(written.id);
    if (fate.state === 'failed') return { ok: false, message: `Refused: ${fate.why?.code}` };
    return { ok: true, message: fate.state === 'sent' ? 'Posted.' : 'Saved on this device; it sends when online.' };
  },
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

`app.test.contract` (run `bunx snapback4 dev` first; the tests talk to it):

```
test "a note posts and appears"
  clock data
  expect text "status" == "Synced"
  type "draft" "hello from a test"
  tap "post"
  clock data
  expect text "notice" == "Posted."

test "offline, a note is kept on the device and marked sending"
  clock data
  fail fetch "http://127.0.0.1:4400"
  type "draft" "from the tunnel"
  tap "post"
  clock data
  expect text "notice" == "Saved on this device; it sends when online."
```

`app.json` mounts the driver (`"typescript": { "sources": { "snapback4":
"<this checkout>/snapback4/ts" } }`), `bun add snapback4@0.4.13` installs the
server, and `node <this checkout>/snapback4/web/build.mjs assets/snapback4.wasm`
builds the device. Built and tested as written (2026-10-08).

### Open, sync, read, write

```ts
import { Snapback } from './snapback4/snapback.ts';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

const db = await Snapback.open({ app: appId, name: `inbox-${persona}`, origin, viewer,
  headers: () => ({ authorization: `Bearer ${session.token}` }),  // or { 'x-snapback-persona': persona } in development
  storage, native });                                  // Exact's own `storage` and `native`, as the source receives them
await db.sync();                                       // open on first sync, page, send the outbox
const inbox = await db.read('inbox', {}, now);         // the device answers; pending rows say so
const sent = await db.write('send', { body }, now);    // kept and predicted now, sent by the next sync
await db.sync();
const fate = await db.outcome(sent.id);                // 'sent' with the server's result, or 'failed' and why
if (await db.poll(0)) await db.sync();                 // the server's head moved
const renewed = await db.refreshSession(now);          // near expiresAt: keep renewed.session at once
```

- **Every source may open for itself.** Opening a partition this page already
  has open shares it (one device per file: Exact's storage locks an open
  database), and `close()` lets it go with the last client.
- **Rounds run one at a time.** A `sync()` while another runs, from any source,
  waits for it and then runs its own, so writes admitted meanwhile go too. It
  never answers `busy`.
- **A partition that has never synced** opens in its first round. The first
  `read` or `write` starts that round (or waits for the one running); if the
  server is not reached, it throws `E_OFFLINE`. After one sync the device
  answers offline from what it keeps.
- **One partition per viewer.** A partition binds its origin and viewer. Two
  personas (or a sign-out and a sign-in) need two names, such as
  `inbox-${persona}`, each covered by the `sqlite.open` grant; opening a name
  already open for another viewer refuses with `E_PARTITION_VIEWER`.
- **What the device answers.** A query whose tables are all synced is read
  from the partition, offline included. A query that reads an `online only`
  table or view is answered by the server (`POST /q/<name>`), marked
  `server: true`; unreached, it answers `denied` with `E_OFFLINE`, never an
  empty page. A synced table holds only its sync horizon (`sync … last 100 by
  byTime`): a read of rows outside it (`first 1` of a `last 100` horizon) is
  `complete: false`. Read in the horizon's order, or widen it. A read the
  device cannot vouch for (`loading` or `speculative`: a total over rows past
  the horizon, a row it has not acquired) asks the server too, unless a write
  is still queued here; unreached, it stays `loading` and says `offline: true`.
  Show such a read as unavailable, never as zero.
- **A round that ends `ok` delivered the outbox; it does not say each write
  was accepted.** `outcome(id)` does: `sent` with `result`, `failed` with
  `why` (the refusal's code, such as one the mutation `require`s), or
  `pending` while unsent. `refusals()` lists every refused write with its
  input until `dismiss()`.
- **Offline.** `sync()` resolves `{ok: false, offline: true}`; writes are kept
  and predicted (`pending: true` rows) and sent by the next round that
  reaches the server, once each.
- **In tests.** Let the app sync once (`clock data`) before `fail fetch`: a
  partition that has never synced cannot open offline. Assert a write's
  fate (`outcome`), not a notice the app shows while it is still pending.
- **Live updates.** A data answer has a bounded life and tests settle on
  answers, so do not hold a long `poll(20)` inside a resource. Drive
  freshness from Contract: a `task` that calls an action every few seconds,
  whose resources `sync()` (or `poll(0)`) and read again, and `refreshes` on
  the mutations that change them.

On the web the partition is the wasm device, persisted through Exact SQLite
after every call, one call at a time; a failed save rebuilds the device from
the disk and keeps what the server said. A second tab finds its database
busy.

Natively the device is the app's native module; link it in the app's Rust:

```rust
struct Snapback(exact_snapback4::Module);
impl exact_js::NativeModule for Snapback {
    fn configure_storage(&mut self, data: PathBuf, cache: PathBuf, temporary: PathBuf) -> Result<(), String> {
        self.0.configure_storage(data, cache, temporary)
    }
    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> { self.0.call(request) }
}
```

The native module holds one partition at a time.

Every exchange names itself (`fetch.exchange`); deliver its reply with that
name. A reply for a round that was cancelled, or a client since reopened, is
refused (`done.stale`), as is a superseded or repeated change-poll reply.

`refreshSession` (`POST /auth/refresh`) trades the bearer `headers()` sends
for a fresh session. The server retires the old token before it answers, so
keep the returned session before the next exchange and have `headers()` read
it; `denied.code == "E_AUTH"` means sign in again. When to refresh is the
app's: Snapback's own client asks shortly before `expiresAt`.

## From Rust

```rust
let (client, host) = module.parts();
let client = client.expect("opened");                  // after {op:"open", path, origin, viewer}
let mut step = client.sync(host);
while let Step::Fetch(fetch) = step {
    let outcome = /* the host runs */ host_request(&fetch, origin, &credentials);
    step = client.deliver(host, &fetch.exchange, host_reply(&outcome));
}
```

`tests/client.rs` drives both shapes against a real `snapback4 dev`;
`ts/snapback.test.ts` drives the TypeScript and the wasm the same way.

## Checks

This workspace is outside the root's; its lock carries the private Snapback
source. Run `cargo test -p exact-snapback4 -p exact-snapback4-client`
(`bun install` at the root first, for the server binary), `bun test
snapback4/ts`, and clippy for `exact-snapback4-web` with `--target
wasm32-unknown-unknown`.

What the TypeScript client of Snapback itself does that this one does not
yet: media and assets (LLP 1108), native jobs on the device, Following
feeds, ephemeral reads, search state, the online fallback fenced by
intersecting predictions (here a read the device cannot vouch for asks the
server only when nothing is queued at all), a server read kept live
(here a query the server answers is asked again, not invalidated by the
change poll; LLP 1110), and the 0.2.32 legacy fallback. `round.rs` names its source in Snapback's `local.ts`.
