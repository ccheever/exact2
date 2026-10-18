# Snapback4 for Exact apps

One Snapback4 client for an Exact app's Rust and TypeScript, on every host:
the device (its partition in SQLite, its outbox, local queries and
predictions) and the client protocol over it (opening, sync rounds, store
identity and generation adoption, at-most-once sends with receipts, the
change poll), written once in Rust and performing no I/O. Each HTTP exchange
is handed to whoever drives it. Pinned to Snapback4 **0.4.29**
(engine sources in `snapback4/Cargo.toml`); the npm CLI in the root
`package.json` matches.

| | |
|---|---|
| `client/` (`exact-snapback4-client`) | the protocol: `Client` (`read`, `write`, `sync` → `Step::Fetch`/`deliver` → `Step::Done`, `changes`/`changed`, `refresh`/`refreshed`, `outcome`, `status`), the JSON `dispatch`, partition binding |
| `src/` (`exact-snapback4`) | the native device under the app's grants and directories: `Module` (an Exact `NativeModule` or `DataSource`), and `host_request`/`host_reply` for a Rust source's `Answer::Later` |
| `web/` (`exact-snapback4-web`) | the same client as wasm over the device's memory store; `web/build.mjs` builds it with its wasm-bindgen glue |
| `ts/` | the TypeScript driver an app's `app.ts` imports: `Snapback.open`, `Snapback.switcher` (one viewer's partition at a time), `read`/`readAll`, `write`, `sync`, `poll`, `signup`/`login`/`logout`, stored password sessions, `refreshSession`, `outcome`, `status`, `upload`/`download` |

