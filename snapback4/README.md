# Snapback4 for Exact apps

One Snapback4 client for an Exact app's Rust and TypeScript, on every host:
the device (its partition in SQLite, its outbox, local queries and
predictions) and the client protocol over it (opening, sync rounds, store
identity and generation adoption, at-most-once sends with receipts, the
change poll), written once in Rust and performing no I/O. Each HTTP exchange
is handed to whoever drives it. Pinned to Snapback4 **0.4.16**
(`468d3aa5be`); the npm CLI in the root `package.json` matches.

| | |
|---|---|
| `client/` (`exact-snapback4-client`) | the protocol: `Client` (`read`, `write`, `sync` → `Step::Fetch`/`deliver` → `Step::Done`, `changes`/`changed`, `refresh`/`refreshed`, `outcome`, `status`), the JSON `dispatch`, partition binding |
| `src/` (`exact-snapback4`) | the native device under the app's grants and directories: `Module` (an Exact `NativeModule` or `DataSource`), and `host_request`/`host_reply` for a Rust source's `Answer::Later` |
| `web/` (`exact-snapback4-web`) | the same client as wasm over the device's memory store; `web/build.mjs` builds it with its wasm-bindgen glue |
| `ts/` | the TypeScript driver an app's `app.ts` imports: `Snapback.open`, `read`/`readAll`, `write`, `sync`, `poll`, `refreshSession`, `outcome`, `status` |

## From TypeScript

### Mount it and build the wasm

Mount the driver in the app's `app.json` (the path is relative to the app,
which the native bake requires: it refuses an absolute one; an app made by
`exact new` outside this checkout names this checkout's `snapback4/ts`
relative to itself), and build the wasm and its glue, which every
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
0.4.16: install that version of `snapback4` for the server.

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
computes from its own clock disagrees with the server in every test on
the fixed epoch. For a drive or test against `snapback4 dev`, say `--epoch now` (a test
file's `epoch now`): the date is then the machine's clock, read once at
launch, and agrees with the server's (the agent's `clock` still moves it).

### A complete app

