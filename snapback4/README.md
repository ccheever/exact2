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
| `ts/` | the TypeScript driver an app's `app.ts` imports: `Snapback.open`, `Snapback.switcher` (one persona's partition at a time), `read`/`readAll`, `write`, `sync`, `poll`, `refreshSession`, `outcome`, `status` |

## From TypeScript

### Mount it and build the wasm

Two steps, both from the app's directory (the one holding `app.json`). An
app made by `exact new` has both, with this checkout's paths filled in, in its
`AGENTS.md`.

1. Mount the driver in `app.json`. The path is relative to the app's
   directory, never absolute (the native bake refuses one), and it counts
   the `../` from there: for an app at `~/work/lane/app` beside a checkout at
   `~/work/exact2` it is `../../exact2/snapback4/ts`. Run in the app's
   directory, `bun -e "console.log(require('path').relative('.',
   '<exact2>/snapback4/ts'))"` prints it.

   ```json
   "typescript": { "sources": { "snapback4": "../../exact2/snapback4/ts" } }
   ```

   A path that is not there fails the web build with the directory it was
   resolved from, the full path, and this checkout's path from the app.

2. Build the device the page loads into the app's `assets/` (the output path
   is from the current directory), and its glue, which every host's bake
   type-checks:

   ```sh
   node <exact2>/snapback4/web/build.mjs assets/snapback4.wasm   # no argument writes only the glue
   ```

   The first build compiles the client for a few minutes; later ones take
   seconds. The web host serves `assets/snapback4.wasm` at
   `/assets/snapback4.wasm`, the driver's default (`Snapback.open`'s `wasm`
   names another). A web build of an app that mounts the driver without it
   says so, with this command. It is pinned to Snapback4
0.4.16: install that version of `snapback4` for the server.

### Time