> **Exact + Snapback checklist.** Each line is a pair that must agree; a
> mismatch fails late, as data that never loads.
>
> - **Directories.** The backend (`snapback/schema.q`, `snapback4@0.4.29`)
>   is the directory above the app (the one holding `app.json`). Run
>   `snapback4` there and `bun exact.mjs …` (`snapback` among them) in the
>   app; `assets/snapback4.wasm` is built into the app's `assets/`.
> - **The app id.** `app.ts`'s `export const appId` is `app.json`'s
>   `"app": { "id" }`.
> - **The mounts.** `app.json`'s `typescript.sources`: `snapback4` (this
>   checkout's `snapback4/ts`, relative) and `backend`
>   (`../snapback/generated`, after the first `snapback4 check`).
> - **The device version.** `bun exact.mjs snapback --rebuild` repairs an
>   unstamped or incompatible artifact. The device crate emits `snapback4.engine`
>   with its real `{format:1, version, capabilities}`. Wiring and web drives read
>   that section and compare the server's compatibility floor and
>   capabilities, as `check` does. `.wasm.json` is derived diagnostic output.
>   Compatible version skew is allowed; empty, malformed or duplicate stamps refuse.
> - **One origin setting** (["Ports"](#ports-the-snapback-origin-and-the-pages-own)):
>   `app.ts`'s `origin`, its `net.fetch` grant (write it as `${origin}`),
>   `bun exact.mjs backend`, and the generated test file's `fail fetch`/`pass fetch`
>   prefixes. On `test web`, write `fail fetch "backend"` / `pass fetch "backend"`: those
>   resolve from the built grant at runtime, so `app.ts` has the only origin setting.
>   With several fetch origins, use explicit URL prefixes. `bun exact.mjs snapback --port N` selects a local override. The page's port (`EXACT_PORT`,
>   `--port` on `bun exact.mjs`) is another, never this one.
> - **Grants.** `sqlite.open app:/data` for the partition; `fs.read app:/tmp`
>   to upload a picked file and `fs.write app:/cache` to download one.
> - **Personas.** A `Snapback.switcher` key sent as `x-snapback-persona` is
>   a persona the schema's `use personas` declares (another is `E_AUTH`),
>   and each key has its own partition `name`.
> - **Tests.** `epoch now` in the test file (the server keeps real time);
>   `snapback4 dev` listening before `test web`; `clock data` before `fail
>   fetch`; `fresh` for an empty dev server before each test through
>   0.4.29's `POST /dev/reset`
>   (["Tests share the server"](#tests-share-the-server)).

## From TypeScript

### Set it up: one command

A new app: from the backend's directory (`snapback/schema.q`, and
`snapback4@0.4.29` installed there: `bun add snapback4@0.4.29` or `npm i
snapback4@0.4.29`), `bun /path/to/exact2/scripts/exact.mjs new app --snapback`
(`--port <n>` for an origin other than 4400). With no backend schema yet, it writes the guestbook below as
the app (`app.contract`, `app.ts` at that origin with its grants,
`app.test.contract`) and `snapback/schema.q` when there is none, then wires it. With an existing schema it keeps the greeting app and mounts
the backend's operations: write the view and data sources for those operations.
For a server-only app, write its schema first, then use `new app --snapback --online`
(or `snapback --online` on an app with no driver mount). This mounts `snapback4/online`,
which `snapback` and `update` preserve. It exports `Snapback` with typed `read(api.query, args)` and `send(api.mutation, args)`:
`new Snapback({origin, headers})`. It loads no wasm and opens no SQLite partition.
Reads return the server's `data` envelope; mutations return `sent` with `result` or
`failed` with `why`. An interrupted write throws: its server fate may be unknown,
so read before retrying. There is no offline prediction, queue or automatic retry.
Snapback `check` and Exact's web preflight gate actual device consumers; the online
mount and generated backend operations need no device artifact.

An existing app (the one holding `app.json`, the backend in the directory
above it): `bun exact.mjs snapback`. Either one:

1. **Generates the operations** (`snapback4 check` above) when
   `snapback/generated/` is not there yet and the server is installed; else
   it says what to install.
2. **Mounts the driver** in `app.json`, as a path relative to the app's
   directory, never absolute (the native bake refuses one): for an app at
   `~/work/lane/app` beside a checkout at `~/work/exact2`,
   `"typescript": { "sources": { "snapback4": "../../exact2/snapback4/ts" } }`.
   A path that is not there fails the web build with the directory it was
   resolved from, the full path, and this checkout's path from the app.
   `bun exact.mjs update` rewrites it after the checkout moves.
3. **Mounts the backend's generated operations** (`"backend":
   "../snapback/generated"`), when they are there, so `app.ts` can import
   `api` from `./backend/api.ts` (["Typed operations"](#typed-operations)).
   Its `snapback4/contract` types are read from the `snapback4` package
   installed above that directory. Run `bun exact.mjs snapback` again after
   the backend first writes `snapback/generated/`.
4. **Builds the device** the page loads into `assets/snapback4.wasm` (`node
   <exact2>/snapback4/web/build.mjs assets/snapback4.wasm` by hand) and its
   glue, which every host's bake type-checks. A default `snapback` run asks Cargo to rebuild when inputs changed, including
   when the existing stamp is missing or stale. It refuses an edited driver or backend
   mount with a before/after diff before changing files. Port-only changes reuse a compatible stamped device. The first build compiles the
   client for a few minutes; later ones take seconds. The web host serves it
   at `/assets/snapback4.wasm`, the driver's default (`Snapback.open`'s
   `wasm` names another). A web build of an app that mounts the driver
   without it says so.

It names the `appId` line `app.ts` needs when the two disagree: `app.json`'s
`"app": { "id" }`, which `exact new` set from the directory's name. Last it
says the server to start, `bunx snapback4 dev --port <n>` above. The driver is
pinned to Snapback4 0.4.29: install that version for the server. A new app
made this way and tested as written against 0.4.29 (2026-10-16): both of the
guestbook's tests pass.

### Upgrading a wave-10-era app

Delete app-local stamp scripts (such as `stamp-device.mjs`) and their build hooks.
Delete `x-snapback-device` header shims; keep persona or bearer credentials.
The linked device supplies its real identity on the device-to-server leg, including
its first schema request. The TypeScript driver discards app-supplied identity
headers and forwards the device's; an app-to-Exact-host request is a separate leg.
Preflight names a leftover shim in `app.ts` as obsolete.

After upgrading the backend package, run in the app:

```sh
bun exact.mjs snapback --rebuild --artifact-only
# In the backend directory above the app:
bunx snapback4 check
```

`--artifact-only` rebuilds `assets/snapback4.wasm` and its derived `.wasm.json`,
without editing sources, mounts, tests or notes. It works with custom mounts;
`--port` and `--online` cannot accompany it. Ordinary `snapback --rebuild` also
refreshes wiring and refuses divergent mounts. No restamping step is needed.
The `app.json` `typescript.sources` entries are virtual module mounts: imports
such as `./backend/api.ts` and `./snapback4/snapback.ts` resolve to their mapped
directories during the bake. They need no matching directories inside the app.
An online app needs no wasm rebuild: upgrade the package and run `check`.

Production `serve` answers unauthenticated `GET /` with `401 E_AUTH`.
Ask it with the same bearer credentials as data requests; it is not a public
readiness probe. Preflight can validate the embedded device identity offline but
cannot compare a production server floor without authentication. The device's
first authenticated exchange and backend `check` enforce compatibility too.

### Personas and password accounts

A persona app declares `use personas alice, bob` and runs against `dev`:

```ts
const db = await Snapback.open({ app: appId, name: Snapback.partitionName('dev:alice'),
  origin, viewer: 'dev:alice', headers: () => ({ 'x-snapback-persona': 'alice' }), storage, native });
```

A password app declares `use identity password`. It runs against `dev` or a
release made with `snapback4 deploy <dir>` and served by `snapback4 serve <dir>`.
`serve` accepts bearer sessions and refuses personas. In a source, pass its
`store`, `storage`, `native`, and Contract's `now` (milliseconds, as below):

```ts
// grants: net.fetch ${origin}\nsqlite.open app:/data\nsecret.keep snapback-session
const db = await Snapback.open({ app: appId, name: 'account', origin, storage, native,
  auth: { store, key: 'snapback-session', now } });
try {
  // Opening restores the stored session. Signed out, no partition opens yet.
  if (!db.session) {
    const signed = await db.signup(email, password);   // or db.login(email, password)
    if (!signed.ok) throw refused(signed.why);
  }
  const principal = db.session!.principal;             // publish this to the view, never the token
  await db.sync();
  const notes = await db.read(api.recent, {}, now);
  // await db.logout(): revoke online and forget locally even when offline.
} finally { await db.close(); }                        // releases the handle; keeps session and rows
```

Sessions live in Exact's app-scoped secret store (`secret.keep <key>`): the
Keychain on Apple, the user's private secret files on Linux, and the app's
web storage. Use a separate key for each origin. `open` drops malformed or
expired stored sessions and refreshes within 60 seconds of expiry using the
supplied Contract time. `refreshSession(now)` also rotates explicitly and
stores the replacement before returning. `E_AUTH` on refresh forgets the
session: show sign-in again. A refusal for an older token never clears a
replacement already stored by refresh. Offline refresh keeps it. `logout` always forgets
locally and retires the previous partition, including shared native handles;
`ok: true` confirms server revocation, `offline: true` says it could
not be confirmed. Await logout before signing into another account. Data and asset requests
that finish after an account change reject with `E_STALE`; open a new handle. No DOM
`Storage`, npm auth import, `Date.now()` or timers are needed in `app.ts`.

Logout also works through a handle opened before another source signs in.
It retires that origin/principal's shared partitions, including siblings using
manual credential headers. Signing back into the same account requires new
sibling handles; retired handles cannot resume cached reads.

`name` is a base for password apps: the driver adds the safely encoded
principal, so two accounts never reuse a partition. For manually managed
credentials, `Snapback.partitionName(principal)` is a lossless ASCII mapping
of `user:<ULID>` (and other principals) to a filename. A switcher uses it
when `name` is omitted and maps an unsafe supplied name too. For example,
this mixed switcher needs no cast on its conditional headers:

```ts
import type { Session } from './snapback4/snapback.ts';
const sessions = new Map<string, Session>();           // populated from db.session after sign-in
const viewers = Snapback.switcher((key: string) => ({ app: appId, origin,
  name: sessions.has(key) ? `account-${Snapback.partitionName(key)}` : Snapback.partitionName(`dev:${key}`),
  viewer: sessions.get(key)?.principal ?? `dev:${key}`,
  headers: () => {
    const session = sessions.get(key);
    return session ? { authorization: `Bearer ${session.token}` } : { 'x-snapback-persona': key };
  } }));
// Password key: its principal; persona key: its declared name (dev only).
// The password name matches the `name: 'account'` auth opening above.
// In a source: viewers.use(key, storage, native, db => db.read(api.recent, {}, now)).
```

Bare `use identity` permits guests: call `await db.guest()` before any read,
then keep its session in the same `auth` store as a password session.
`await db.authConfig()` reads `GET /auth/config` and reports `guests`, `password`
and `development`. `use identity password` removes guests. A complete tested guest
and mixed persona/password recipe is [here](../docs/recipes/snapback-auth.md).

### Time

A data module has no clock (`Date.now()` in one fails the build, with this
repair). Declare the host's clock as a resource and pass milliseconds since
the epoch into every source that reads or writes:

```
shape Clock
  epochAtZero: number

  resource time = exactTime() as shape Clock
  state sampledAt = 0
  resource inbox = inbox(time.epochAtZero + sampledAt) as Message[]
  action reloadInbox
    sampledAt = performanceNow()
    refresh inbox
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
launch. `clock +60000` then advances only the UI's clock; it does not advance
Snapback's `now`, sessions, expiry jobs or server schedules. `clock data` waits for
real network/storage work and leaves both clocks unchanged. Use the Snapback CLI's
own clock controls for a dev-server scheduling proof. `epoch now` aligns the start,
not the two clocks after a virtual jump. Production wall-clock time keeps moving
while a test's UI clock stands still.

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
`Result<'notes'>`, `Result<'post'>`, never by the slot it fills, `posted`;
a name that is not a source fails the build listing those that are):

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
  notes: Note[]
shape Ack
  ok: bool
  message: string
  id: string

component Guestbook
  state persona = "alice"
  state draft = ""
  state notice = ""
  state sampledAt = 0
  resource time = exactTime() as shape Clock
  resource board = notes(persona, time.epochAtZero + sampledAt) as shape Board else empty(message="Connecting…")
  mutation posted as shape Ack queue refreshes board then afterPost
  derive postProblem = match failure(posted) { case some(f) => "Couldn't post. Try again.", case none => "" }
  derive banner = match failure(board) {
    case some(f) => (f.code == "offline" || f.code == "E_OFFLINE") ? "You're offline" : f.reason != "" ? `Couldn't load notes (${f.reason})` : `Couldn't load notes (${f.message})`,
    case none => ""
  }
  task live mount
    every(5000, refreshBoard)
  action refreshBoard
    sampledAt = performanceNow()
    refresh board
  action choosePersona(value: string)
    persona = value
  action editDraft(value: string)
    draft = value
  action postNote
    send posted = post(persona, draft.trim(), time.epochAtZero + performanceNow())
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
          button appearance="auto" press=postNote disabled=(draft.trim() == "") testId="post"
            text "Post"
        text postProblem testId="post-problem"
        text notice testId="notice"
        text board.latest testId="latest"
        for note, i in board.notes key=note.id
          text `${note.author}: ${note.body}${note.pending ? " (sending)" : ""}` testId=`note-${i}`
```

`app.ts`:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused, pick, type Sent } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';                 // the backend's generated operations, mounted

export const appId = 'com.example.app';                 // app.json's "app": { "id" }, as `bun exact.mjs snapback` checks it
const origin = 'http://127.0.0.1:4400';
export const grants = `net.fetch ${origin}\nsqlite.open app:/data`;

// One partition open at a time (natively the module holds one): switching
// persona waits for the other persona's work in flight, then closes its
// partition (what it keeps stays for its next open), then opens this one's.
const personas = Snapback.switcher((persona: string) => ({ app: appId, name: `guestbook-${persona}`, origin,
  viewer: `dev:${persona}`, headers: () => ({ 'x-snapback-persona': persona }) }));

// Results are keyed by source name: `Result<'notes'>`, `Result<'post'>`. Put
// the type on the callback `use` runs, where the object is returned: there an
// undeclared field fails the build (TS2353 names it); on the outer arrow, a
// callback's literal is never checked for extra fields.
function ack(fate: Sent<{id: string} | null>): Result<'post'> {
  if (fate.state === 'failed') throw refused(fate.why);
  if (fate.state === 'sent') return {ok: true, message: 'Posted.', id: fate.result?.id ?? ''};
  return {ok: true, message: 'Saved on this device; it sends when online.', id: ''};
}
const note = pick<Result<'notes'>['notes'][number]>();

const sources: Sources = {
  notes: ([persona, now], _store, storage, native) => personas.use(persona, storage, native, async (db): Promise<Result<'notes'>> => {
    const round = await db.sync();                      // offline is fine once synced
    const read = await db.read(api.recent, {}, now);   // rows typed by the schema, `pending` included
    if (read.denied) throw refused(read.denied);        // failure(board).code preserves the refusal CODE
    // Project each row onto the shape: an answer's extra fields are refused
    // (a row spread, `{ ...row }`, keeps every field the type check cannot see).
    const notes = (read.data ?? []).map(row => note({...row, pending: row.pending === true}, 'id', 'author', 'body', 'pending'));
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
    // The handle checks `{ body }` against the mutation's arguments.
    return ack(await db.send(api.post, { body }, now));
  }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

`app.test.contract` (run `bunx snapback4 dev` first; the tests talk to it,
and `epoch now` dates them as the server does; `test web` asks the origin
before it starts, and refuses to run with nothing listening there). Each test
opens the app fresh, but the server keeps what earlier tests wrote, so each
test creates what it asserts on (["Tests share the server"](#tests-share-the-server)). A `--memory` server starts empty: rows the
app cannot write itself (an `owners` or `staff` table a rule reads) go in
before the run, as the system, with `POST <origin>/seed` and a body such as
`[{"table":"owners","row":{"id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","person":"dev:alice"}}]` (`snapback4
guide wire`, "Development doors"), or before each test with a `fresh
"seed.json"` line through `POST /dev/reset`. Resolve the seed path from the test file's directory; each entry is
`{table, row}` and an explicit row `id` must have 26 characters. A `select` is driven with
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
  fail fetch "backend"
  type "draft" "from the tunnel"
  tap "post"
  clock data
  expect text "notice" == "Saved on this device; it sends when online."
  expect text "sending" == "1 sending"
  pass fetch "backend"
  clock +5000
  clock data
  expect text "sending" == "0 sending"
```

The app is `bun <this checkout>/scripts/exact.mjs new app --snapback`, run in
the backend's directory, which writes these four files and wires them. Built that way and tested as written against 0.4.29 (2026-10-16): both tests pass, and pass again on the data a first run leaves.

### A complete online app

For a schema that syncs nothing, install `snapback4@0.4.29` in the backend,
write this `snapback/schema.q`, then run
`bun <exact2>/scripts/exact.mjs new app --snapback --online --port 30100`.
It mounts the online adapter and generated operations, and writes a query/mutation
explorer for an untouched greeting app. Replace that explorer
with these files; it opens no SQLite partition and needs no wasm:

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
  online only

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200):
  require body != 'refused' else FULL
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
```

`app.contract`:

```contract
shape Note
  id: string
  author: string
  body: string