A guestbook: two personas, a list that syncs and works offline, a post whose
fate (and new row's id) the app reports, a failure banner. `snapback/schema.q`:

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

`app.contract` (a source's result type is keyed by its source name:
`Result<'notes'>`, `Result<'post'>`):

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
  latest: string
  sending: number
  notes: list<Note>
shape Ack
  ok: bool
  message: string
  id: string

component Guestbook
  state persona = "alice"
  state draft = ""
  state notice = ""
  resource time = exactTime() as shape Clock
  resource board = notes(persona, time.epochAtZero + performanceNow()) as shape Board
  mutation posted as shape Ack queue refreshes board then afterPost
  derive banner = match failure(board) {
    case some(f) => f.code == "offline" ? "You're offline" : `Couldn't load notes (${f.code})`,
    case none => ""
  }
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
        when banner != ""
          text banner testId="banner"
        text (board.online ? "Synced" : board.message) testId="status"
        text `${board.sending} sending` testId="sending"
        row gap=8
          input value=draft input=editDraft testId="draft" placeholder="Say something" aria-label="Note" flex=1
          button appearance="auto" press=postNote disabled=(trim(draft) == "") testId="post"
            text "Post"
        text notice testId="notice"
        text board.latest testId="latest"
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

// One partition open at a time (natively the module holds one): switching
// persona waits for the other persona's work in flight, then closes its
// partition (what it keeps stays for its next open).
type Held = { persona: string; opening: Promise<Snapback>; busy: Set<Promise<unknown>> };
let current: Held | null = null;
function using<T>(persona: string, storage: Storage, native: NativeModule | null | undefined, work: (db: Snapback) => Promise<T>): Promise<T> {
  if (current?.persona !== persona) {
    const previous = current;
    const opening = (async () => {
      if (previous) {
        await Promise.allSettled([...previous.busy]);
        await (await previous.opening.catch(() => null))?.close();
      }
      return Snapback.open({ app: appId, name: `guestbook-${persona}`, origin, viewer: `dev:${persona}`,
        headers: () => ({ 'x-snapback-persona': persona }), storage, native });
    })();
    const entry: Held = { persona, opening, busy: new Set() };
    current = entry;
    opening.catch(() => { if (current === entry) current = null; });
  }
  const held = current!;
  const run = held.opening.then(work);
  held.busy.add(run);
  run.then(() => held.busy.delete(run), () => held.busy.delete(run));
  return run;
}

type Row = { id: string; author: string; body: string; pending?: boolean };

// Results are keyed by source name: `Result<'notes'>`, `Result<'post'>`.
const sources: Sources = {
  notes: ([persona, now], _store, storage, native): Promise<Result<'notes'>> => using(persona, storage, native, async db => {
    const round = await db.sync();                      // offline is fine once synced
    const read = await db.read<Row[]>('recent', {}, now);
    if (read.denied) throw new Error(`${read.denied.code}: ${read.denied.message}`);
    // Project each row onto the shape: an answer's extra fields are refused.
    const notes = (read.data ?? []).map(row => ({ id: row.id, author: row.author, body: row.body, pending: row.pending === true }));
    return { online: round.ok, message: round.ok ? '' : 'Offline: showing what this device keeps',
      latest: notes.length ? `${notes[0].author}: ${notes[0].body}` : '',
      sending: notes.filter(note => note.pending).length, notes };
  }).catch(error => {
    // At build (bake) time there is no storage; the app asks again when it runs.
    if ((error as { code?: string }).code === 'bake') return { online: false, message: 'Connecting…', latest: '', sending: 0, notes: [] };
    throw error;                                        // the view shows it through failure(board)
  }),
  post: ([persona, body, now], _store, storage, native): Promise<Result<'post'>> => using(persona, storage, native, async db => {
    const written = await db.write('post', { body }, now);
    if (written.state === 'failed') return { ok: false, message: `Refused: ${written.why.code}`, id: '' };
    await db.sync();
    // A round that ends ok delivered the outbox; the write's own fate is here,
    // with the mutation's result (here the new row's id) once it was sent.
    const fate = await db.outcome<{ id: string }>(written.id);
    if (fate.state === 'failed') return { ok: false, message: `Refused: ${fate.why?.code}`, id: '' };
    if (fate.state === 'sent') return { ok: true, message: 'Posted.', id: fate.result?.id ?? '' };
    return { ok: true, message: 'Saved on this device; it sends when online.', id: '' };
  }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

`app.test.contract` (run `bunx snapback4 dev` first; the tests talk to it,
and `epoch now` dates them as the server does). A `select` is driven with
`type`; the offline test proves delivery after `pass fetch`, not only that
the write was kept:

```
epoch now

test "a note posts, and another persona sees it"
  clock data
  expect text "status" == "Synced"
  type "draft" "hello from alice"
  tap "post"
  clock data
  expect text "notice" == "Posted."
  type "persona" "bob"
  clock data
  expect text "latest" == "dev:alice: hello from alice"

test "offline, a note is kept, then delivered on reconnect"
  clock data
  fail fetch "http://127.0.0.1:4400"
  type "draft" "from the tunnel"
  tap "post"
  clock data
  expect text "notice" == "Saved on this device; it sends when online."
  expect text "sending" == "1 sending"
  pass fetch "http://127.0.0.1:4400"
  clock +5000
  clock data
  expect text "sending" == "0 sending"
```

`app.json` mounts the driver (`"typescript": { "sources": { "snapback4":
"<this checkout>/snapback4/ts" } }`), `bun add snapback4@0.4.16` installs the
server, and `node <this checkout>/snapback4/web/build.mjs assets/snapback4.wasm`
builds the device. Built and tested as written against 0.4.16 (2026-10-08):
both tests pass, and pass again on the data a first run leaves.

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
  `complete: false`. Read in the horizon's order, or widen it. Otherwise the reply says whose answer it is (Snapback's
  CLIENT-AND-OPERATIONS.md, "The device reply"): the device's unless it is
  `unknown` (rows past the horizon, a total over them, a page ordered
  against the horizon), or `retained` history, or the schema holds the
  query to the device; an `unknown` read asks the server, unless a write is
  still queued here. Unreached, a usable partial comes back marked
  `offline: true`, and a placeholder (`speculative`, `loading`) comes back as
  `loading` with `offline: true`: show it as unavailable, never as zero.
- **A round that ends `ok` delivered the outbox; it does not say each write
  was accepted.** `outcome(id)` does: `sent` with `result`, `failed` with
  `why` (the refusal's code, such as one the mutation `require`s), or
  `pending` while unsent. `refusals()` lists every refused write with its
  input until `dismiss()`.
- **A write the device refuses fails at once, and that is final.** When the
  device's own rows refuse a write as the server would (a row rule's
  `E_RULE`, a `require` code such as `NOT_LEAD` over synced rows), `write()`
  answers `{state: 'failed', why}` and, as Snapback's own client does, never
  sends it. `outcome(id)` answers `failed` with that `why`, and `refusals()`
  lists it until dismissed, exactly like a refusal the server gave, so an
  app that reports the outcome never reports a refused write as done. If
  the device's rows may be stale (the viewer was just made a lead
  elsewhere), `sync()` and write again: that is a new write. A keyed write
  (`write(name, args, now, key)`) is not refused by a prediction: it queues
  and the server decides. The same key with other input answers
  `E_WRITE_ID_REUSE` to that call only; `outcome(id)` stays the first
  write's.
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

The native module holds one partition at a time: an app that switches
persona closes the one client, once its work in flight is done, before
opening the other's (the guestbook's `using` does), or the open is refused.

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
feeds, ephemeral reads and writes (a write that touches an ephemeral table
is refused at once with `E_CLIENT_UNSUPPORTED`, never queued), search state, the online fallback fenced by
intersecting predictions (here a read the device cannot vouch for asks the
server only when nothing is queued at all), a server read kept live
(here a query the server answers is asked again, not invalidated by the
change poll; LLP 1110), and the 0.2.32 legacy fallback. `round.rs` names its source in Snapback's `local.ts`.