A data module has no clock (`Date.now()` in one fails the build, with this
repair). Declare the host's clock as a resource and pass milliseconds since
the epoch into every source that reads or writes:

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
  resource board = notes(persona, time.epochAtZero + performanceNow()) as shape Board else empty(message="Connecting…")
  mutation posted as shape Ack queue refreshes board then afterPost
  derive banner = match failure(board) {
    case some(f) => f.code == "offline" ? "You're offline" : `Couldn't load notes (${f.message})`,
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
        if banner != ""
          text banner testId="banner"
        text (board.online ? "Synced" : board.message) testId="status"
        text `${board.sending} sending` testId="sending"
        row gap=8
          input value=draft input=editDraft testId="draft" placeholder="Say something" aria-label="Note" flex=1
          button appearance="auto" press=postNote disabled=(trim(draft) == "") testId="post"
            text "Post"
        text notice testId="notice"
        text board.latest testId="latest"
        for note, i in board.notes key=note.id
          text `${note.author}: ${note.body}${note.pending ? " (sending)" : ""}` testId=`note-${i}`
```

`app.ts`:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused } from './snapback4/snapback.ts';

export const appId = 'com.example.guestbook';
const origin = 'http://127.0.0.1:4400';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

// One partition open at a time (natively the module holds one): switching
// persona waits for the other persona's work in flight, then closes its
// partition (what it keeps stays for its next open), then opens this one's.
const personas = Snapback.switcher((persona: string) => ({ app: appId, name: `guestbook-${persona}`, origin,
  viewer: `dev:${persona}`, headers: () => ({ 'x-snapback-persona': persona }) }));

type Row = { id: string; author: string; body: string; pending?: boolean };

// Results are keyed by source name: `Result<'notes'>`, `Result<'post'>`. Put
// the type on the callback `use` runs, where the object is returned: there an
// undeclared field fails the build (TS2353 names it); on the outer arrow, a
// callback's literal is never checked for extra fields.
const sources: Sources = {
  notes: ([persona, now], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'notes'>> => {
    const round = await db.sync();                      // offline is fine once synced
    const read = await db.read<Row[]>('recent', {}, now);
    if (read.denied) throw refused(read.denied);        // failure(board): `offline`, or `error` with "CODE: message"
    // Project each row onto the shape: an answer's extra fields are refused
    // (a row spread, `{ ...row }`, keeps every field the type check cannot see).
    const notes = (read.data ?? []).map(row => ({ id: row.id, author: row.author, body: row.body, pending: row.pending === true }));
    return { online: round.ok, message: round.ok ? '' : 'Offline: showing what this device keeps',
      latest: notes.length ? `${notes[0].author}: ${notes[0].body}` : '',
      sending: notes.filter(note => note.pending).length, notes };
  }),
  // At build (bake) there is no storage or native module: `use` rejects with
  // code 'bake', and a resource whose source lets it through shows its
  // `else empty(…)` until the app runs and asks again. No catch is needed.
  post: ([persona, body, now], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'post'>> => {
    // Write, sync, and the write's fate in one call: `sent` with the mutation's
    // result (here the new row's id), `failed` with the refusal's code, or
    // `pending` when the round did not reach the server (kept; sent later).
    const fate = await db.send<{ id: string }>('post', { body }, now);
    if (fate.state === 'failed') return { ok: false, message: `Refused: ${fate.why?.code}`, id: '' };
    if (fate.state === 'sent') return { ok: true, message: 'Posted.', id: fate.result?.id ?? '' };
    return { ok: true, message: 'Saved on this device; it sends when online.', id: '' };
  }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

`app.test.contract` (run `bunx snapback4 dev` first; the tests talk to it,
and `epoch now` dates them as the server does; `test web` asks the origin
before it starts, and refuses to run with nothing listening there). Each test
opens the app fresh, but the server keeps what earlier tests wrote, so each
test creates what it asserts on. A `--memory` server starts empty: rows the
app cannot write itself (an `owners` or `staff` table a rule reads) go in
before the run, as the system, with `POST <origin>/seed` and a body such as
`[{"table":"owners","row":{"id":"o1","person":"dev:alice"}}]` (`snapback4
guide wire`, "Development doors"). A `select` is driven with
`type`; the offline test proves delivery after `pass fetch`, not only that
the write was kept:

```
epoch now

test "a note posts, and another persona sees it"
  clock data
  expect text "status" == "Synced"
  expect tree missing "banner"
  type "draft" "hello from alice"
  tap "post"
  clock data
  expect text "notice" == "Posted."
  type "persona" "bob"
  clock data
  expect text "latest" == "dev:alice: hello from alice"
  expect text "note-0" == "dev:alice: hello from alice"

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
builds the device. Built and tested as written against 0.4.16 (2026-10-11):
both tests pass, and pass again on the data a first run leaves.

### Ports: the Snapback origin and the page's own

Two ports, set apart. **The Snapback origin** is a constant in `app.ts`
(`const origin = 'http://127.0.0.1:4400'`) and its `net.fetch` grant; a data
module reads no environment, so the server must come to it:
`snapback4 dev --port 4400` (4400 is also `dev`'s default), and the test
file's `fail fetch`/`pass fetch` prefixes name the same origin. To move it,
change the constant, the `--port` and the prefixes together. `test web` reads
this origin from the build's `net.fetch` grants (`app.ts`'s `grants`), asks
whether anything listens there, and refuses the run when nothing does.
**The Exact page's port** is separate: `EXACT_PORT=<n>` (or `--port <n>`)
serves `web`, `test web` and `agent web` on exactly that port, else a free
one. It never changes the origin the app fetches, and `--port` on `test web`
is the page's, not the server's. On a machine that assigns you ports, take
one for `snapback4 dev --port` and the origin, and another for `EXACT_PORT`.

### Open, sync, read, write

```ts
import { Snapback } from './snapback4/snapback.ts';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

const db = await Snapback.open({ app: appId, name: `inbox-${persona}`, origin, viewer,
  headers: () => ({ authorization: `Bearer ${session.token}` }),  // or { 'x-snapback-persona': persona } in development
  storage, native });                                  // Exact's own `storage` and `native`, as the source receives them
await db.sync();                                       // open on first sync, page, send the outbox
const inbox = await db.read('inbox', {}, now);         // the device answers; pending rows say so
const written = await db.write('send', { body }, now); // kept and predicted now, sent by the next sync
await db.sync();
const fate = await db.outcome(written.id);             // 'sent' with the server's result, or 'failed' and why
const same = await db.send('send', { body }, now);     // the three lines above in one call (and the round)
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
- **Switching persona or account: `Snapback.switcher`.** ``const personas =
  Snapback.switcher(persona => ({ app, name: `inbox-${persona}`, origin,
  viewer, headers }))`` (`Options` less `storage` and `native`), then in each
  source `personas.use(persona, storage, native, async db => …)`, as the
  guestbook does. A new key
  waits for the old one's work in flight, closes its partition (what it keeps
  stays for its next open), then opens its own, on every host. An open that
  fails (`code: 'bake'`, `E_OFFLINE`) rejects that call and is tried again by
  the next; a client whose device gave up is reopened. `personas.close()` lets
  the current one go (a sign-out); `personas.key` is the key in use. Work that
  never settles (a fetch with no timeout) holds the switch.
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
  `readAll` keeps the same facts for every page it read: its rows carry
  `complete` (false when any page was not the whole answer) and `offline`.
- **`write` only admits; a round sends.** `outcome(id)` asked before a
  round is `pending`. A `sync()` asked for after the write, that ends `ok`,
  has sent it (whatever other sources sync meanwhile), so the outcome is
  then `sent` with `result` or `failed` with `why` (the refusal's code, such
  as one the mutation `require`s, or `E_RULE`); it is `pending` after a
  round only when that round did not end `ok` (offline, `retry`, a refused
  session). A mutation source calls `send(name, args, now)`, which is
  write, sync and outcome in one, and answers that round as `round`.
  `refusals()` lists every refused write with its input until `dismiss()`.
- **Failures in the view.** A resource's source that throws fails it, and
  `failure(x)` says why. Its `code` is Exact's closed list (`offline`,
  `error`, …, the same on every host), never a Snapback code; throw a
  refusal with `refused(why)` (`if (read.denied) throw refused(read.denied)`)
  and it is `offline` when the server was not reached (`E_OFFLINE`), else
  `error`, with `failure(x).message` the refusal's `CODE: message` on every
  host: branch on a query's `require … else NOT_MANAGER` with
  `startsWith(f.message, "NOT_MANAGER:")`, or return the code in the
  shape's own field. The driver's own refusals are thrown the same way:
  `readAll` throws a page's (where `read` answers `denied`), and a first
  read that cannot reach the server throws `E_OFFLINE`. A mutation's
  refusal is not a failure: it is the outcome's `why`. A mutation source's
  answer outside its shape (a field the shape does not declare) is refused
  too: the slot keeps what it had and its `then` never runs. A failing
  `test web` expect and an `agent` drive name it (``mutation posted's reply
  was refused: `post` answered outside its shape: field `seq` is not in
  the shape``), and the type on `use`'s callback stops it at build.
- **Typing a helper.** `db` in `personas.use(…, async db => …)` is a
  `Snapback`: annotate a helper's parameter as that (`import { Snapback,
  type Outcome } from './snapback4/snapback.ts'`), never `any` or a
  narrowed `{ read(…) }`, which drop the generics (`db.read<Row[]>` then
  fails the bake with TS2347 or TS2558).
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
- **Ids the server mints are unique, not ordered.** A row's `id` does not
  sort in creation order (a later row's can sort first), so never take the
  largest id as the newest: give the table a `time` the mutation sets (`at:
  now`) and read in its index's order (`last 50 by byTime`). A test file
  cannot know a minted id either, so a `testId` built from one is out of
  its reach. Reach a row by position, from `each`'s index (`each note, i in
  board.notes key=note.id` with ``testId=`note-${i}` ``, as the guestbook
  does: `expect text "note-0" == …`), by what it shows (`tap "<its exact
  text or label>"`), or through a derive of the row the test made (the
  mutation's reply id, or the newest row, as the guestbook's `latest`). A
  button on a row passes the row's id to its action: `button
  press=remove(note.id)` with `action remove(id: string)`, whose `send`
  carries it to the source.
- **Live updates.** A data answer has a bounded life and tests settle on
  answers, so do not hold a long `poll(20)` inside a resource. Drive
  freshness from Contract: a `task` that calls an action every few seconds,
  whose resources `sync()` (or `poll(0)`) and read again, and `refreshes` on
  the mutations that change them. `refreshes` asks again only the resources
  it names: name every one the write changes (`refreshes board, feed`), or a
  second view of the same rows stays as it was until its next refresh.

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
opening the other's (`Snapback.switcher` does), or the open is refused.

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