shape Board
  complete: bool
  latest: string
  notes: Note[]
shape Post
  id: string

component App
  state persona = "alice"
  state draft = ""
  resource board = notes(persona) as shape Board else empty()
  mutation posted as shape Post refreshes board
  derive problem = match failure(posted) { case some(f) => f.code == "FULL" ? "The guestbook is full." : "Couldn't post. Try again.", case none => "" }
  derive problemCode = match failure(posted) { case some(f) => f.code, case none => "" }
  derive problemMessage = match failure(posted) { case some(f) => f.message, case none => "" }
  action choosePersona(value: string)
    persona = value
  action edit(value: string)
    draft = value
  action post
    send posted = post(persona, draft.trim())
  view
    column gap=8 padding=16
      select value=persona change=choosePersona testId="persona" aria-label="Persona"
        option value="alice"
          text "Alice"
        option value="bob"
          text "Bob"
      input value=draft input=edit submit=post testId="draft" aria-label="Note"
      button press=post disabled=(draft.trim() == "" or pending(posted)) testId="post"
        text "Post"
      text problem testId="problem"
      text (match posted { case some(p) => "Posted.", case none => "" }) testId="notice"
      if failed(board)
        text (match failure(board) { case some(f) => f.code, case none => "" }) testId="unavailable"
      if not pending(board) and not failed(board) and not board.complete
        text "Partial answer; more rows may exist." testId="partial"
      text board.latest testId="latest"
      for note, i in board.notes key=note.id
        text `${note.author}: ${note.body}` testId=`note-${i}`
