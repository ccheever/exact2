# Recipe: your own REST server

A list shared through a small Bun server: `fetch` with a bearer token, polling with a
`task`, every failure answered as words, and an empty database for each test run.

**`server.ts`**, started with `bun server.ts` (`bun:sqlite`; a persona's token is its name).
It listens where app.ts calls it: the address is in one place, app.ts's `API`.

```typescript
import { Database } from 'bun:sqlite';
import { API } from './app.ts';

const db = new Database(process.env.DB ?? 'data.db'); // verify and api-test name an empty one
db.run('CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, by TEXT NOT NULL)');
const PEOPLE: Record<string, string> = { ada: 'Ada', grace: 'Grace' };
const person = (req: Request) => PEOPLE[req.headers.get('authorization')?.replace('Bearer ', '') ?? ''];
// The web app's page is another origin: every answer, and the preflight, allows it.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization, content-type' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });

const { hostname, port } = new URL(API);
Bun.serve({ hostname, port: Number(port), routes: {
  '/health': () => new Response('ok'),
  '/items': {
    OPTIONS: () => new Response(null, { status: 204, headers: CORS }),
    GET: (req) => person(req) ? send(db.query('SELECT id, title, by FROM items ORDER BY id').all())
      : send({ error: 'Sign in again.' }, 401),
    POST: async (req) => {
      const by = person(req), { title } = await req.json().catch(() => ({}));
      if (!by) return send({ error: 'Sign in again.' }, 401);
      if (typeof title !== 'string' || !title.trim()) return send({ error: 'Enter a title.' }, 400);
      if (db.query('SELECT 1 FROM items WHERE lower(title) = lower(?)').get(title.trim())) return send({ error: `${title.trim()} is already on the list.` }, 409);
      db.query('INSERT INTO items (title, by) VALUES (?, ?)').run(title.trim(), by);
      return send({ ok: true }, 201);
    },
  },
} });
```

**The app**: the list is read again every 4 seconds, and after each write.

```contract
routes nav
  home "/"

shape Item
  id: string
  title: string
  by: string

shape Items
  ok: bool
  error: string
  items: list<Item>

shape Reply
  ok: bool
  error: string

component Shared
  state draft = ""
  resource items = loadItems() as shape Items else empty(ok=true)
  mutation added as shape Reply refreshes items then afterAdd
  derive addError = match added { case some(r) => r.error, case none => "" }
  derive listError = pending(added) ? "" : addError != "" ? addError : items.error
  task poll mount
    every(4000, pull)
  action pull
    refresh items
  action edit(v: string)
    draft = v
  action add
    if trim(draft) != "" and not pending(added)
      send added = addItem(trim(draft))
  action afterAdd
    if addError == ""
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Shared List" role="heading" aria-level=1
          list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                column flex=1 min-width=0 gap=4
                  input value=draft input=edit submit=add placeholder="Add an item"
                    aria-label="New item" testId="draft"
                  when listError != ""
                    text listError font="-exact-footnote" color="-exact-system-red" testId="error"
            section
              each i in items.items key=i.id
                row
                  text i.title testId=`item-${i.id}`
                  text i.by
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.shared'; // exact new writes this line
export const API = 'http://127.0.0.1:8787'; // the server's address, its one place
export const grants = `net.fetch ${API}`;
const TOKEN = 'ada'; // a real sign-in keeps its token with `secret.keep token` and store.set

type Answered = { ok: true; body: unknown } | { ok: false; error: string };
async function call(path: string, init: RequestInit = {}): Promise<Answered> {
  try {
    const res = await fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
      headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' } });
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    if (res.ok) return { ok: true, body };
    return { ok: false, error: typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.' };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { ok: false, error: 'Can’t reach the server. Check that it’s running, then try again.' }; // the field keeps an unsent add
  }
}

let last: Result<'loadItems'>['items'] = []; // a failed poll keeps the list on screen
const sources: Sources = {
  loadItems: async () => {
    const r = await call('/items');
    if (!r.ok) return { ok: false, error: r.error, items: last };
    last = (r.body as Array<{ id: number; title: string; by: string }>).map((i) => ({ id: String(i.id), title: i.title, by: i.by }));
    return { ok: true, error: '', items: last };
  },
  addItem: async ([title]) => {
    const r = await call('/items', { method: 'POST', body: JSON.stringify({ title }) });
    return r.ok ? { ok: true, error: '' } : { ok: false, error: r.error };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "an item is added for everyone, and another person's arrives on the poll; a refusal and a lost server are said in words"
  type "draft" "Milk"
  type "draft" key "Enter"
  clock data
  expect text "item-1" == "Milk"
  request POST "/items" {"title": "Bread"} as grace
  clock +4000
  clock data
  expect text "item-2" == "Bread"
  type "draft" "milk"
  type "draft" key "Enter"
  clock data
  expect text "error" == "milk is already on the list."
  fail fetch "http://127.0.0.1:8787"
  type "draft" "Eggs"
  type "draft" key "Enter"
  clock data
  expect text "error" == "Can’t reach the server. Check that it’s running, then try again."
  expect text "draft" == "Eggs"
  pass fetch "http://127.0.0.1:8787"
  type "draft" key "Enter"
  clock data
  expect text "item-3" == "Eggs"
  expect text "draft" == ""
```

**`verify` runs the server itself**, on an empty database: it finds server.ts (or
server/index.ts, or app.json's `commands.server`), starts it with `DB` naming a scratch
file at the address app.ts calls, keeps it through the tests, their end screens and the
relaunch, then stops it and removes the file. Your data.db is never touched. A server
already answering there, your dev server, is refused in one line naming its pid: stop it
first. `bun exact.mjs api-test web` runs `test` the same way; its `test.mjs` (in
`app.json`'s `"commands": {"api-test": ["bun", "test.mjs"]}`) is a few lines:

```js
if (!process.env.EXACT2) throw new Error('run it as bun exact.mjs api-test web');
const { withAppServer } = await import(`${process.env.EXACT2}/scripts/verify.mjs`);
process.exit(await withAppServer(import.meta.dir, () =>
  Bun.spawn(['bun', 'exact.mjs', 'test', ...process.argv.slice(2)], { cwd: import.meta.dir, stdio: ['inherit', 'inherit', 'inherit'] }).exited));
```

- **A second person is a test line** (LLP 1116.006 D8): `request POST "/items" {"title":
  "Bread"} as grace` is the driver calling the server, under app.ts's `API`, with
  `Authorization: Bearer grace` (a persona's token is its name). The app shows it when it
  next reads, so `clock +4000` (the poll) and `clock data` come before the `expect`. A
  status outside 2xx fails the step with the status and body (`as nobody`: 401 `Sign in
  again.`). No curl timed against the poll.
- Every failure is an answer the view shows in words: the server's own `error`, or a
  sentence for a lost connection. Never a status code, `Refused: E_INPUT`, or a
  "Synced" line. The field clears in `then`, only on an ok reply, so a refused or unsent
  add keeps its text ([after-a-write](after-a-write.md)).
- `task poll mount` with `every(4000, pull)` polls; `refreshes items` reads again
  after each write. A failed poll keeps the last list (`last`) under its message.
- The grant names the server's origin, app.ts's `API`. On the web the page is another
  origin, so the server answers CORS (the `OPTIONS` preflight too). A phone reaches the
  Mac by its LAN address or HTTPS: change `API`, and the server listens there too.
- Before calling it done, stop the dev server and run `verify`: it runs the server on an
  empty database, its tests write to that, and its relaunch reads it back on real time
  ([tests](tests.md), "Before you call it done"). A test's `fail fetch` names the same
  address as app.ts.