```

`app.ts` (project server rows onto the Contract shape):

```ts
import type { Answer, Result, Sources, ArgsOf } from './app.contract.d.ts';
import { Snapback, refused, pick } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';
export const appId = 'com.example.app'; // app.json's app.id
const origin = 'http://127.0.0.1:30100';
export const grants = `net.fetch ${origin}`;
const client = (persona: string) => new Snapback({origin,
  headers: () => ({ 'x-snapback-persona': persona })});
const sources: Sources = {
  notes: async ([persona]): Promise<Result<'notes'>> => {
    const page = await client(persona).read(api.recent, {});
    if (page.denied) throw refused(page.denied);
    const note = pick<Result<'notes'>['notes'][number]>();
    const notes = (page.data ?? []).map(row => note(row, 'id', 'author', 'body'));
    return {complete: page.complete === true, notes,
      latest: notes.length ? `${notes[0].author}: ${notes[0].body}` : ''};
  },
  post: async ([persona, body]): Promise<Result<'post'>> => {
    const args: ArgsOf<typeof api.post> = {body};
    const sent = await client(persona).send(api.post, args);
    if (sent.state === 'failed') throw refused(sent.why);
    return {id: sent.result.id};
  },
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

`app.test.contract`:

```contract
fresh
epoch now

test "the online write is visible to another persona"
  clock data
  type "draft" "online hello"
  tap "post"
  clock data
  expect text "notice" == "Posted."
  type "persona" "bob"
  clock data
  expect text "latest" == "dev:alice: online hello"

test "a refused write exposes its domain code"
  clock data
  type "draft" "refused"
  tap "post"
  clock data
  expect text "problem" == "The guestbook is full."
  expect state problemCode == "FULL"
  expect text "notice" == ""
```

Generate the seam once with `bun exact.mjs contract types app.contract -o
app.contract.d.ts`; subsequent `contract build` refreshes that existing file.
Start `bun exact.mjs backend` and run `bun exact.mjs test web` in another terminal.
For production, send a bearer session instead of a development persona.
Reads preserve the server's `complete`, `cut`, branded `next` cursor and `denied`;
pass a cursor unchanged to the generated query's cursor argument. A cut or partial
answer is not an empty complete list. An interrupted write throws and may already
have committed: read state before retrying it.

The online adapter also exposes `upload` and `download` as in the media section
below. Pass a source's `storage` to `new Snapback({origin, headers, storage})` for
downloads and grant `fs.write app:/cache`; picked uploads need `fs.read app:/tmp`.
These use the server's asset routes and keep no upload intent or offline queue.
Durable upload intents belong to the device, not an app-local second outbox.

### A complete online password app

This variant of the online guestbook works on `dev` and production `serve`.
Use the online schema above with `use identity password` in place of
`use personas alice, bob`; the table and operations stay the same. Create
`app` with `new app --snapback --online --port 30100`, then use these files.
The signed-out view never renders retained private rows. The session token
stays in Exact's `secret.keep` store; only the principal reaches Contract.

`app.contract`:

```contract
shape Clock
  epochAtZero: number
shape Account
  principal: string
shape Note
  id: string
  author: string
  body: string
shape Board
  principal: string
  complete: bool
  notes: Note[]
shape Ack
  ok: bool

component App
  state email = ""
  state password = ""
  state draft = ""
  state sampledAt = 0
  resource time = exactTime() as shape Clock
  resource account = restoreAccount(time.epochAtZero + sampledAt) as shape Account else empty()
  resource board = notes(account.principal, time.epochAtZero + sampledAt) as shape Board else empty()
  mutation signed as shape Ack refreshes account, board
  mutation posted as shape Ack refreshes board
  task live mount
    every(5000, tick)
  action tick
    sampledAt = performanceNow()
    refresh account
    refresh board
  action editEmail(value: string)
    email = value
  action editPassword(value: string)
    password = value
  action editDraft(value: string)
    draft = value
  action signup
    send signed = signup(email, password, time.epochAtZero + performanceNow())
  action login
    send signed = login(email, password, time.epochAtZero + performanceNow())
  action logout
    send signed = logout(time.epochAtZero + performanceNow())
  action post
    send posted = post(draft.trim(), time.epochAtZero + performanceNow())
  view
    column gap=8 padding=16
      text (account.principal == "" ? "Signed out" : "Signed in") testId="session"
      text (match failure(signed) { case some(f) => f.code, case none => "" }) testId="auth-error"
      if account.principal == ""
        input value=email input=editEmail type="email" testId="email" aria-label="Email"
        input value=password input=editPassword type="password" testId="password" aria-label="Password"
        button "Sign up" press=signup disabled=pending(signed) testId="signup"
        button "Log in" press=login disabled=pending(signed) testId="login"
      else
        button "Log out" press=logout disabled=pending(signed) testId="logout"
        input value=draft input=editDraft testId="draft" aria-label="Note"
        button "Post" press=post disabled=(pending(posted) or draft.trim() == "") testId="post"
      if failed(board)
        text (match failure(board) { case some(f) => f.code, case none => "" }) testId="read-error"
      if not pending(account) and not failed(account) and account.principal != "" and not failed(board) and board.principal == account.principal
        if not board.complete
          text "Partial answer; more rows may exist." testId="partial"
        for note, i in board.notes key=note.id
          text note.body testId=`note-${i}`
```

`app.ts`:

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused, pick } from './snapback4/snapback.ts';
export const appId = 'com.example.app';
const origin = 'http://127.0.0.1:30100';
export const grants = `net.fetch ${origin}\nsecret.keep snapback-session`;
const sources: Sources = {
  restoreAccount: async ([now], store): Promise<Result<'restoreAccount'>> => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    return {principal: (await db.restore(now))?.principal ?? ''};
  },
  notes: async ([principal, now], store): Promise<Result<'notes'>> => {
    if (!principal) return {principal: '', complete: true, notes: []};
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    if (db.session?.principal !== principal) throw refused({code: 'E_AUTH', message: 'Sign in again.'});
    const rows = await db.readAll(api.recent, {});
    return {principal, complete: rows.complete, notes: rows.map(row => pick<Result<'notes'>['notes'][number]>()(row, 'id', 'author', 'body'))};
  },
  signup: async ([email, password, now], store): Promise<Result<'signup'>> => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    const signed = await db.signup(email, password);
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  login: async ([email, password, now], store): Promise<Result<'login'>> => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    const signed = await db.login(email, password);
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  logout: async ([now], store): Promise<Result<'logout'>> => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    await db.logout(); // local sign-out always succeeds, even if revocation cannot be confirmed
    return {ok: true};
  },
  post: async ([body, now], store): Promise<Result<'post'>> => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now: Math.floor(now)}});
    const sent = await db.send(api.post, {body});
    if (sent.state === 'failed') throw refused(sent.why);
    return {ok: true};
  },
};
import { api } from './backend/api.ts';
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

`app.test.contract` (against `dev`; `fresh` resets accounts as well as rows):

```contract
fresh
epoch now

test "password sessions restore and sign out"
  expect text "session" == "Signed out"
  type "email" "alice@example.com"
  type "password" "correct horse battery staple"
  tap "signup"
  clock data
  expect text "session" == "Signed in"
  type "draft" "password hello"
  tap "post"
  clock data
  expect text "note-0" == "password hello"
  reload
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "password hello"
  tap "logout"
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  reload
  clock data
  expect text "session" == "Signed out"
  type "email" "alice@example.com"
  type "password" "wrong password"
  tap "login"
  clock data
  expect text "auth-error" == "E_AUTH"
  expect text "session" == "Signed out"
  type "password" "correct horse battery staple"
  tap "login"
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "password hello"

test "a new test has no account or browser session"
  expect text "session" == "Signed out"
  expect tree missing "note-0"
```

Generate the declaration, start `bun exact.mjs backend`, and run
`bun exact.mjs test web`. The same UI works on `serve`; omit `fresh` there
and use a unique signup email. `restore(now)` validates stored sessions and
refreshes within 60 seconds of expiry; `refreshSession(now)` rotates explicitly.
Use a different secret key for each origin. Auth operations serialize across Exact's
per-source secret facades; a late auth reply never overwrites a changed session.
An old-token request denied during refresh waits for auth to finish before
invalidating, so it cannot discard the replacement token. A later poll still
returns no expired session, but does not delete the old secret while its matching
rotation is in flight; the pending refresh can commit its replacement.
Await `logout` before another
sign-in; its `ok` confirms server revocation, and `offline` means that could
not be confirmed. Reads and writes interrupted by an account change throw
`E_STALE`; query denials with `E_AUTH` forget the matching stored token.

### A complete mixed identity app

Install the pinned Snapback4 in the backend directory, write `snapback/schema.q`
below, then run `bun <exact2>/scripts/exact.mjs new app --snapback --online`.
Replace the explorer with these files. `snapback --port` selects the backend at bake.
This small public guestbook uses a bounded query; a partial answer is refused.

`snapback/schema.q`:

```quarry
use identity
use personas alice, bob

table notes:
  author: principal
  body: text 1..200 ?
  at: time
  by byTime: at, id
  public 'guestbook notes are public'
  insert <- .author = viewer
  update <- deny
  delete <- deny
  online only

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200 ?):
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
```

`app.contract`:

```contract
shape Clock
  epochAtZero: number
shape Account
  principal: string
shape Note
  id: string
  body: string
shape Board
  notes: Note[]
shape Ack
  ok: bool

component App
  state persona = ""
  state email = ""
  state password = ""
  state draft = ""
  resource time = exactTime() as shape Clock
  resource account = restoreAccount(time.epochAtZero) as shape Account else empty()
  resource board = notes(persona, account.principal, time.epochAtZero) as shape Board else empty()
  mutation signed as shape Ack refreshes account, board
  mutation posted as shape Ack refreshes board
  derive active = persona != "" or account.principal != ""
  action choosePersona
    persona = "alice"
  action leave
    persona = ""
    send signed = logout(time.epochAtZero + performanceNow())
  action editEmail(value: string)
    email = value
  action editPassword(value: string)
    password = value
  action editDraft(value: string)
    draft = value
  action guest
    send signed = guest(time.epochAtZero + performanceNow())
  action signup
    send signed = signup(email, password, time.epochAtZero + performanceNow())
  action post
    send posted = post(persona, draft.trim() == "" ? none : some(draft.trim()), time.epochAtZero + performanceNow())
  view
    column gap=8
      text (active ? "Signed in" : "Signed out") testId="session"
      text (match failure(signed) { case some(f) => "Couldn't sign in. Try again.", case none => "" }) testId="problem"
      if not active
        button "Use Alice (dev only)" press=choosePersona testId="alice"
        button "Continue as guest" press=guest disabled=pending(signed) testId="guest"
        input value=email input=editEmail type="email" aria-label="Email" testId="email"
        input value=password input=editPassword type="password" aria-label="Password" testId="password"
        button "Sign up" press=signup disabled=pending(signed) testId="signup"
      else
        button "Log out" press=leave disabled=pending(signed) testId="logout"
        input value=draft input=editDraft aria-label="Optional note" testId="draft"
        button "Post" press=post disabled=pending(posted) testId="post"
        if not pending(account) and not failed(account) and not pending(board) and not failed(board)
          for note, i in board.notes key=note.id
            text note.body testId=`note-${i}`
```

`app.ts`:

```ts
import type {Answer, Result, Sources} from './app.contract.d.ts';
import {Snapback, refused, pick} from './snapback4/snapback.ts';
import {api} from './backend/api.ts';
export const appId = 'com.example.app';
const origin = 'http://127.0.0.1:30100';
export const grants = `net.fetch ${origin}\nsecret.keep snapback-session`;
const note = pick<Result<'notes'>['notes'][number]>();
const sources: Sources = {
  restoreAccount: async ([now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    return {principal: (await db.restore(now))?.principal ?? ''};
  },
  notes: async ([persona, principal, now], store) => {
    if (!persona && !principal) return {notes: []};
    const db = persona
      ? new Snapback({origin, headers: () => ({'x-snapback-persona': persona})})
      : new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    if (!persona && db.session?.principal !== principal) throw refused({code: 'E_AUTH', message: 'Sign in again.'});
    const rows = await db.readAll(api.recent, {});
    if (!rows.complete) throw refused({code: 'E_PARTIAL', message: 'The full guestbook is unavailable.'});
    return {notes: rows.map(row => note({...row, body: row.body ?? '(no note)'}, 'id', 'body'))};
  },
  guest: async ([now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const config = await db.authConfig();
    if (!config.guests) throw refused({code: 'E_AUTH', message: 'Guests are unavailable.'});
    const signed = await db.guest();
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  signup: async ([email, password, now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const signed = await db.signup(email, password);
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  logout: async ([now], store) => {
    await new Snapback({origin, auth: {store, key: 'snapback-session', now}}).logout();
    return {ok: true};
  },
  post: async ([persona, body, now], store) => {
    const db = persona
      ? new Snapback({origin, headers: () => ({'x-snapback-persona': persona})})
      : new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const sent = await db.send(api.post, {body});
    if (sent.state === 'failed') throw refused(sent.why);
    return {ok: true};
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

`app.test.contract`:

```contract-test
fresh
epoch now

test "guests authenticate before reading and can omit an optional note"
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  tap "guest"
  clock data
  expect text "session" == "Signed in"
  tap "post"
  clock data
  expect text "note-0" == "(no note)"
  reload
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "(no note)"
  tap "logout"
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"

test "persona reload signs out while a password session restores"
  expect text "session" == "Signed out"
  tap "alice"
  clock data
  type "draft" "persona hello"
  tap "post"
  clock data
  expect text "note-0" == "persona hello"
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  type "email" "alice@example.com"
  type "password" "correct horse battery staple"
  tap "signup"
  clock data
  expect text "session" == "Signed in"
  type "draft" "account hello"
  tap "post"
  clock data
  expect text "note-0" == "account hello"
  reload
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "account hello"
  tap "logout"
  clock data
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
```

Run `bun exact.mjs contract types app.contract -o app.contract.d.ts`,
`bun exact.mjs backend`, then `bun exact.mjs test web` in another terminal.
The README retest command executes this example along with the guestbooks.
The existing [password example](#a-complete-online-password-app)
adds login and periodic session refresh. Remove persona controls for production;
`serve` refuses persona headers. Use a distinct secret key for each backend origin.

### Live resources on the online adapter

One Contract timer can refresh every resource that needs live data. The password
example above samples the clock into state in `tick`, then refreshes `account`
and `board`. A write uses `refreshes board` for its immediate update. This also
catches writes from another browser on the next poll; no JavaScript timer or
`Date.now()` belongs in the data module. Use `clock +5000` followed by `clock data`
in a test. A direct `performanceNow()` argument, including one through a derive
or function, produces `analyze-resource-clock`: it changes every frame.

Resources keep their last value on failure. Hide private data while the account
is pending or failed, and check that the answer's principal matches the current
account before rendering it, as above. A denial must throw `refused(denied)`,
not turn into an empty successful list. For device-backed apps, serialize reads
with writes through `Snapback.switcher.use`, and await `send`/`sync` before reading
server authority. The online adapter has no outbox; always await `send` before
a dependent read. A cut page remains partial even after `readAll` has followed
all available cursors; `rows.complete` and `rows.cut` retain those facts.

### Tests share the server

Each test opens the app on a device of its own, but every test of a run, and
every run, talks to the one `snapback4 dev` at the origin, which keeps what
each wrote. A later test sees an earlier test's rows and a second run sees
the first's: a total is off, a `unique` code answers `CODE_TAKEN`, a position
holds another test's row. A failed test whose server already held commits
when it began says so in a note, counting those from before the run and
those from earlier tests in it. Two ways out:

- **Each test starts empty: `fresh`.** A `fresh` line at the top of the
  test file (or leading one test) empties the `snapback4 dev` server the
  app fetches before each test, through the server's development door
  `POST /dev/reset` (the app's tables and derived data; it answers
  `{reset, seeded, generation}`); `fresh "seed.json"` then seeds it, as the
  system, with that file's rows, a path from the test file:
  `[{"table": "owners", "row": {"id": "01ARZ3NDEKTSV4RRFFQ69G5FAV", "person": "dev:alice"}}]`.

  ```
  epoch now
  fresh

  test "a new board has no notes"
    clock data
    expect tree missing "note-0"
  ```

  Snapback4 0.4.29 has this door in `dev`; `serve` has no development
  reset. `fresh` is for tests against `dev`.
- **Each test owns its rows.** It writes a code or name no other test uses
  and asserts on that: newest first (`note-0` in a `last … by byTime` list),
  or through its own reply (the guestbook's `notice`, a derive of the reply's
  id). Never a total of a shared table, or "the list is empty". The
  guestbook's tests are of this kind, so they pass again on what a first run
  leaves.
### Ports: the Snapback origin and the page's own

The backend defaults to `const origin` in `app.ts`. Change it without editing
sources using `EXACT_BACKEND_ORIGIN=https://api.example` at build/run time, or
`bun exact.mjs test web --backend http://127.0.0.1:30100`. The build injects that
origin and its matching `net.fetch` grant into its captured module; authored
files retain their default. Use `net.fetch ${origin}` for the grant. A computed
origin is refused when overriding it; declare one top-level literal instead.
`fail fetch "backend"` follows the built grant, including an override.

`bun exact.mjs snapback --port N` stores a local development selection in
`.exact/backend-origin` (ignored), used by builds and `bun exact.mjs backend`.
The authored `app.ts` `origin` constant is overridden at bake by that file.
`EXACT_BACKEND_ORIGIN` wins over that selection. Existing sources and literal
test prefixes are preserved; use the `backend` alias in portable tests.
A compatible stamped wasm is reused for a port-only change; `--rebuild` forces
its rebuild. Backend preflight reads the built grant and names a missing server.

**The Exact page's port** is separate: `EXACT_PORT=<n>` (or `--port <n>`)
serves `web`, `test web` and `agent web` on exactly that port, else a free
one selected within `PORT_LO`–`PORT_HI` when both are set, else by the OS
for temporary tests; no remembered development
page or temporary-store hash supplies it. Reload keeps the same page origin. It never changes the origin the app fetches, and `--port` on `test web`
is the page's, not the server's. On a machine that assigns you ports, take
one for `snapback4 dev --port` and the origin, and another for `EXACT_PORT`.

### Contract answers and vocabulary

| Resource kind | What `app.ts` returns |
| --- | --- |
| `as shape Board` | A record object, such as `{notes, complete}` |
| `as shape Note[]` | The list array itself, such as `rows.map(row => note(row, 'id', 'body'))` |
| A mutation `as shape Ack` | The declared record on success; throw `refused(why)` on refusal |

A list source returns an array, without a `{data: …}` or `{notes: …}` wrapper.
A backend point lookup returns an object; a list query returns an array. Unwrap
the adapter's `read.data` and project it before returning the Contract answer.

`match` is an expression; use it inside `fn`, a derive, or an `if` condition
(`if (match failure(posted) { case some(f) => f.code == "FULL", case none => false })`).
The indented statement form in an action matches only `some`/`none`. Write
`refreshes board, feed` with commas. A grouped platform list needs both
`appearance="auto"` and `listStyle="inset-grouped"`; its `section`s hold the rows.

An optional argument is `none` (or `some(value)`), rather than an empty string:
`send posted = post(draft.trim() == "" ? none : some(draft.trim()))`.
The source receives `string | null`; send `{note}` to an optional Snapback
`note: text 1..200 ?` argument. For an omitted property instead, build the
arguments in TypeScript: `const args = note === null ? {} : {note}`.

`navigate=follow` belongs on the first root outside regions, alongside
``navigationKey=`${top(nav).id}```; `navigationBack="back"` is optional.
`top(nav).params.id` reads the current entry. `params(nav, "id")` returns the
selected stack's nonempty ids, bottom first. Tests read slot paths, such as
`expect state board.notes.0.body == "hello"`, rather than DOM ids in `expect state`.
See [navigation](../docs/recipes/navigation-stack.md).

A PDF picker needs `input type="file" accept="application/pdf"` and an app manifest
handler such as `"file_handlers": [{"action":"/", "accept":{"application/pdf":[".pdf"]}}]`.
Give the input an `id` for `showPicker` and a `testId` for `pick`.

A generic helper forwarding record arguments needs the same constraint as the
adapter: `function helper<A extends Record<string, unknown>>(args: A) { … }`.
A plain unconstrained `A` cannot stand for a JSON argument record.

### Typed operations

`snapback4 check` (and `dev`) writes `snapback/generated/api.ts`: `api`, a
handle per query and mutation, typed by the schema. Mount that directory
(`bun exact.mjs snapback` does, as `backend`) and pass a handle where a name goes:

```ts
import { api } from './backend/api.ts';
const read = await db.read(api.recent, {}, now);     // Read<{ id, author, body, at, pending? }[]>
const rows = await db.readAll(api.recent, {}, now);  // the same rows, every page
const fate = await db.send(api.post, { body }, now); // fate.result: { id: string }
```

The arguments are checked against the operation's (an unknown field fails
the build: TS2353 names it; a Contract `string` given to an `enum` argument
needs its type, `role as ArgsOf<typeof api.join>['role']` with `ArgsOf` from
`./snapback4/snapback.ts`, and the server refuses any other value with
`E_INPUT`), the answer is the server's type, and a query's
handle is refused where a mutation goes. `write(api.post, …)` takes one
too. A name still works (`db.read<Row[]>('recent', {}, now)`), typed by
hand. `api.ts` imports its types from `snapback4/contract`; the build reads
them from the `snapback4` package installed above `snapback/generated`
(the backend's), so nothing else is installed in the app. With no
`snapback4` above it the build fails with `backend/api.ts: error TS2307:
Cannot find module 'snapback4/contract'` (listed after the errors it causes
in `app.ts`: install the package, not types by hand). Use `pick<T>()` to project
backend rows onto Contract shapes: generated rows carry fields (`at`, `pending`)
the view may not declare.

Both adapters export a checked projection helper:
`const note = pick<Result<'notes'>['notes'][number]>();`
then `note(row, 'id', 'author', 'body', 'pending')`. It emits only those keys;
missing target keys, misspellings and incompatible row values fail TypeScript.
Normalize optional fields before projecting, as the guestbook does for `pending`.
A field projected through a hidden parent join is nullable even when the child's
column is required: handle `row.parentField ?? fallback` at that boundary.
The guestbook's shared `ack(sent)` also accepts `Sent<null>`; a void mutation
needs no invented result object.

For a Snapback mutation, `Result<'post'>` constrains a bool `ok` field to `true`:
a false answer must throw `refused(why)` to populate `failure(posted)`. Give a
helper that result annotation, as `ack` does, or preserve an inferred success
literal with `ok: true as const`. Handled failures inside helpers are ordinary
values; resource result types retain their bool fields. The compiler supplies
mutation source names from the plan, including calls in imported actions.

### One view across a rollback

Both adapters expose `release(): Promise<ReleaseSnapshot>` and
`subscribeGeneration(generation => …)` (returns an unsubscribe function).
`release()` uses authenticated `GET /` with the same credentials as data traffic.
A subscription reports generations observed by requests; it starts no timer.
Keep a Contract refresh running even when every operation gate is closed:

```ts
// Inside a source, using the same db as reads and writes:
const release = await db.release();
return {canPost: api.post.available(release)};
```

```contract
shape Features
  canPost: bool
component App
  state draft = ""
  resource features = features() as shape Features else empty()
  task catalog mount
    every(1000, refreshFeatures)
  action refreshFeatures
    refresh features
  action edit(value: string)
    draft = value
  view
    column
      input value=draft input=edit testId="draft"
      when features.canPost
        text "Posting available" testId="available"
      else
        text "Posting unavailable in this release" testId="unavailable"
```

Use that same gate on the posting control. Keep draft/selection state above the
gate: rolling back removes the control without remounting the view or losing the
draft. A rollback has a new generation; an older in-flight catalog cannot reopen
its gate. Loading/offline catalogs close it. Availability can race a deployment,
so still handle `E_OP`/`E_INPUT` from the write. Subscriptions can trigger an
embedding's refresh; Contract apps normally use the task above.

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
- **Failures in the view.** A resource or an admitted mutation request that throws
  exposes `failure(x)`. `refused(why)` preserves the domain code in both `code` and
  `reason`, so branch on `f.code == "SLOT_FULL"` or `"NOT_MANAGER"`. Transport,
  storage and shape errors without a domain reason use Exact's infrastructure
  classes (`offline`, `timeout`, `refused`, `storage`, `shape`, `error`). A failed
  mutation keeps its previous value, runs no `then` and never retries automatically;
  a new send, assignment or success clears its failure. `message` is developer text.
  The driver's own refusals follow the same contract.
  `readOptional(api.announcements, args, now, ['NOT_SIGNED_UP'])` reads one
  page and maps only those named application refusals to `data: null`,
  retaining `denied` and marking `complete: false`. Render an empty view for
  that deliberate refusal; `E_OFFLINE`, authentication, input and retryable
  failures still throw, even if named in that list. Other application codes
  throw too. This helper does not paginate; use `readAll` for a complete list.
  `readAll` throws a page's (where `read` answers `denied`), and a first
  read that cannot reach the server throws `E_OFFLINE`. A mutation's
  refusal is the outcome's `why`: throw `refused(fate.why)` from the source
  to fill `failure(slot)`. For a mutation whose shape has a bool `ok`, the
  Snapback bake requires `ok: true` in its TypeScript result. A failed result
  must throw `refused(why)`. A mutation source's
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
  reaches the server, once each. A mutation source answers once: a write it
  answered as pending stays so in its slot when a later round delivers it.
  Show delivery from a resource that reads again (the guestbook's `sending`
  count), not from the slot.
- **In tests.** Let the app sync once (`clock data`) before `fail fetch`: a
  partition that has never synced cannot open offline. Assert a write's
  fate (`outcome`), not a notice the app shows while it is still pending.
- **Device and server ids have different minting orders.** Use an explicit order
  column (for example `at: now` with an index) for creation order on both.
- **Ids the server mints are unique, not ordered.** A row's `id` does not
  sort in creation order (a later row's can sort first), so never take the
  largest id as the newest: give the table a `time` the mutation sets (`at:
  now`) and read in its index's order (`last 50 by byTime`). A test file
  cannot know a minted id either, so a `testId` built from one is out of
  its reach. Reach a row by position, from `for`'s index (`each note, i in
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

### Photos and video: upload and download

Device-backed apps can keep a picked file and its placing mutation in one durable
outbox entry, after identifying the backend once online:

```ts
const intent = await db.uploadAndPlace(api.addPhoto, {}, path, now,
  {argument: 'photo', id: draftId}); // id is an optional stable intent key
// Admission has copied bytes into the device. The picked path may now disappear.
await db.sync();                    // uploads, commits the asset facts, then places
const fate = await db.outcome(intent.id);
```

Grant `fs.read` for the picked path and pass the dispatcher's `storage`. Exact
currently admits nonempty durable files up to **4 MiB**, so their hex custody
record fits its 16 MiB storage/call boundary. Larger files may use online
`upload`; Snapback's device quota remains authoritative. Reopening the same
partition resumes queued bytes without the picked path. Lost upload or placement
responses retry the same server intent and mutation id. `uploads()` lists kept
states; a nonretryable refusal stays blocked with its bytes, and
`retryUpload(id)` requeues it for the next sync. `discardUpload(id)` durably
requests cancellation; a sync retires the bytes only after server acknowledgement.
After placement dispatch starts, recover its receipt instead. A blocked upload
leaves the round unsuccessful but does not hold up independent ordinary writes. A retained terminal receipt
prevents different bytes or arguments reusing the same key. Keep the same viewer
and backend store; switching either cannot send the old intent elsewhere.


A Snapback `image <=N` or `video <=N` column holds an asset: the bytes are
uploaded first, and the mutation takes `{ id }`. `db.upload` and
`db.download` are the two routes (`POST /assets`, `GET /assets/<id>`), sent
with either adapter's `headers`, so a persona or a bearer session decides as it
does for a query:

```ts
export const grants = `net.fetch ${origin}\nsqlite.open app:/data\nfs.read app:/tmp\nfs.write app:/cache`;

// A source given the picked file's path (`picked(files: Picked[])`, `f.path`):
const sent = await db.upload(path);                    // the host sends the file; or bytes
if (!sent.ok) return { message: `Upload refused: ${sent.why.code}` };   // E_ASSET_TYPE, E_ASSET_TOO_LARGE, E_OFFLINE …
const fate = await db.send(api.addPhoto, { photo: { id: sent.asset.id } }, now);
// Showing one: the bytes into an app file, whose path an `image` or `video` shows.
const got = await db.download(row.photo.id);           // app:/cache/snapback4-<name>-<id>.png, or download(id, path)
if (got.ok) shown = got.path; else why = got.why.code; // E_ASSET_UNKNOWN: missing, or the row's read rule hides it
```

- `upload(path)` sends an app file (`exactBodyFrom`: the bytes never enter
  JavaScript, at most 64 MiB, under `fs.read`); `upload(bytes)` sends bytes.
  `asset` is what the server sniffed: `id`, `type`, `bytes`, `width`,
  `height`, `duration`. By default the server sniffs JPEG, PNG, WebP and MP4. For a PDF or
  another attachment, use `upload(path, { file: { type: picked.type,
  name: picked.name } })`: it sends `POST /assets?as=file&name=…` with the
  declared content type. Place `{ id: asset.id }` in a `file` column.
  `download` returns the attachment `name` and uses it in the default cache
  filename; pass that name to `saveFile` when exporting it.
- `upload` and `download` are online only and never retried here (a data module has no
  timers): offline is `E_OFFLINE` with nothing kept, so keep the picked
  path and ask again. A write naming an uploaded `{ id }` made offline
  queues unpredicted, and the server judges it.
- `download` writes through the dispatcher's `storage.fs` (pass it, as
  `use` does), to `app:/cache` unless given a path, and keeps nothing for
  you: no retention cap, no purge when the row goes. A `once` asset's
  download spends its one view: give it an `app:/tmp` path.
- Give the input both `id="attachment"` and `testId="attachment"`:
  `showPicker("attachment")` names its `id`; tests name its `testId`.
  In a test, `tap` the button whose action calls `showPicker`, then
  `pick "attachment" "<file>"` (the file relative to the test file).
- Tried on a scratch app against 0.4.16 under `test web` (2026-10-12): a
  picked PNG uploads, is placed, shows from its app file, and another
  persona's download is `E_ASSET_UNKNOWN`. Not yet driven natively.

On the web the partition is the wasm device, persisted through Exact SQLite
after every call, one call at a time; a failed save rebuilds the device from
the disk and keeps what the server said. A second tab finds its database
busy.

If an app's dispatcher supplies an unrelated native module (such as a camera
hatch), set `device: 'web'` in `Snapback.open` or the switcher's options.
It uses the wasm device and never calls that module. `device: 'native'`
requires a supplied Snapback module. Without a choice, a supplied `native`
selects the native device; otherwise it uses web. Pickers for documents need
both `accept` on the input and the app's `file_handlers`; `capture` remains
outside this adapter's camera contract.

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
it; `denied.code == "E_AUTH"` means sign in again. For an `auth` opening, the driver keeps the replacement itself; a later
`open` with Contract time refreshes shortly before `expiresAt`. With manual
`headers`, keep the returned session yourself before the next exchange.

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

`node scripts/retest-snapback-readme.mjs --version <v> [--registry <url>]`
executes all four complete apps above and the auth recipe’s reload test twice against
that registry's package. Pass `--revision <published-source-sha>` when the registry
omits `gitHead`. A failure restores the candidate pins and leaves the claims
unchanged. Only a successful run updates the npm and Cargo pins, lockfiles,
tested-against dates and `readme-tested.json`; it makes no commit.
`node scripts/check-snapback-readme.mjs` refuses claims whose source or pin has
changed since that run. Run the retest command when repinning, rather than editing
the tested-against claims.

This workspace is outside the root's; its lock carries the private Snapback
source. Run `cargo test -p exact-snapback4 -p exact-snapback4-client`
(`bun install` at the root first, for the server binary), `bun test
snapback4/ts`, and clippy for `exact-snapback4-web` with `--target
wasm32-unknown-unknown`.

What the TypeScript client of Snapback itself does that this one does not
yet: retained media caching (LLP 1108: a retained, purged cache and `once` assets shown
from memory), native jobs on the device, Following
feeds, ephemeral reads and writes (a write that touches an ephemeral table
is refused at once with `E_CLIENT_UNSUPPORTED`, never queued), search state, the online fallback fenced by
intersecting predictions (here a read the device cannot vouch for asks the
server only when nothing is queued at all), a server read kept live
(here a query the server answers is asked again, not invalidated by the
change poll; LLP 1110), and the 0.2.32 legacy fallback. `round.rs` names its source in Snapback's `local.ts`.

## Device identity (HS, 2026-10-13)

The device crate emits canonical compact UTF-8 `snapback4.engine` JSON into every linked wasm artifact: `{format:1, version, capabilities}`. Build tooling preserves it and derives `assets/snapback4.wasm.json` for diagnostics; preflight reads the embedded section, including when offline. It compares the same server floor and schema capabilities as Snapback `check`, via `/dev/status` on dev or an authenticated `GET /` on serve. Compatible version differences are allowed. Missing or malformed identity names `bun exact.mjs snapback --rebuild`.

Web and native wrappers expose `engine_info` and `engine_headers` before partition open. Every Rust Fetch carries headers from `snapback4_device::engine::headers()`, and both transport drivers forward them automatically, including the first schema request. Applications supply credentials only. Generated backend operations typecheck using Snapback's DOM-free public declarations.

Uploads currently expose a completed `fetch` response, not transferred-byte events.
Show an indeterminate indicator until the response arrives; determinate progress
requires transport support for loaded/total byte events. Do not estimate a percentage
from elapsed UI time. Picked CSV, TSV, plain text, Markdown, HTML, CSS, XML and YAML
files retain their MIME types; unknown extensions remain `application/octet-stream`.
